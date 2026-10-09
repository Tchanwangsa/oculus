//! The submission queue: which documents travel in one `POST` together.
//!
//! The API is batch-shaped: one submit and one poll loop per batch, against a
//! per-minute submit budget. A window of `WINDOW` or `MAX_FILES`, whichever
//! comes first, with at most `IN_FLIGHT` batches running. The window opens
//! when the dispatcher finds work, so a lone file waits one window and goes.
//! A file over `SOLO_UPLOAD_BYTES` skips the window and travels alone.

use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::parse::{ParseError, Progress};
use crate::ratelimit::{hold, Permits};

use super::client::{CloudDocument, DocumentOutput};

pub const WINDOW: Duration = Duration::from_secs(5);
pub const MAX_FILES: usize = 20;
pub const IN_FLIGHT: usize = 8;
/// Polling starts only after every PUT of a batch, so a large slow upload
/// would hold its batch-mates' extraction back; above this a file goes alone.
pub const SOLO_UPLOAD_BYTES: u64 = 8 * 1024 * 1024;

/// What parses a batch; a trait so the batching rules test without a server.
pub trait BatchRun: Send + Sync {
    fn run(&self, documents: &[Arc<CloudDocument>]) -> Vec<Result<DocumentOutput, ParseError>>;
}

struct Queued {
    key: u64,
    document: Arc<CloudDocument>,
    runner: Arc<dyn BatchRun>,
}

impl Queued {
    fn solo(&self) -> bool {
        self.document.bytes() > SOLO_UPLOAD_BYTES
    }
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
        SHARED
            .get_or_init(|| Arc::new(Batcher::new(WINDOW, MAX_FILES, IN_FLIGHT)))
            .clone()
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

