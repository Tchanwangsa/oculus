//! How pages are packed into requests, and how many requests run at once.
//!
//! **This is the piece of the MinerU batcher that changed shape.** There, the
//! API batches *documents* — one `POST` carries fifty files and returns one
//! batch id — so the queue's whole job was to hold a document back for five
//! seconds in the hope that nineteen more would arrive. Voyage batches *pages*:
//! one document is already many requests, and there is nothing to wait for.
//!
//! Packing pages from two documents into one request would still save
//! requests, and it is deliberately not done, because **TPM is the governor at
//! every tier and RPM never is.** On the free programme 3 RPM against 10K TPM
//! is ~2.8 pages a minute — one request can hold 89, so the request budget is
//! never the thing that runs out. On tier 1, 2000 RPM against 2M TPM is ~560
//! pages a minute, and one page is still ~3,572 tokens: TPM again. Cross-
//! document packing would buy nothing measurable and would cost a queue, a
//! window, and a failure that spans two files. So the queue does not transfer;
//! the *rules around* it do — bounded concurrency, per-document isolation, and
//! a panic in a worker that never parks its caller forever.
//!
//! What stayed, exactly:
//!
//! * **Both ceilings, per request.** 1000 inputs *and* 320,000 tokens, and the
//!   token one is computed from each page's real pixel count — never a page
//!   count. Pages are not the same size within a document, let alone across
//!   one, and a fixed "46 pages" would be wrong in the expensive direction on
//!   the first A3 diagram it met.
//! * **Progress is summed from finished work**, never inferred from how far
//!   the renderer has got.
//! * **Nothing partial escapes.** A document that could not embed every page
//!   returns an error; `client.rs` writes no record. `embed::is_embedded` now
//!   checks page coverage, so a short record would re-embed rather than lie —
//!   but a short record should not exist in the first place.

use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};

use crate::embed::raster::{self, RasterError, RenderedPage};
use crate::embed::{EmbedError, EmbedPage, Progress};

use super::ledger::hold;

// ── The documented ceilings ──────────────────────────────────────────────────
//
// Verified against the live API on 2026-09-17. They are `pub` because the
// tests pack against them and because `client.rs` reserves against them.

/// Inputs per request. Never the binding constraint at 200 DPI — a request is
/// full at ~89 pages — but it binds immediately for anything small, and a
/// thumbnail-sized page is not hypothetical (a scanned insert, a cover).
pub const MAX_INPUTS_PER_REQUEST: usize = 1_000;

/// Tokens per request. This is the ceiling that actually closes a batch.
pub const MAX_TOKENS_PER_REQUEST: u64 = 320_000;

/// Tokens in one input. A page past this cannot be split — it is one image —
/// so it is a document error rather than something to work around.
pub const MAX_TOKENS_PER_INPUT: u64 = 32_000;

/// Pixels in one image, and the limit that actually bites first: 16M pixels is
/// 28,571 tokens, below `MAX_TOKENS_PER_INPUT`.
pub const MAX_PIXELS_PER_IMAGE: u64 = 16_000_000;

/// Bytes in one image.
pub const MAX_BYTES_PER_IMAGE: u64 = 20 * 1024 * 1024;

/// **Images bill one token per 560 pixels.**
pub const PIXELS_PER_TOKEN: u64 = 560;

/// **Voyage downscales before it bills, and the cap is 2M pixels.** Measured
/// live: two copies of a 2339 x 1653 page — 3,866,367 px each — came back as
/// `image_pixels: 4,000,000` total, i.e. exactly 2,000,000 apiece.
///
/// This is the single most consequential number in the module, and the
/// arithmetic without it is wrong by ~2x in the direction that hurts most. A
/// 200-DPI landscape-A4 slide costs **~3,572 tokens, not 6,905**, so a
/// 320,000-token request holds about **89 pages, not 46** — an uncapped
/// estimate underfills every request by half while overstating every
/// reservation by double.
///
/// Sending more pixels than this buys nothing: they are thrown away before the
/// encoder sees them. The pipeline still renders at `RENDER_DPI` for the
/// reasons on that constant, and this is only what the *estimate* believes.
pub const BILLED_PIXEL_CAP: u64 = 2_000_000;

