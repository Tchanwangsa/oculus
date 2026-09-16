//! The submission queue: which documents travel in one `POST` together.
//!
//! Batching is not an optimisation here, it is how the API is shaped. One
//! submit carries up to fifty tasks and returns one batch id, and the poll
//! endpoint is per batch — so fifty files sent one at a time cost fifty
//! submits and fifty poll loops, against a per-minute budget of fifty
//! requests. Sending them together costs one of each.
//!
//! Against that, a parse queue hands files over one at a time, and a user
//! watching one PDF should not wait for the next nineteen to arrive. The
//! compromise is the Python's and it is unchanged: a short window — **five
//! seconds or twenty files, whichever comes first** — and at most **eight
//! batches in flight**. The window opens when the dispatcher wakes and finds
//! work, not when a file is enqueued, so a lone file at 3am waits five seconds
//! and goes.

use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::parse::{ParseError, Progress};

use super::client::{CloudDocument, DocumentOutput};
use super::ledger::hold;

pub const WINDOW: Duration = Duration::from_secs(5);
pub const MAX_FILES: usize = 20;
pub const IN_FLIGHT: usize = 8;

/// What actually parses a batch. A trait rather than a concrete client so the
/// queue has no opinion about the protocol — and so the batching rules can be
/// tested without a server.
pub trait BatchRun: Send + Sync {
    fn run(&self, documents: &[Arc<CloudDocument>]) -> Vec<Result<DocumentOutput, ParseError>>;
}

struct Queued {
    key: u64,
    document: Arc<CloudDocument>,
    runner: Arc<dyn BatchRun>,
}

pub struct Batcher {
    queue: Mutex<Queue>,
    wake: Condvar,
    window: Duration,
    max_files: usize,
    permits: Permits,
}

#[derive(Default)]
struct Queue {
    waiting: Vec<Queued>,
    dispatching: bool,
}

impl Batcher {
    pub fn shared() -> Arc<Batcher> {
        static SHARED: OnceLock<Arc<Batcher>> = OnceLock::new();
        SHARED.get_or_init(|| Arc::new(Batcher::new(WINDOW, MAX_FILES, IN_FLIGHT))).clone()
    }

    pub fn new(window: Duration, max_files: usize, in_flight: usize) -> Self {
        Self {
            queue: Mutex::new(Queue::default()),
            wake: Condvar::new(),
            window,
            max_files: max_files.max(1),
            permits: Permits::new(in_flight.max(1)),
        }
    }

    /// Queue one document and block until its batch has an answer for it.
    ///
    /// `key` is the client's identity: only jobs that would produce the same
    /// `POST` — same token, same API root — may share a batch.
    pub fn submit(
        self: &Arc<Self>,
        key: u64,
        document: Arc<CloudDocument>,
        runner: Arc<dyn BatchRun>,
        on_progress: &dyn Fn(Progress),
    ) -> Result<DocumentOutput, ParseError> {
        {
            let mut queue = hold(&self.queue);
            queue.waiting.push(Queued { key, document: document.clone(), runner });
            if !queue.dispatching {
                queue.dispatching = true;
                let batcher = self.clone();
                // Started on first use rather than at boot: an app that never
                // parses anything should not carry a parked thread.
                if let Err(error) = std::thread::Builder::new()
                    .name("mineru-cloud-batcher".into())
                    .spawn(move || batcher.dispatch())
                {
                    queue.dispatching = false;
                    queue.waiting.retain(|job| !Arc::ptr_eq(&job.document, &document));
                    return Err(ParseError::Io(format!("start the MinerU batcher: {error}")));
                }
            }
        }
        self.wake.notify_all();
        document.wait(on_progress)
    }

    fn dispatch(self: Arc<Self>) {
        loop {
            let batch = self.next_batch();
            // Blocks the dispatcher once eight batches are running, which is
            // the backpressure: work keeps queueing, nothing else is sent.
            self.permits.acquire();
            let batcher = self.clone();
            if std::thread::Builder::new()
                .name("mineru-cloud-batch".into())
                .spawn(move || {
                    run(batch);
                    batcher.permits.release();
                })
                .is_err()
            {
                self.permits.release();
            }
        }
    }

    fn next_batch(&self) -> Vec<Queued> {
        let mut queue = hold(&self.queue);
        while queue.waiting.is_empty() {
            queue = self.wake.wait(queue).unwrap_or_else(std::sync::PoisonError::into_inner);
        }

        // The window opens now, on the oldest job's key.
        let opened = Instant::now();
        let key = queue.waiting[0].key;
        loop {
            let compatible = queue.waiting.iter().filter(|job| job.key == key).count();
            let remaining = self.window.saturating_sub(opened.elapsed());
            if compatible >= self.max_files || remaining.is_zero() {
                break;
            }
            queue = self
                .wake
                .wait_timeout(queue, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }

        let mut batch = Vec::new();
        let mut kept = Vec::new();
        for job in std::mem::take(&mut queue.waiting) {
            if job.key == key && batch.len() < self.max_files {
                batch.push(job);
            } else {
                kept.push(job);
            }
        }
        queue.waiting = kept;
        batch
    }
}

/// Run one batch and hand every document its answer.
///
/// A panic in the client would otherwise park every caller in `wait` forever,
/// so it is caught and turned into a failure for the documents that do not
/// have one yet. `CloudDocument::finish` keeps the first answer, so a document
/// the client already failed properly keeps its real reason.
fn run(batch: Vec<Queued>) {
    let Some(runner) = batch.first().map(|job| job.runner.clone()) else {
        return;
    };
    let documents: Vec<Arc<CloudDocument>> =
        batch.iter().map(|job| job.document.clone()).collect();

    let results = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runner.run(&documents)));
    match results {
        Ok(results) if results.len() == documents.len() => {
            for (document, result) in documents.iter().zip(results) {
                document.finish(result);
            }
        }
        Ok(_) => {
            for document in &documents {
                document.finish(Err(ParseError::Io(
                    "the MinerU client returned the wrong number of results".into(),
                )));
            }
        }
        Err(_) => {
            for document in &documents {
                document.finish(Err(ParseError::Io("the MinerU batch worker panicked".into())));
            }
        }
    }
}