    /// Queue one document and block until its batch answers. Only jobs with
    /// the same `key` (`MinerUCloud::batch_key`) share a batch.
    pub fn submit(
        self: &Arc<Self>,
        key: u64,
        document: Arc<CloudDocument>,
        runner: Arc<dyn BatchRun>,
        on_progress: &dyn Fn(Progress),
    ) -> Result<DocumentOutput, ParseError> {
        {
            let mut queue = hold(&self.queue);
            queue.waiting.push(Queued {
                key,
                document: document.clone(),
                runner,
            });
            if !queue.dispatching {
                queue.dispatching = true;
                let batcher = self.clone();
                // Started on first use, not at boot.
                if let Err(error) = std::thread::Builder::new()
                    .name("mineru-cloud-batcher".into())
                    .spawn(move || batcher.dispatch())
                {
                    queue.dispatching = false;
                    queue
                        .waiting
                        .retain(|job| !Arc::ptr_eq(&job.document, &document));
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
            // Backpressure: work keeps queueing, nothing else is sent.
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
            queue = self
                .wake
                .wait(queue)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }

        // A large file has nothing to wait for: it goes now, alone.
        if let Some(index) = queue.waiting.iter().position(Queued::solo) {
            return vec![queue.waiting.remove(index)];
        }

        // The window opens now, on the oldest job's key. A large file that
        // arrives meanwhile is not counted, and leaves on the next round.
        let opened = Instant::now();
        let key = queue.waiting[0].key;
        loop {
            let compatible = queue
                .waiting
                .iter()
                .filter(|job| job.key == key && !job.solo())
                .count();
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
            if job.key == key && !job.solo() && batch.len() < self.max_files {
                batch.push(job);
            } else {
                kept.push(job);
            }
        }
        queue.waiting = kept;
        batch
    }
}

/// Run one batch and hand every document its answer. A panic is caught, or
/// every caller would park in `wait` forever; `finish` keeps the first answer.
/// A document skipped while it queued is answered here and never sent.
fn run(batch: Vec<Queued>) {
    let Some(runner) = batch.first().map(|job| job.runner.clone()) else {
        return;
    };
    let mut documents: Vec<Arc<CloudDocument>> = Vec::new();
    for job in &batch {
        if job.document.cancelled() {
            job.document.finish(Err(ParseError::Cancelled));
        } else {
            documents.push(job.document.clone());
        }
    }
    if documents.is_empty() {
        return;
    }

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
                document.finish(Err(ParseError::Io(
                    "the MinerU batch worker panicked".into(),
                )));
            }
        }
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
        /// Each batch's documents, by file size.
        bytes: Mutex<Vec<Vec<u64>>>,
        running: AtomicUsize,
        peak: AtomicUsize,
        hold_for: Duration,
    }

    impl Recorder {
        fn new(hold_for: Duration) -> Arc<Self> {
            Arc::new(Self {
                sizes: Mutex::new(Vec::new()),
                bytes: Mutex::new(Vec::new()),
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
            hold(&self.bytes).push(documents.iter().map(|document| document.bytes()).collect());
            let now = self.running.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(self.hold_for);
            self.running.fetch_sub(1, Ordering::SeqCst);
            documents
                .iter()
                .map(|_| {
                    Ok(DocumentOutput {
                        pages: Vec::new(),
                        image_count: 0,
                        total_pages: 0,
                    })
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
        // A ten-second window nothing waits out: batches close on the count.
        let batcher = Arc::new(Batcher::new(Duration::from_secs(10), 2, 8));
        let started = Instant::now();
        for handle in submit_all(&batcher, recorder.clone(), 1, 4) {
            handle.join().unwrap();
        }
        assert!(
            started.elapsed() < Duration::from_secs(9),
            "{:?}",
            started.elapsed()
        );
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
        assert!(
            sizes.len() >= 2,
            "two tokens cannot travel together: {sizes:?}"
        );
    }

    #[test]
    fn no_more_than_the_permitted_batches_run_at_once() {
        let recorder = Recorder::new(Duration::from_millis(120));
        let batcher = Arc::new(Batcher::new(Duration::from_millis(10), 1, 2));
        for handle in submit_all(&batcher, recorder.clone(), 3, 6) {
            handle.join().unwrap();
        }
        assert_eq!(recorder.sizes().len(), 6);
        assert!(
            recorder.peak.load(Ordering::SeqCst) <= 2,
            "{}",
            recorder.peak.load(Ordering::SeqCst)
        );
    }

    /// Submit one document of `bytes` from its own thread, timing its answer.
    fn submit_sized(
        batcher: &Arc<Batcher>,
        runner: Arc<dyn BatchRun>,
        name: &str,
        bytes: u64,
    ) -> std::thread::JoinHandle<(Result<(), ParseError>, Duration)> {
        let batcher = batcher.clone();
        let path = format!("/library/{name}.pdf");
        std::thread::spawn(move || {
            let started = Instant::now();
            let document =
                CloudDocument::sized(Path::new(&path), Path::new("/tmp/images"), "images", bytes);
            let result = batcher.submit(5, document, runner, &|_| {}).map(|_| ());
            (result, started.elapsed())
        })
    }

    #[test]
    fn a_large_file_never_shares_a_batch() {
        let recorder = Recorder::new(Duration::ZERO);
        let batcher = Arc::new(Batcher::new(Duration::from_millis(400), 20, 8));
        let handles = vec![
            submit_sized(&batcher, recorder.clone(), "small-a", 1_000),
            submit_sized(&batcher, recorder.clone(), "large", SOLO_UPLOAD_BYTES + 1),
            submit_sized(&batcher, recorder.clone(), "small-b", 2_000),
        ];
        for handle in handles {
            handle.join().unwrap().0.unwrap();
        }
        let mut batches = hold(&recorder.bytes).clone();
        batches.iter_mut().for_each(|batch| batch.sort_unstable());
        batches.sort();
        assert_eq!(
            batches,
            vec![vec![1_000, 2_000], vec![SOLO_UPLOAD_BYTES + 1]]
        );
    }

    #[test]
    fn a_large_file_does_not_wait_out_the_window() {
        let recorder = Recorder::new(Duration::ZERO);
        let batcher = Arc::new(Batcher::new(Duration::from_secs(2), 20, 8));
        let (result, took) =
            submit_sized(&batcher, recorder.clone(), "alone", SOLO_UPLOAD_BYTES * 2)
                .join()
                .unwrap();
        result.unwrap();
        assert!(took < Duration::from_secs(1), "{took:?}");
    }

    #[test]
    fn a_document_skipped_while_queued_is_never_run() {
        let recorder = Recorder::new(Duration::ZERO);
        let batcher = Arc::new(Batcher::new(Duration::from_millis(300), 20, 8));
        let skipped = Path::new("/library/batch-skipped-while-queued.pdf");
        crate::parse::Skips::shared().mark(skipped);
        let kept = submit_sized(&batcher, recorder.clone(), "batch-kept", 10);
        let error = batcher
            .submit(
                5,
                document(&skipped.to_string_lossy()),
                recorder.clone(),
                &|_| {},
            )
            .unwrap_err();
        assert!(matches!(error, ParseError::Cancelled), "{error}");
        kept.join().unwrap().0.unwrap();
        crate::parse::Skips::shared().clear(skipped);
        assert_eq!(
            hold(&recorder.bytes).concat(),
            vec![10],
            "only the kept file ran"
        );
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