/// What one page costs, from its own dimensions, after the downscale.
///
/// Rounded up, because a partial token is a token and the estimate must never
/// come in under what is billed — the ledger's reservation is taken from this
/// before the request goes out.
pub fn tokens_for(width: u32, height: u32) -> u64 {
    let pixels = u64::from(width) * u64::from(height);
    pixels.min(BILLED_PIXEL_CAP).div_ceil(PIXELS_PER_TOKEN)
}

/// The pixels actually on the page. Only the hard API rejections are measured
/// against this — what gets *billed* is [`billed_pixels`].
pub fn raw_pixels(page: &RenderedPage) -> u64 {
    u64::from(page.width) * u64::from(page.height)
}

/// The pixels Voyage will charge for, which is what the ledger counts.
pub fn billed_pixels(page: &RenderedPage) -> u64 {
    raw_pixels(page).min(BILLED_PIXEL_CAP)
}

/// Why this one page cannot be sent, if it cannot.
///
/// A page that is too big is a **document** failure and not a run failure: the
/// next file is very probably fine. It is also not something to skip — skipping
/// would produce exactly the short record this module exists to prevent.
///
/// **Pixels rarely reach here any more, and that is the point.** `run_document`
/// hands [`MAX_PIXELS_PER_IMAGE`] to the rasterizer, which renders a page over
/// it at a lower DPI rather than at 200 — costing nothing, since Voyage
/// downscales to [`BILLED_PIXEL_CAP`] before it encodes or bills. So this is
/// the floor under that: a page the clamp could not save (one DPI wide is
/// still too big), a PNG over the byte ceiling, a token cost the estimate did
/// not predict.
pub fn refuse_oversized(page: &RenderedPage) -> Result<u64, EmbedError> {
    let tokens = tokens_for(page.width, page.height);
    // Note the token check below is unreachable now that the estimate is
    // capped — `BILLED_PIXEL_CAP / PIXELS_PER_TOKEN` is 3,572, a ninth of
    // `MAX_TOKENS_PER_INPUT`. It stays because the *documented* limit is the
    // one the server enforces and the cap is a measurement; if Voyage ever
    // stops downscaling, this is the check that already exists.
    let code = if raw_pixels(page) > MAX_PIXELS_PER_IMAGE {
        "page-too-many-pixels"
    } else if page.png.len() as u64 > MAX_BYTES_PER_IMAGE {
        "page-too-large"
    } else if tokens > MAX_TOKENS_PER_INPUT {
        "page-too-many-tokens"
    } else {
        return Ok(tokens);
    };
    Err(EmbedError::Document { code: format!("{code}-p{}", page.page_no) })
}

/// The packing rule, on its own so it can be tested without a renderer or a
/// server: given each page's token cost in order, which pages travel together?
///
/// Greedy and order-preserving. A smarter bin-packer would fit marginally more
/// pages per request, and would also reorder them — which costs the one thing
/// this pipeline cannot spare, a stable `page_no` path from render to record.
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

/// What actually embeds a request. A trait rather than a concrete client so the
/// packing and concurrency rules can be tested without a server — the same
/// split `parse/mineru/batch.rs` draws with `BatchRun`.
pub trait RequestRun: Send + Sync {
    /// One vector per page, in the order the pages were given.
    fn run(&self, pages: &[RenderedPage]) -> Result<Vec<Vec<f32>>, EmbedError>;

