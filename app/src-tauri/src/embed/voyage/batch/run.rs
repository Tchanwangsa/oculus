//! Running one document: render, pack, dispatch, and gather the pages.

use super::{refuse_oversized, Limits, RequestRun, MAX_RENDER_PIXELS};
use crate::embed::raster;
use crate::embed::raster::{RasterError, RenderedPage};
use crate::embed::{EmbedError, EmbedPage, Progress, Wait};
use crate::providers::ratelimit::hold;
use std::collections::BTreeMap;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

struct RunState {
    pages: Vec<EmbedPage>,
    done: u32,
    in_flight: usize,
    failure: Option<EmbedError>,
    /// Each held-back request's wait, keyed by its dispatch number.
    waits: BTreeMap<usize, Wait>,
}

impl RunState {
    /// What the row should show: the wait that ends first, since that is when
    /// the document next moves.
    fn waiting(&self) -> Option<Wait> {
        self.waits
            .values()
            .min_by_key(|wait| wait.until_ms)
            .copied()
    }
}

struct DocumentRun {
    state: Mutex<RunState>,
    changed: Condvar,
    /// Requests allowed out at once.
    in_flight: usize,
}

impl DocumentRun {
    fn new(in_flight: usize) -> Self {
        Self {
            state: Mutex::new(RunState {
                pages: Vec::new(),
                done: 0,
                in_flight: 0,
                failure: None,
                waits: BTreeMap::new(),
            }),
            changed: Condvar::new(),
            in_flight: in_flight.max(1),
        }
    }

    fn wait_for_change<'a>(&self, state: MutexGuard<'a, RunState>) -> MutexGuard<'a, RunState> {
        self.changed
            .wait(state)
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn set_wait(&self, request: usize, wait: Option<Wait>) {
        let mut state = hold(&self.state);
        let changed = match wait {
            Some(wait) => state.waits.insert(request, wait) != Some(wait),
            None => state.waits.remove(&request).is_some(),
        };
        drop(state);
        if changed {
            self.changed.notify_all();
        }
    }

    fn failure(&self) -> Option<EmbedError> {
        hold(&self.state).failure.clone()
    }

    /// First failure wins, so a later panic cannot overwrite the real reason.
    fn fail(&self, error: EmbedError) {
        let mut state = hold(&self.state);
        if state.failure.is_none() {
            state.failure = Some(error);
        }
        drop(state);
        self.changed.notify_all();
    }

    fn finished(&self, pages: Vec<EmbedPage>) {
        let mut state = hold(&self.state);
        state.done += pages.len() as u32;
        state.pages.extend(pages);
        drop(state);
        self.changed.notify_all();
    }

    fn leave(&self) {
        let mut state = hold(&self.state);
        state.in_flight = state.in_flight.saturating_sub(1);
        drop(state);
        self.changed.notify_all();
    }
}

/// Reports for the run, from the calling thread: `on_progress` is neither
/// `Send` nor `'static`, so workers record numbers behind the lock and this
/// says them, once each time the page count or the wait changes.
struct Narrator<'a> {
    on_progress: &'a dyn Fn(Progress),
    total: u32,
    last: Option<(u32, Option<Wait>)>,
}

impl<'a> Narrator<'a> {
    /// Something not yet said, read under the lock and said after it.
    fn news(&self, state: &RunState) -> Option<(u32, Option<Wait>)> {
        let now = (state.done, state.waiting());
        (self.last != Some(now)).then_some(now)
    }

    fn tell(&mut self, (pages_done, waiting): (u32, Option<Wait>)) {
        self.last = Some((pages_done, waiting));
        (self.on_progress)(Progress {
            pages_done,
            total_pages: self.total,
            backend: crate::embed::voyage::client::BACKEND,
            waiting,
        });
    }

    fn catch_up(&mut self, run: &DocumentRun) {
        let news = self.news(&hold(&run.state));
        if let Some(news) = news {
            self.tell(news);
        }
    }
}

