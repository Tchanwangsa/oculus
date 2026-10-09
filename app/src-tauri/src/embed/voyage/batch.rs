//! How pages are packed into requests, and how many requests run at once.
//!
//! Voyage batches pages, not documents, and never across documents: TPM binds
//! at every tier, so cross-document packing would buy nothing and cost a
//! failure spanning two files.
//!
//! * Both per-request ceilings apply (inputs and tokens), with tokens computed
//!   from each page's real pixels, never a page count.
//! * Progress is summed from finished requests. A request held back by a
//!   rate limit reports the wait, so a paced run is not mistaken for a hang.
//! * Nothing partial escapes: a document that did not embed every page is an
//!   error and no record is written.

use std::collections::BTreeMap;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use crate::embed::raster::{self, RasterError, RenderedPage};
use crate::embed::{EmbedError, EmbedPage, Progress, Wait};
use crate::ratelimit::hold;

// ── The documented ceilings ──────────────────────────────────────────────────

/// Inputs per request. Binds only for small pages.
pub const MAX_INPUTS_PER_REQUEST: usize = 1_000;

/// Tokens per request — the API's, not the account's (see
/// `RequestRun::max_tokens`).
pub const MAX_TOKENS_PER_REQUEST: u64 = 320_000;

/// Tokens in one input. A page cannot be split, so past this is a document
/// error.
pub const MAX_TOKENS_PER_INPUT: u64 = 32_000;

/// Pixels in one image; `run_document` renders oversized pages at a lower DPI
/// to stay under it.
pub const MAX_PIXELS_PER_IMAGE: u64 = 16_000_000;

/// Bytes in one image.
pub const MAX_BYTES_PER_IMAGE: u64 = 20 * 1024 * 1024;

/// Images bill one token per this many pixels.
pub const PIXELS_PER_TOKEN: u64 = 560;

/// Voyage downscales an image to this many pixels before it bills (observed in
/// its `usage.image_pixels`). A full-DPI slide is over it, so every such page
/// costs `tokens_for` the cap; an uncapped estimate would be ~2x wrong. This
/// affects only the estimate — rendering still happens at `RENDER_DPI`.
pub const BILLED_PIXEL_CAP: u64 = 2_000_000;

/// What one page costs after the downscale, rounded up: the ledger reserves
/// from this and must never come in under what is billed.
pub fn tokens_for(width: u32, height: u32) -> u64 {
    let pixels = u64::from(width) * u64::from(height);
    pixels.min(BILLED_PIXEL_CAP).div_ceil(PIXELS_PER_TOKEN)
}

/// The pixels on the page, for the hard API limits; billing is [`billed_pixels`].
pub fn raw_pixels(page: &RenderedPage) -> u64 {
    u64::from(page.width) * u64::from(page.height)
}

/// The pixels Voyage will charge for, which is what the ledger counts.
pub fn billed_pixels(page: &RenderedPage) -> u64 {
    raw_pixels(page).min(BILLED_PIXEL_CAP)
}

/// Why this one page cannot be sent, if it cannot — a document failure, never
/// a skip (that would be a short record). The rasterizer's DPI clamp usually
/// keeps pixels under the limit; this catches what it cannot, plus bytes.
pub fn refuse_oversized(page: &RenderedPage) -> Result<u64, EmbedError> {
    let tokens = tokens_for(page.width, page.height);
    // The token check cannot fire while the billing cap holds; it enforces the
    // documented limit in case Voyage stops downscaling.
    let code = if raw_pixels(page) > MAX_PIXELS_PER_IMAGE {
        "page-too-many-pixels"
    } else if page.png.len() as u64 > MAX_BYTES_PER_IMAGE {
        "page-too-large"
    } else if tokens > MAX_TOKENS_PER_INPUT {
        "page-too-many-tokens"
    } else {
        return Ok(tokens);
    };
    Err(EmbedError::Document {
        code: format!("{code}-p{}", page.page_no),
    })
}

/// The packing rule, also used by `estimate`: given each page's token cost in
/// order, which pages travel together? Greedy and order-preserving, so
/// `page_no` stays stable from render to record.
pub fn plan(costs: &[u64], max_inputs: usize, max_tokens: u64) -> Vec<Vec<usize>> {
    let max_inputs = max_inputs.max(1);
    let mut requests: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut spent: u64 = 0;

    for (index, cost) in costs.iter().enumerate() {
        let full = current.len() >= max_inputs || spent + cost > max_tokens;
        if full && !current.is_empty() {
            requests.push(std::mem::take(&mut current));
            spent = 0;
        }
        current.push(index);
        spent += cost;
    }
    if !current.is_empty() {
        requests.push(current);
    }
    requests
}

// ── Running one document ─────────────────────────────────────────────────────