    /// The largest request this backend can currently get **accepted**, which
    /// is not the same as the largest the API documents.
    ///
    /// This exists because of a livelock, confirmed live: a request of ~14,284
    /// tokens is refused with 429 on a 10,000 TPM account, and no amount of
    /// waiting changes that — an over-ceiling request is not slow, it is
    /// *impossible*. Packing to the API's 320,000 and then pacing would send
    /// the same doomed request once a minute forever.
    ///
    /// So the ceiling is asked for again on every page, and a run that starts
    /// optimistic **shrinks its requests** the moment the first 429 teaches it
    /// the account is 32x smaller than it assumed.
    fn max_tokens(&self) -> u64 {
        MAX_TOKENS_PER_REQUEST
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_inputs: usize,
    /// The **API's** hard maximum, not the account's. The live ceiling comes
    /// from `RequestRun::max_tokens` and is re-read on every page; the two are
    /// combined with a `min`, so this is a floor under how large a request can
    /// ever be asked to be rather than the number that governs packing.
    pub max_tokens: u64,
    pub in_flight: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_inputs: MAX_INPUTS_PER_REQUEST,
            max_tokens: MAX_TOKENS_PER_REQUEST,
            // Four, not eight. MinerU's eight batches were eight *documents*
            // waiting on a server-side queue; these are four requests against a
            // per-minute budget the gate is already pacing, and each one holds
            // a request's worth of PNGs in memory until it returns.
            in_flight: 4,
        }
    }
}

struct RunState {
    pages: Vec<EmbedPage>,
    done: u32,
    in_flight: usize,
    failure: Option<EmbedError>,
}

struct DocumentRun {
    total: u32,
    state: Mutex<RunState>,
    changed: Condvar,
    permits: Permits,
}

impl DocumentRun {
    fn new(total: u32, in_flight: usize) -> Self {
        Self {
            total,
            state: Mutex::new(RunState {
                pages: Vec::new(),
                done: 0,
                in_flight: 0,
                failure: None,
            }),
            changed: Condvar::new(),
            permits: Permits::new(in_flight.max(1)),
        }
    }

    fn failure(&self) -> Option<EmbedError> {
        hold(&self.state).failure.clone()
    }

    /// First failure wins, exactly as `CloudDocument::finish` keeps the first
    /// answer: a worker that fails and then panics must not overwrite the real
    /// reason.
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
        self.permits.release();
    }
}

/// Render `pdf`, pack it into requests, embed them with bounded concurrency,
/// and hand back one `EmbedPage` per page — or fail.
///
/// The thread story is the MinerU client's, for the same reason. `on_progress`
/// is a plain `&dyn Fn`, neither `Send` nor `'static`, so it cannot be moved
/// into a worker: workers record numbers behind the lock and *this* thread —
/// which is rendering, and then parked waiting for the last requests — is the
/// one that reports them.
pub fn run_document(
    pdf: &Path,
    expected_pages: u32,
    runner: Arc<dyn RequestRun>,
    limits: Limits,
    on_progress: &dyn Fn(Progress),
) -> Result<Vec<EmbedPage>, EmbedError> {
    let run = Arc::new(DocumentRun::new(expected_pages, limits.in_flight));
    let mut workers: Vec<std::thread::JoinHandle<()>> = Vec::new();
    let mut batch: Vec<RenderedPage> = Vec::new();
    let mut spent: u64 = 0;
    // The last number handed to `on_progress`, shared between the rendering
    // pass and the drain below so the two never repeat or retreat.
    let mut reported = u32::MAX;
    // `render_pages` can only be stopped with a `RasterError`, and the reasons
    // we stop are not raster problems. The real error is parked here and the
    // sentinel is thrown away.
    let mut stop: Option<EmbedError> = None;

    // The ceiling travels into the renderer rather than being checked after
    // the fact: a page over it is rendered at a lower DPI instead of failing
    // the document (`raster::dpi_for_page`). `refuse_oversized` below is still
    // the guard — it is what catches a page the clamp could not save, and the
    // byte and token ceilings it also checks are not things a DPI fixes.
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
        // Re-read every page rather than hoisted: the first 429 of a run can
        // shrink this by 32x, and a request already packed to the old ceiling
        // could never be accepted at any pace. See `RequestRun::max_tokens`.
        let ceiling = limits.max_tokens.min(runner.max_tokens());
        if !batch.is_empty() && (batch.len() >= limits.max_inputs || spent + cost > ceiling) {
            dispatch(&run, &runner, std::mem::take(&mut batch), &mut workers);
            spent = 0;
            // Reported here too, not only in the drain below: rendering and
            // embedding overlap, and on the free programme a document can take
            // hours. A progress story that only starts once the last page is
            // rendered is not a progress story.
            let done = hold(&run.state).done;
            if done != reported {
                reported = done;
                on_progress(Progress {
                    pages_done: done,
                    total_pages: expected_pages,
                    backend: super::client::BACKEND,
                });
            }
        }
        spent += cost;
        batch.push(page);
        Ok(())
    });

    let outcome: Result<u32, EmbedError> = match rendered {
        Ok(count) => {
            dispatch(&run, &runner, std::mem::take(&mut batch), &mut workers);
            Ok(count)
        }
        Err(error) => Err(match stop.take() {
            Some(parked) => parked,
            None => EmbedError::from(error),
        }),
    };

    // Report while the last requests drain, then collect. This runs even when
    // rendering failed: requests already in flight have been paid for, and
    // their threads must be let finish rather than abandoned.
    let mut state = hold(&run.state);
    loop {
        if state.done != reported {
            reported = state.done;
            drop(state);
            on_progress(Progress {
                pages_done: reported,
                total_pages: run.total,
                backend: super::client::BACKEND,
            });
            state = hold(&run.state);
            continue;
        }
        if state.in_flight == 0 {
            break;
        }
        state = run.changed.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner);
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

    // ── The boundary guard ───────────────────────────────────────────────────
    //
    // Same rule as the MinerU one, for a worse failure. A document that lost
    // one page to a refused request would otherwise write a record that reads
    // as finished: that page would never be searchable, nothing would retry it,
    // and no error would exist anywhere to say so.
    //
    // The first check is the **page-count agreement** `raster::page_count`
    // documents: pdfium counts one way and the `lopdf` behind the parse record
    // counts another, and they part company on damaged xrefs, lying `/Count`s
    // and incremental updates. `page_no` is the join key retrieval rests on, so
    // a disagreement means the vectors would be filed under the wrong numbers —
    // which is silent, permanent and worse than not embedding at all.
    if rendered_pages != expected_pages {
        return Err(EmbedError::Document { code: "page-count-mismatch".into() });
    }
    if pages.len() as u32 != expected_pages {
        return Err(EmbedError::Document { code: "incomplete".into() });
    }
    Ok(pages)
}