/// Render `pdf`, pack it into requests, embed them with bounded concurrency,
/// and hand back one `EmbedPage` per page — or fail. Progress is reported
/// through a `Narrator`.
pub fn run_document(
    pdf: &Path,
    expected_pages: u32,
    runner: Arc<dyn RequestRun>,
    limits: Limits,
    on_progress: &dyn Fn(Progress),
) -> Result<Vec<EmbedPage>, EmbedError> {
    let run = Arc::new(DocumentRun::new(limits.in_flight));
    let mut workers: Vec<std::thread::JoinHandle<()>> = Vec::new();
    let mut batch: Vec<RenderedPage> = Vec::new();
    let mut spent: u64 = 0;
    // Shared by both passes so neither repeats the other.
    let mut narrator = Narrator {
        on_progress,
        total: expected_pages,
        last: None,
    };
    // `render_pages` stops only on a `RasterError`; the real reason is parked
    // here and the sentinel discarded.
    let mut stop: Option<EmbedError> = None;

    // A page over the render ceiling renders at a lower DPI
    // (`raster::dpi_for_page`); `refuse_oversized` catches the rest.
    let rendered = raster::render_pages(pdf, Some(MAX_RENDER_PIXELS), |page| {
        if let Some(error) = run.failure() {
            stop = Some(error);
            return Err(halt(page.page_no));
        }
        let cost = match refuse_oversized(&page) {
            Ok(cost) => cost,
            Err(error) => {
                stop = Some(error);
                return Err(halt(page.page_no));
            }
        };
        // Re-read every page: see `RequestRun::max_tokens`.
        let ceiling = limits.max_tokens.min(runner.max_tokens());
        if !batch.is_empty() && (batch.len() >= limits.max_inputs || spent + cost > ceiling) {
            dispatch(
                &run,
                &runner,
                std::mem::take(&mut batch),
                &mut workers,
                &mut narrator,
            );
            spent = 0;
            // Reported while rendering too: rendering and embedding overlap.
            narrator.catch_up(&run);
        }
        spent += cost;
        batch.push(page);
        Ok(())
    });

    let outcome: Result<u32, EmbedError> = match rendered {
        Ok(count) => {
            dispatch(
                &run,
                &runner,
                std::mem::take(&mut batch),
                &mut workers,
                &mut narrator,
            );
            Ok(count)
        }
        Err(error) => Err(match stop.take() {
            Some(parked) => parked,
            None => EmbedError::from(error),
        }),
    };

    // Report while the last requests drain, then collect — even when rendering
    // failed, since requests in flight are paid for and must finish.
    let mut state = hold(&run.state);
    loop {
        if let Some(news) = narrator.news(&state) {
            drop(state);
            narrator.tell(news);
            state = hold(&run.state);
            continue;
        }
        if state.in_flight == 0 {
            break;
        }
        state = run.wait_for_change(state);
    }
    let failure = state.failure.clone();
    let pages = std::mem::take(&mut state.pages);
    drop(state);
    for worker in workers {
        worker.join().ok();
    }

    let rendered_pages = outcome?;
    if let Some(error) = failure {
        return Err(error);
    }

    // The boundary guard: the renderer must agree with the parse record's page
    // count (`page_no` is the join key), and every page must have embedded.
    if rendered_pages != expected_pages {
        return Err(EmbedError::Document {
            code: "page-count-mismatch".into(),
        });
    }
    if pages.len() as u32 != expected_pages {
        return Err(EmbedError::Document {
            code: "incomplete".into(),
        });
    }
    Ok(pages)
}

/// Hand one request to a worker. The slot is taken on the calling thread
/// before the spawn: that is the backpressure, and it bounds memory to
/// `in_flight` requests' worth of PNGs. A paced run spends most of its time
/// blocked here, so it keeps reporting while it waits.
fn dispatch(
    run: &Arc<DocumentRun>,
    runner: &Arc<dyn RequestRun>,
    pages: Vec<RenderedPage>,
    workers: &mut Vec<std::thread::JoinHandle<()>>,
    narrator: &mut Narrator,
) {
    if pages.is_empty() {
        return;
    }
    let mut state = hold(&run.state);
    loop {
        if let Some(news) = narrator.news(&state) {
            drop(state);
            narrator.tell(news);
            state = hold(&run.state);
            continue;
        }
        if state.in_flight < run.in_flight {
            break;
        }
        state = run.wait_for_change(state);
    }
    state.in_flight += 1;
    drop(state);

    // Unique among this run's requests: a failed spawn never reports a wait.
    let request = workers.len();
    let worker_run = run.clone();
    let runner = runner.clone();
    let spawned = std::thread::Builder::new()
        .name("voyage-embed-request".into())
        .spawn(move || {
            let numbers: Vec<u32> = pages.iter().map(|page| page.page_no).collect();
            let on_wait = |wait: Option<Wait>| worker_run.set_wait(request, wait);
            let result =
                std::panic::catch_unwind(AssertUnwindSafe(|| runner.run(&pages, &on_wait)));
            // An error or panic mid-wait must not leave the row counting down.
            worker_run.set_wait(request, None);
            drop(pages);
            match result {
                Ok(Ok(vectors)) if vectors.len() == numbers.len() => {
                    let mut built = Vec::with_capacity(vectors.len());
                    let mut failed = None;
                    for (page_no, vector) in numbers.iter().zip(vectors) {
                        match EmbedPage::new(*page_no, &vector) {
                            Ok(page) => built.push(page),
                            Err(error) => {
                                failed = Some(error);
                                break;
                            }
                        }
                    }
                    match failed {
                        Some(error) => worker_run.fail(error),
                        None => worker_run.finished(built),
                    }
                }
                Ok(Ok(_)) => worker_run.fail(EmbedError::Document {
                    code: "vector-count-mismatch".into(),
                }),
                Ok(Err(error)) => worker_run.fail(error),
                // Otherwise a panic would park the caller in the progress loop.
                Err(_) => worker_run.fail(EmbedError::Io("the embedding worker panicked".into())),
            }
            worker_run.leave();
        });

    match spawned {
        Ok(handle) => workers.push(handle),
        Err(error) => {
            run.fail(EmbedError::Io(format!(
                "start an embedding worker: {error}"
            )));
            run.leave();
        }
    }
}

/// The sentinel that stops `render_pages` early. Its message never surfaces —
/// the caller replaces it with the reason it parked.
fn halt(page_no: u32) -> RasterError {
    RasterError::Page {
        page_no,
        message: "stopped by the embedder".into(),
    }
}

/// Reconcile the renderer's vocabulary into the seam's. A render thread that
/// would not start says nothing about the document: `NotReady`, retryable.
impl From<RasterError> for EmbedError {
    fn from(error: RasterError) -> Self {
        match error {
            RasterError::Worker(_) => EmbedError::NotReady {
                backend: "page renderer".into(),
            },
            RasterError::Unreadable(_) => EmbedError::Document {
                code: "unreadable-pdf".into(),
            },
            RasterError::Encrypted => EmbedError::Document {
                code: "encrypted-pdf".into(),
            },
            RasterError::Empty => EmbedError::Document {
                code: "empty-pdf".into(),
            },
            // The message is the renderer's; only the page number travels.
            RasterError::Page { page_no, .. } => EmbedError::Document {
                code: format!("page-render-failed-p{page_no}"),
            },
        }
    }
}