/// A counting semaphore. `std` has none, and this needs four lines.
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
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Records the shape of every batch it is handed and answers immediately.
    struct Recorder {
        sizes: Mutex<Vec<usize>>,
        running: AtomicUsize,
        peak: AtomicUsize,
        hold_for: Duration,
    }

    impl Recorder {
        fn new(hold_for: Duration) -> Arc<Self> {
            Arc::new(Self {
                sizes: Mutex::new(Vec::new()),
                running: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
                hold_for,
            })
        }

        fn sizes(&self) -> Vec<usize> {
            let mut sizes = hold(&self.sizes).clone();
            sizes.sort_unstable();
            sizes
        }
    }

    impl BatchRun for Recorder {
        fn run(&self, documents: &[Arc<CloudDocument>]) -> Vec<Result<DocumentOutput, ParseError>> {
            hold(&self.sizes).push(documents.len());
            let now = self.running.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(self.hold_for);
            self.running.fetch_sub(1, Ordering::SeqCst);
            documents
                .iter()
                .map(|_| {
                    Ok(DocumentOutput { pages: Vec::new(), image_count: 0, total_pages: 0 })
                })
                .collect()
        }
    }

    fn document(name: &str) -> Arc<CloudDocument> {
        CloudDocument::new(Path::new(name), Path::new("/tmp/images"), "images")
    }

    /// Submit `count` documents under `key` from their own threads, as real
    /// callers do, and wait for all of them.
    fn submit_all(
        batcher: &Arc<Batcher>,
        runner: Arc<dyn BatchRun>,
        key: u64,
        count: usize,
    ) -> Vec<std::thread::JoinHandle<()>> {
        (0..count)
            .map(|index| {
                let batcher = batcher.clone();
                let runner = runner.clone();
                std::thread::spawn(move || {
                    let document = document(&format!("/library/{key}-{index}.pdf"));
                    batcher.submit(key, document, runner, &|_| {}).unwrap();
                })
            })
            .collect()
    }

    #[test]
    fn the_window_gathers_what_arrives_inside_it() {
        let recorder = Recorder::new(Duration::ZERO);
        let batcher = Arc::new(Batcher::new(Duration::from_millis(400), 20, 8));
        let handles = submit_all(&batcher, recorder.clone(), 7, 3);
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(recorder.sizes(), vec![3], "one window, one batch");
    }

    #[test]
    fn a_full_batch_closes_the_window_early() {
        let recorder = Recorder::new(Duration::ZERO);
        // A ten-second window nothing ever waits out: four jobs at two per
        // batch have to close on the count, not the clock. (A fifth job would
        // correctly sit out the whole window on its own — the window is the
        // other half of the rule, not a bug.)
        let batcher = Arc::new(Batcher::new(Duration::from_secs(10), 2, 8));
        let started = Instant::now();
        for handle in submit_all(&batcher, recorder.clone(), 1, 4) {
            handle.join().unwrap();
        }
        assert!(started.elapsed() < Duration::from_secs(9), "{:?}", started.elapsed());
        assert_eq!(recorder.sizes(), vec![2, 2]);
    }

    #[test]
    fn jobs_with_different_keys_never_share_a_batch() {
        let recorder = Recorder::new(Duration::ZERO);
        let batcher = Arc::new(Batcher::new(Duration::from_millis(200), 20, 8));
        let mut handles = submit_all(&batcher, recorder.clone(), 11, 2);
        handles.extend(submit_all(&batcher, recorder.clone(), 22, 2));
        for handle in handles {
            handle.join().unwrap();
        }
        let sizes = recorder.sizes();
        assert_eq!(sizes.iter().sum::<usize>(), 4);
        assert!(sizes.len() >= 2, "two tokens cannot travel together: {sizes:?}");
    }

    #[test]
    fn no_more_than_the_permitted_batches_run_at_once() {
        let recorder = Recorder::new(Duration::from_millis(120));
        let batcher = Arc::new(Batcher::new(Duration::from_millis(10), 1, 2));
        for handle in submit_all(&batcher, recorder.clone(), 3, 6) {
            handle.join().unwrap();
        }
        assert_eq!(recorder.sizes().len(), 6);
        assert!(recorder.peak.load(Ordering::SeqCst) <= 2, "{}", recorder.peak.load(Ordering::SeqCst));
    }

    #[test]
    fn a_panicking_client_does_not_park_its_callers_forever() {
        struct Exploding;
        impl BatchRun for Exploding {
            fn run(&self, _: &[Arc<CloudDocument>]) -> Vec<Result<DocumentOutput, ParseError>> {
                panic!("boom");
            }
        }
        let batcher = Arc::new(Batcher::new(Duration::from_millis(50), 20, 8));
        let error = batcher
            .submit(99, document("/library/x.pdf"), Arc::new(Exploding), &|_| {})
            .unwrap_err();
        assert!(matches!(error, ParseError::Io(_)), "{error}");
    }
}