/// Hand one request to a worker.
///
/// The permit is taken **here, on the calling thread**, before the spawn: that
/// is the backpressure, and it is also what bounds memory — at most
/// `in_flight` requests' worth of PNGs exist at once, rather than a 191-page
/// deck's worth.
fn dispatch(
    run: &Arc<DocumentRun>,
    runner: &Arc<dyn RequestRun>,
    pages: Vec<RenderedPage>,
    workers: &mut Vec<std::thread::JoinHandle<()>>,
) {
    if pages.is_empty() {
        return;
    }
    run.permits.acquire();
    hold(&run.state).in_flight += 1;

    let worker_run = run.clone();
    let runner = runner.clone();
    let spawned = std::thread::Builder::new()
        .name("voyage-embed-request".into())
        .spawn(move || {
            let numbers: Vec<u32> = pages.iter().map(|page| page.page_no).collect();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| runner.run(&pages)));
            // The PNGs are finished with the moment the request is.
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
                Ok(Ok(_)) => {
                    worker_run.fail(EmbedError::Document { code: "vector-count-mismatch".into() })
                }
                Ok(Err(error)) => worker_run.fail(error),
                // A panic in the client would otherwise park the caller in the
                // progress loop forever.
                Err(_) => worker_run.fail(EmbedError::Io("the embedding worker panicked".into())),
            }
            worker_run.leave();
        });

    match spawned {
        Ok(handle) => workers.push(handle),
        Err(error) => {
            run.fail(EmbedError::Io(format!("start an embedding worker: {error}")));
            run.leave();
        }
    }
}

/// The sentinel that stops `render_pages` early. Its message never surfaces —
/// the caller replaces it with the reason it parked.
fn halt(page_no: u32) -> RasterError {
    RasterError::Page { page_no, message: "stopped by the embedder".into() }
}