/// What embeds a request; a trait so packing and concurrency test without a
/// server.
pub trait RequestRun: Send + Sync {
    /// One vector per page, in the order the pages were given. `on_wait`
    /// hears of a rate-limit wait as it starts, and `None` when it ends.
    fn run(
        &self,
        pages: &[RenderedPage],
        on_wait: &dyn Fn(Option<Wait>),
    ) -> Result<Vec<Vec<f32>>, EmbedError>;

    /// The largest request this backend can currently get accepted. A request
    /// over the account's TPM is refused whatever the pace, so this is re-read
    /// on every page and a run shrinks its requests once a 429 teaches the tier.
    fn max_tokens(&self) -> u64 {
        MAX_TOKENS_PER_REQUEST
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_inputs: usize,
    /// The API's hard maximum; packing uses its `min` with
    /// `RequestRun::max_tokens`.
    pub max_tokens: u64,
    pub in_flight: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_inputs: MAX_INPUTS_PER_REQUEST,
            max_tokens: MAX_TOKENS_PER_REQUEST,
            // The gate already paces; each request holds its PNGs in memory.
            in_flight: 4,
        }
    }
}

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
            backend: super::client::BACKEND,
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

    // A page over the pixel ceiling renders at a lower DPI
    // (`raster::dpi_for_page`); `refuse_oversized` catches the rest.
    let rendered = raster::render_pages(pdf, Some(MAX_PIXELS_PER_IMAGE), |page| {
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

    // The boundary guard: pdfium must agree with the parse record's page count
    // (`page_no` is the join key), and every page must have embedded.
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

/// Reconcile the renderer's vocabulary into the seam's. A missing libpdfium
/// is `NotReady`, and `VoyageCloud::health` catches it before the run starts.
impl From<RasterError> for EmbedError {
    fn from(error: RasterError) -> Self {
        match error {
            RasterError::Library(_) => EmbedError::NotReady {
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
            // The message is a pdfium string; only the page number travels.
            RasterError::Page { page_no, .. } => EmbedError::Document {
                code: format!("page-render-failed-p{page_no}"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::EMBED_DIM;
    use crate::ratelimit::Permits;

    /// A landscape A4 slide at `RENDER_DPI`.
    const A4_LANDSCAPE: (u32, u32) = (2339, 1653);

    #[test]
    fn a_full_dpi_page_costs_what_was_measured() {
        let (width, height) = A4_LANDSCAPE;
        assert_eq!(u64::from(width) * u64::from(height), 3_866_367);
        // Capped at 2M billed pixels, rounded up.
        assert_eq!(tokens_for(width, height), 3_572);
        assert_eq!(BILLED_PIXEL_CAP.div_ceil(PIXELS_PER_TOKEN), 3_572);
        assert_eq!(tokens_for(842, 595), 895); // 72 dpi
        assert_eq!(tokens_for(1170, 827), 1_728); // 100 dpi
        assert_eq!(tokens_for(1754, 1240), 3_572); // 150 dpi is already at the cap
        assert_eq!(tokens_for(1, 1), 1);
        assert_eq!(tokens_for(560, 1), 1);
        assert_eq!(tokens_for(561, 1), 2);
        // The cap is on billing, not on the raster.
        let page = RenderedPage {
            page_no: 1,
            width,
            height,
            png: Vec::new(),
        };
        assert_eq!(raw_pixels(&page), 3_866_367);
        assert_eq!(billed_pixels(&page), BILLED_PIXEL_CAP);
    }

    #[test]
    fn a_request_holds_eighty_nine_pages_not_three_hundred_and_twenty() {
        let costs = vec![tokens_for(A4_LANDSCAPE.0, A4_LANDSCAPE.1); 200];
        let requests = plan(&costs, MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST);
        assert_eq!(requests[0].len(), 89);
        assert!(89 * 3_572 <= MAX_TOKENS_PER_REQUEST as usize);
        assert!(90 * 3_572 > MAX_TOKENS_PER_REQUEST as usize);
        let flat: Vec<usize> = requests.iter().flatten().copied().collect();
        assert_eq!(flat, (0..200).collect::<Vec<_>>());
    }

    #[test]
    fn the_batch_is_computed_from_real_pixels_never_a_page_count() {
        // A deck whose second half is cheaper.
        let mut costs = vec![tokens_for(2339, 1653); 100];
        costs.extend(vec![tokens_for(827, 1170); 100]);
        let requests = plan(&costs, MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST);
        assert!(requests.len() >= 2);
        assert!(
            requests.iter().all(
                |request| request.iter().map(|index| costs[*index]).sum::<u64>()
                    <= MAX_TOKENS_PER_REQUEST
            ),
            "a request went over the token ceiling: {requests:?}"
        );
        assert_eq!(requests[0].len(), 89);
        assert!(requests.last().unwrap().len() > 89, "{:?}", requests.last());
    }

    #[test]
    fn the_input_ceiling_binds_when_the_token_one_does_not() {
        let costs = vec![1u64; 2_500];
        let requests = plan(&costs, MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST);
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].len(), MAX_INPUTS_PER_REQUEST);
        assert_eq!(requests[2].len(), 500);
    }

    #[test]
    fn one_page_larger_than_a_whole_request_still_travels_alone() {
        let costs = vec![5, MAX_TOKENS_PER_REQUEST + 1, 5];
        let requests = plan(&costs, MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST);
        assert_eq!(requests, vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn a_free_tier_ceiling_shrinks_the_plan_instead_of_repeating_it() {
        // Over the account's TPM the plan must get smaller, not slower.
        let costs = vec![tokens_for(A4_LANDSCAPE.0, A4_LANDSCAPE.1); 8];
        let free_ceiling = 10_000;

        let optimistic = plan(&costs, MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST);
        assert_eq!(optimistic.len(), 1, "8 pages fit in one tier-1 request");
        assert!(
            optimistic[0].iter().map(|i| costs[*i]).sum::<u64>() > free_ceiling,
            "this is the request that can never be accepted"
        );

        let shrunk = plan(&costs, MAX_INPUTS_PER_REQUEST, free_ceiling);
        assert_eq!(shrunk.len(), 4, "2 pages a request at 3,572 tokens each");
        assert!(
            shrunk
                .iter()
                .all(|r| r.iter().map(|i| costs[*i]).sum::<u64>() <= free_ceiling),
            "{shrunk:?}"
        );
        assert_eq!(shrunk.iter().flatten().count(), 8);
    }

    #[test]
    fn one_page_always_fits_inside_the_smallest_programme_voyage_runs() {
        // Shrinking always terminates: the billing cap keeps any page under
        // the free tier's TPM.
        assert!(
            (BILLED_PIXEL_CAP.div_ceil(PIXELS_PER_TOKEN) as f64) < super::super::ledger::FREE_TPM
        );
    }

    #[test]
    fn an_empty_document_plans_no_requests() {
        assert!(plan(&[], MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST).is_empty());
    }

    #[test]
    fn an_oversized_page_is_this_documents_problem_and_nobody_elses() {
        let page = RenderedPage {
            page_no: 7,
            width: 5_000,
            height: 5_000,
            png: vec![0; 16],
        };
        let error = refuse_oversized(&page).unwrap_err();
        assert_eq!(error.kind(), "document");
        assert!(!error.latching(), "one huge page must not condemn the run");
        assert!(format!("{error:?}").contains("p7"), "{error:?}");

        // Refused on bytes at a legal pixel count.
        let heavy = RenderedPage {
            page_no: 1,
            width: 100,
            height: 100,
            png: vec![0; MAX_BYTES_PER_IMAGE as usize + 1],
        };
        assert!(refuse_oversized(&heavy).is_err());

        let ordinary = RenderedPage {
            page_no: 1,
            width: A4_LANDSCAPE.0,
            height: A4_LANDSCAPE.1,
            png: vec![0; 16],
        };
        assert_eq!(refuse_oversized(&ordinary).unwrap(), 3_572);
    }

    #[test]
    fn the_raster_vocabulary_reconciles_into_the_seams() {
        let library = EmbedError::from(RasterError::Library("no dylib".into()));
        assert_eq!(library.kind(), "not_ready");
        assert!(library.retryable());

        for (raster, code) in [
            (RasterError::Unreadable("junk".into()), "unreadable-pdf"),
            (RasterError::Encrypted, "encrypted-pdf"),
            (RasterError::Empty, "empty-pdf"),
        ] {
            let error = EmbedError::from(raster);
            assert_eq!(error.kind(), "document");
            assert!(format!("{error:?}").contains(code), "{error:?}");
            assert!(!error.latching());
        }

        let page = EmbedError::from(RasterError::Page {
            page_no: 12,
            message: "internal pdfium detail".into(),
        });
        assert!(!format!("{page:?}").contains("pdfium"), "{page:?}");
        assert!(format!("{page:?}").contains("12"), "{page:?}");
    }

    // ── Concurrency, without a renderer ──────────────────────────────────────

    #[test]
    fn permits_bound_what_runs_at_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let permits = Arc::new(Permits::new(2));
        let running = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let permits = permits.clone();
            let running = running.clone();
            let peak = peak.clone();
            handles.push(std::thread::spawn(move || {
                permits.acquire();
                let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(30));
                running.fetch_sub(1, Ordering::SeqCst);
                permits.release();
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        assert!(
            peak.load(Ordering::SeqCst) <= 2,
            "{}",
            peak.load(Ordering::SeqCst)
        );
    }

    #[test]
    fn a_vector_of_the_wrong_width_is_caught_before_it_reaches_a_record() {
        assert!(EmbedPage::new(1, &vec![0.5; EMBED_DIM]).is_ok());
        assert!(EmbedPage::new(1, &vec![0.5; EMBED_DIM - 1]).is_err());
    }
}