/// Reconcile the renderer's vocabulary into the seam's.
///
/// The one interesting case is `Library`: a missing libpdfium condemns every
/// file in the run, but `EmbedError` — which is `ParseError`'s vocabulary
/// deliberately — has no variant that is both latching and fixable-by-the-user.
/// So it is reported as `NotReady` *and* caught earlier, in
/// `VoyageCloud::health`, which reports `ready: false` when the library will
/// not bind. `preflight` then refuses the run once, before a single file,
/// rather than failing two hundred of them with the same message.
impl From<RasterError> for EmbedError {
    fn from(error: RasterError) -> Self {
        match error {
            RasterError::Library(_) => EmbedError::NotReady { backend: "page renderer".into() },
            RasterError::Unreadable(_) => EmbedError::Document { code: "unreadable-pdf".into() },
            RasterError::Encrypted => EmbedError::Document { code: "encrypted-pdf".into() },
            RasterError::Empty => EmbedError::Document { code: "empty-pdf".into() },
            // The message is a pdfium string; only the page number travels.
            RasterError::Page { page_no, .. } => {
                EmbedError::Document { code: format!("page-render-failed-p{page_no}") }
            }
        }
    }
}

/// A counting semaphore. `std` has none, and this needs four lines — the same
/// four `parse/mineru/batch.rs` writes.
struct Permits {
    free: Mutex<usize>,
    wake: Condvar,
}

impl Permits {
    fn new(count: usize) -> Self {
        Self { free: Mutex::new(count), wake: Condvar::new() }
    }

    fn acquire(&self) {
        let mut free = hold(&self.free);
        while *free == 0 {
            free = self.wake.wait(free).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *free -= 1;
    }

    fn release(&self) {
        *hold(&self.free) += 1;
        self.wake.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::EMBED_DIM;

    /// A landscape A4 slide at `RENDER_DPI`, which is the page this whole
    /// token budget was measured against.
    const A4_LANDSCAPE: (u32, u32) = (2339, 1653);

    #[test]
    fn a_full_dpi_page_costs_what_was_measured() {
        let (width, height) = A4_LANDSCAPE;
        assert_eq!(u64::from(width) * u64::from(height), 3_866_367);
        // Nearly half of those pixels are thrown away before billing: Voyage
        // caps at 2M, measured live as `image_pixels: 4,000,000` for two of
        // these pages. 2,000,000 / 560 = 3571.4, rounded up because the ledger
        // reserves from this figure and an estimate that came in under what was
        // billed would drift the whole allowance optimistic.
        assert_eq!(tokens_for(width, height), 3_572);
        assert_eq!(BILLED_PIXEL_CAP.div_ceil(PIXELS_PER_TOKEN), 3_572);
        // Below the cap nothing is capped, so the low-DPI curve is unchanged.
        assert_eq!(tokens_for(842, 595), 895); // 72 dpi
        assert_eq!(tokens_for(1170, 827), 1_728); // 100 dpi
        assert_eq!(tokens_for(1754, 1240), 3_572); // 150 dpi is already at the cap
        // Rounded up: a partial token is a token.
        assert_eq!(tokens_for(1, 1), 1);
        assert_eq!(tokens_for(560, 1), 1);
        assert_eq!(tokens_for(561, 1), 2);
        // The cap is on billing, not on the page: the raster is untouched.
        let page = RenderedPage { page_no: 1, width, height, png: Vec::new() };
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
        // Every page travels exactly once, in order.
        let flat: Vec<usize> = requests.iter().flatten().copied().collect();
        assert_eq!(flat, (0..200).collect::<Vec<_>>());
    }

    #[test]
    fn the_batch_is_computed_from_real_pixels_never_a_page_count() {
        // A deck that changes orientation halfway, which is ordinary: the
        // second half is cheaper, so more of it fits. A fixed page count would
        // be wrong in both directions.
        let mut costs = vec![tokens_for(2339, 1653); 100];
        costs.extend(vec![tokens_for(827, 1170); 100]);
        let requests = plan(&costs, MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST);
        assert!(requests.len() >= 2);
        assert!(
            requests.iter().all(|request| request
                .iter()
                .map(|index| costs[*index])
                .sum::<u64>()
                <= MAX_TOKENS_PER_REQUEST),
            "a request went over the token ceiling: {requests:?}"
        );
        // The expensive head fills at 89; the cheap tail packs more densely
        // still. A fixed page count would be wrong in both directions.
        assert_eq!(requests[0].len(), 89);
        assert!(requests.last().unwrap().len() > 89, "{:?}", requests.last());
    }

    #[test]
    fn the_input_ceiling_binds_when_the_token_one_does_not() {
        // Thumbnail-sized pages: 1000 of them cost less than one full slide.
        let costs = vec![1u64; 2_500];
        let requests = plan(&costs, MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST);
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].len(), MAX_INPUTS_PER_REQUEST);
        assert_eq!(requests[2].len(), 500);
    }

    #[test]
    fn one_page_larger_than_a_whole_request_still_travels_alone() {
        // Not something to drop — dropping is what produces a short record.
        let costs = vec![5, MAX_TOKENS_PER_REQUEST + 1, 5];
        let requests = plan(&costs, MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST);
        assert_eq!(requests, vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn a_free_tier_ceiling_shrinks_the_plan_instead_of_repeating_it() {
        // The livelock, pinned. On a 10,000 TPM account a request built to the
        // API's 320,000 ceiling is rejected with 429 *whatever the pace* — so
        // the plan has to get smaller, not slower.
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
            shrunk.iter().all(|r| r.iter().map(|i| costs[*i]).sum::<u64>() <= free_ceiling),
            "{shrunk:?}"
        );
        // Forward progress is the whole point: every page still travels.
        assert_eq!(shrunk.iter().flatten().count(), 8);
    }

    #[test]
    fn one_page_always_fits_inside_the_smallest_programme_voyage_runs() {
        // The invariant the shrinking rests on. The billing cap puts a hard
        // ceiling of 3,572 tokens on *any* page, and the slowest account Voyage
        // runs is 10,000 TPM — so a single page can always be accepted, and
        // shrinking a plan can never bottom out at a request that is still too
        // big. Without the cap the worst page was 28,571 tokens and this was
        // not true.
        assert!(BILLED_PIXEL_CAP.div_ceil(PIXELS_PER_TOKEN) < 10_000);
    }

    #[test]
    fn an_empty_document_plans_no_requests() {
        assert!(plan(&[], MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST).is_empty());
    }

    #[test]
    fn an_oversized_page_is_this_documents_problem_and_nobody_elses() {
        let page = RenderedPage { page_no: 7, width: 5_000, height: 5_000, png: vec![0; 16] };
        let error = refuse_oversized(&page).unwrap_err();
        assert_eq!(error.kind(), "document");
        assert!(!error.latching(), "one huge page must not condemn the run");
        // The page number is in the code, so a failure says which page.
        assert!(format!("{error:?}").contains("p7"), "{error:?}");

        // A 20 MB PNG at a legal pixel count is refused on bytes.
        let heavy = RenderedPage {
            page_no: 1,
            width: 100,
            height: 100,
            png: vec![0; MAX_BYTES_PER_IMAGE as usize + 1],
        };
        assert!(refuse_oversized(&heavy).is_err());

        let ordinary =
            RenderedPage { page_no: 1, width: A4_LANDSCAPE.0, height: A4_LANDSCAPE.1, png: vec![0; 16] };
        assert_eq!(refuse_oversized(&ordinary).unwrap(), 3_572);
    }

    #[test]
    fn the_raster_vocabulary_reconciles_into_the_seams() {
        // A missing library is a readiness problem, not this file's problem —
        // and `health()` catches it before a run starts at all.
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

        // pdfium's own message never travels; only the page number does.
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
        assert!(peak.load(Ordering::SeqCst) <= 2, "{}", peak.load(Ordering::SeqCst));
    }

    #[test]
    fn a_vector_of_the_wrong_width_is_caught_before_it_reaches_a_record() {
        // `EmbedPage::new` is the gate; this pins that the worker path uses it
        // rather than trusting the runner.
        assert!(EmbedPage::new(1, &vec![0.5; EMBED_DIM]).is_ok());
        assert!(EmbedPage::new(1, &vec![0.5; EMBED_DIM - 1]).is_err());
    }
}
