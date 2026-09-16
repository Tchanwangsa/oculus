//! The MinerU cloud protocol, and the `Parser` the app parses through.
//!
//! The shape of a parse is fixed by the API, not by us: a batch of documents
//! is *submitted* as a list of names, MinerU answers with one signed upload URL
//! per name, each file is `PUT` to its URL, and then one endpoint is polled
//! until every task in the batch reports `done` with a result zip. There is no
//! per-file endpoint and no callback, so the whole thing is one long blocking
//! conversation per batch.
//!
//! Three rules run through all of it:
//!
//! * **Errors carry a code, never the server's text.** MinerU's messages can
//!   quote the signed URLs it issued, and those must not reach the UI, a log
//!   or a pasted bug report. Every `ParseError` built here is a short code.
//! * **Failures are scoped.** A malformed result, a `failed` task or a refused
//!   upload condemns *that document*; only credentials, quota and a dead poll
//!   channel condemn the batch. See `Scope`.
//! * **Progress is counted, never inferred.** Per-task page counts are summed;
//!   nothing is derived from page offsets or from how many tasks have finished.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use url::Url;

use crate::parse::{
    check_size, parse_config, Health, ParseError, ParseOutput, ParsePage, Parser, Progress,
    PARSER_VERSION,
};

use super::batch::{BatchRun, Batcher};
use super::ledger::{
    hold, poll_bucket, submit_bucket, TokenBucket, UsageLedger, DAILY_FILES, MAX_FILES_PER_BATCH,
    MAX_FILE_BYTES, MAX_PAGES_PER_TASK,
};
use super::render;

/// The `backend` stamped into every record this client writes.
pub const BACKEND: &str = "mineru-cloud";

/// Attempts per API call. A 429 deliberately does not consume one.
const ATTEMPTS: u32 = 4;
const FIRST_RETRY: Duration = Duration::from_secs(1);
const MAX_RETRY: Duration = Duration::from_secs(10);
const API_TIMEOUT: Duration = Duration::from_secs(30);
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
const UPLOAD_CHUNK: usize = 1024 * 1024;
/// How long a batch may stay unfinished before it is abandoned. This is also
/// the *only* bound on the submit path's 429 loop — see `api_json`.
const POLL_DEADLINE: Duration = Duration::from_secs(60 * 60);
const FIRST_POLL_DELAY: Duration = Duration::from_millis(2_000);
const MAX_POLL_DELAY: Duration = Duration::from_secs(10);

// ── One document in flight ───────────────────────────────────────────────────

/// What a batch worker fills in, and what the blocked caller reads out.
///
/// The caller is parked in `wait` for the whole parse while a worker thread
/// does the work, so everything they share lives behind this one lock. The
/// progress callback is deliberately *not* shared: `Parser::parse` takes a
/// plain `&dyn Fn`, which is neither `Send` nor `'static`, so the worker
/// records numbers and the caller — who owns the callback and is awake anyway
/// — is the thread that calls it.
pub struct CloudDocument {
    pdf: PathBuf,
    images_dir: PathBuf,
    images_rel: String,
    state: Mutex<DocumentState>,
    changed: Condvar,
}

#[derive(Default)]
struct DocumentState {
    total_pages: u32,
    /// data_id → pages MinerU says it has extracted for that task.
    task_pages: HashMap<String, u32>,
    /// The content-list items of every finished task, rebased to absolute
    /// page indices.
    content: Vec<Value>,
    source_images: PathBuf,
    outcome: Option<Result<DocumentOutput, ParseError>>,
}

/// What one document's parse produced, before the seam turns it into a record.
#[derive(Debug)]
pub struct DocumentOutput {
    pub pages: Vec<ParsePage>,
    pub image_count: u32,
    pub total_pages: u32,
}

impl CloudDocument {
    pub fn new(pdf: &Path, images_dir: &Path, images_rel: &str) -> Arc<Self> {
        Arc::new(Self {
            pdf: pdf.to_path_buf(),
            images_dir: images_dir.to_path_buf(),
            images_rel: images_rel.to_string(),
            state: Mutex::new(DocumentState::default()),
            changed: Condvar::new(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.pdf
    }

    /// Park until the batch this document travelled in has an answer for it,
    /// delivering progress on this thread as it changes.
    pub fn wait(&self, on_progress: &dyn Fn(Progress)) -> Result<DocumentOutput, ParseError> {
        let mut state = hold(&self.state);
        // Seeded with the opening zeroes so a document that finishes before
        // anything is known does not emit a pointless 0/0.
        let mut last = (0u32, 0u32);
        loop {
            if let Some(outcome) = state.outcome.take() {
                return outcome;
            }
            let now = (state.pages_done(), state.total_pages);
            if now != last {
                last = now;
                drop(state);
                on_progress(Progress {
                    pages_done: now.0,
                    total_pages: now.1,
                    backend: BACKEND,
                });
                state = hold(&self.state);
                continue;
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// Hand this document its answer. First writer wins: a worker that fails
    /// one document and then panics must not overwrite the real reason.
    pub fn finish(&self, outcome: Result<DocumentOutput, ParseError>) {
        let mut state = hold(&self.state);
        if state.outcome.is_none() {
            state.outcome = Some(outcome);
        }
        drop(state);
        self.changed.notify_all();
    }

    fn set_total_pages(&self, pages: u32) {
        hold(&self.state).total_pages = pages;
        self.changed.notify_all();
    }

    fn total_pages(&self) -> u32 {
        hold(&self.state).total_pages
    }

    fn set_source_images(&self, dir: PathBuf) {
        hold(&self.state).source_images = dir;
    }

    fn source_images(&self) -> PathBuf {
        hold(&self.state).source_images.clone()
    }

    /// Monotonic per task, clamped to that task's own length: MinerU's
    /// `extracted_pages` has been seen to go backwards between polls, and a
    /// progress bar that retreats reads as a stall.
    fn report(&self, data_id: &str, done: u32, page_count: u32) {
        let mut state = hold(&self.state);
        let slot = state.task_pages.entry(data_id.to_string()).or_default();
        *slot = (*slot).max(done.min(page_count));
        drop(state);
        self.changed.notify_all();
    }

    fn push_content(&self, items: Vec<Value>) {
        hold(&self.state).content.extend(items);
    }

    fn take_content(&self) -> Vec<Value> {
        std::mem::take(&mut hold(&self.state).content)
    }
}

impl DocumentState {
    /// The sum over this document's tasks, never more than the document is
    /// long — tasks can only report their own pages, so the clamp is belt to
    /// the per-task brace.
    fn pages_done(&self) -> u32 {
        self.task_pages.values().sum::<u32>().min(self.total_pages)
    }
}

// ── Tasks ────────────────────────────────────────────────────────────────────

/// One extraction task: a page range of one document.
///
/// A document over `MAX_PAGES_PER_TASK` becomes several tasks that each upload
/// **the whole original PDF** and let the server take the range. That is what
/// the API offers — there is no "upload once, extract twice" — so a 600-page
/// file crosses the wire three times. Physically slicing it locally would save
/// the bandwidth and cost a PDF writer, a temp file per slice, and a second
/// page-offset scheme to get wrong.
struct Task {
    document: usize,
    data_id: String,
    upload_name: String,
    source: PathBuf,
    page_offset: u32,
    page_count: u32,
    page_ranges: Option<String>,
}

impl Task {
    fn api_entry(&self) -> Value {
        let mut entry = json!({
            "name": self.upload_name,
            "data_id": self.data_id,
            "is_ocr": false,
        });
        if let Some(ranges) = &self.page_ranges {
            entry["page_ranges"] = Value::String(ranges.clone());
        }
        entry
    }
}

/// Who a failure belongs to.
///
/// The Python caught `BaseException` around a whole batch and gave every job
/// in it the same error, so one unreadable PDF failed the other nineteen. That
/// was survivable only because each of those nineteen then fell back to the
/// local parser. **There is no fallback now**, so the same code would turn one
/// bad file into nineteen files with no markdown at all. Hence this split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Document,
    Batch,
}

/// Credentials, quota and a version mismatch are true of every file in the
/// batch — `ParseError::latching` is exactly that question, so it decides.
fn scope_of(error: &ParseError) -> Scope {
    if error.latching() {
        Scope::Batch
    } else {
        Scope::Document
    }
}

// ── The client ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct MinerUCloud {
    base_url: Arc<String>,
    token: Arc<String>,
    ledger: Arc<UsageLedger>,
    submit: Arc<TokenBucket>,
    poll: Arc<TokenBucket>,
    pages_per_task: u32,
    /// Every wait in this client is multiplied by this. Production is 1.0;
    /// tests shrink it so a retry ladder or a poll loop costs milliseconds
    /// instead of seconds.
    time_scale: f64,
}

impl MinerUCloud {
    /// The client the app uses: engine and API root from the settings row,
    /// token from the keychain.
    pub fn from_config() -> Result<Self, ParseError> {
        let config = parse_config();
        let token = config.credentials.token().unwrap_or_default();
        Self::new(&config.base_url, &token)
    }

    /// `base_url` is always passed in — nothing here knows MinerU's address,
    /// which is what lets the tests point the whole protocol at a local server.
    pub fn new(base_url: &str, token: &str) -> Result<Self, ParseError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(ParseError::MissingCredentials);
        }
        Ok(Self {
            base_url: Arc::new(base_url.trim_end_matches('/').to_string()),
            token: Arc::new(token.to_string()),
            ledger: UsageLedger::shared(),
            submit: submit_bucket(),
            poll: poll_bucket(),
            pages_per_task: MAX_PAGES_PER_TASK,
            time_scale: 1.0,
        })
    }

    pub fn with_ledger(mut self, ledger: Arc<UsageLedger>) -> Self {
        self.ledger = ledger;
        self
    }

    pub fn with_buckets(mut self, submit: Arc<TokenBucket>, poll: Arc<TokenBucket>) -> Self {
        self.submit = submit;
        self.poll = poll;
        self
    }

    pub fn with_pages_per_task(mut self, pages: u32) -> Self {
        self.pages_per_task = pages.max(1);
        self
    }

    pub fn with_time_scale(mut self, scale: f64) -> Self {
        self.time_scale = scale;
        self
    }

    /// Which batch this client's documents may travel in. Only same-token jobs
    /// can share a `POST`, and the API root is part of it so a redirected
    /// client never lands in another one's batch. Hashed rather than
    /// concatenated so no structure that might be printed holds the token.
    pub fn batch_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.base_url.hash(&mut hasher);
        self.token.hash(&mut hasher);
        hasher.finish()
    }

    fn nap(&self, duration: Duration) {
        let scaled = duration.mul_f64(self.time_scale);
        if !scaled.is_zero() {
            std::thread::sleep(scaled);
        }
    }

    // ── Protocol ─────────────────────────────────────────────────────────────

    /// One API call, with the retry policy the whole client shares.
    ///
    /// A 429 sleeps `Retry-After` (default 60, clamped to 1..60 seconds) and
    /// retries **without consuming an attempt**: per-minute pressure is a wait,
    /// not a failure, and there is nothing else to do with the work. That does
    /// mean sustained 429s loop here indefinitely on the submit path; the poll
    /// path's `POLL_DEADLINE` is the only thing that ever bounds it, and that
    /// is accepted rather than accidental.
    fn api_json(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        bucket: &TokenBucket,
    ) -> Result<Value, ParseError> {
        let url = format!("{}{}", self.base_url, path);
        let mut attempt = 0;
        let mut delay = FIRST_RETRY;

        while attempt < ATTEMPTS {
            bucket.acquire();
            let request = ureq::request(method, &url)
                .timeout(API_TIMEOUT)
                .set("Authorization", &format!("Bearer {}", self.token))
                .set("Accept", "application/json");
            // `ureq`'s `json` feature is not enabled in this crate, so the
            // body is encoded here. The `Content-Type` is wanted on *this*
            // request — it is only the signed upload that must go without one.
            let sent = match &body {
                Some(value) => {
                    let encoded = serde_json::to_vec(value).unwrap_or_default();
                    request.set("Content-Type", "application/json").send_bytes(&encoded)
                }
                None => request.call(),
            };

            let response = match sent {
                Ok(response) => response,
                Err(ureq::Error::Status(429, response)) => {
                    let wait = response
                        .header("Retry-After")
                        .and_then(|value| value.trim().parse::<f64>().ok())
                        .unwrap_or(60.0)
                        .clamp(1.0, 60.0);
                    self.nap(Duration::from_secs_f64(wait));
                    continue;
                }
                // A token MinerU refuses cannot be retried into working.
                Err(ureq::Error::Status(401 | 403, response)) => {
                    return Err(auth_error(response.into_string().unwrap_or_default().as_str()))
                }
                Err(ureq::Error::Status(status, _)) => {
                    if status >= 500 && attempt + 1 < ATTEMPTS {
                        backoff(self, &mut attempt, &mut delay);
                        continue;
                    }
                    return Err(ParseError::Document { code: format!("http-{status}") });
                }
                Err(ureq::Error::Transport(transport)) => {
                    if attempt + 1 < ATTEMPTS {
                        backoff(self, &mut attempt, &mut delay);
                        continue;
                    }
                    return Err(ParseError::Offline(transport_detail(&transport)));
                }
            };

            let body_text = response.into_string().unwrap_or_default();
            let payload = match serde_json::from_str::<Value>(&body_text).ok() {
                Some(payload) => payload,
                None if attempt + 1 < ATTEMPTS => {
                    backoff(self, &mut attempt, &mut delay);
                    continue;
                }
                None => return Err(ParseError::Offline("unreadable response".into())),
            };
            if !payload.is_object() {
                return Err(ParseError::Document { code: "invalid-response".into() });
            }

            let code = payload.get("code");
            if code.and_then(Value::as_i64) == Some(0) {
                return Ok(payload.get("data").cloned().unwrap_or_else(|| json!({})));
            }
            if matches!(code.and_then(Value::as_str), Some("A0202" | "A0211")) {
                return Err(auth_error(&payload.to_string()));
            }
            if code.and_then(Value::as_i64) == Some(-60018) {
                // The server's own answer, which outranks the local guess.
                self.ledger.latch_exhausted();
                return Err(ParseError::QuotaExhausted);
            }
            if matches!(code.and_then(Value::as_i64), Some(-60009 | -10001 | -60007))
                && attempt + 1 < ATTEMPTS
            {
                backoff(self, &mut attempt, &mut delay);
                continue;
            }
            // Only the code. The message beside it can quote a signed URL.
            return Err(ParseError::Document { code: safe_code(code) });
        }
        Err(ParseError::Offline("request failed".into()))
    }

    /// `PUT` the file to a signed URL.
    ///
    /// **No `Content-Type`.** MinerU rejects the upload when one is present,
    /// which is why the Python hand-rolled `http.client` rather than use a
    /// normal HTTP library. `ureq` turns out not to need that: it only ever
    /// adds `Host`, `User-Agent` and `Accept` of its own, and a `Content-Type`
    /// appears solely when the caller sets one or uses `send_json`/`send_form`.
    /// Setting `Content-Length` explicitly also keeps it out of chunked
    /// encoding, which a signed PUT would reject.
    ///
    /// No `Authorization` either — the signature in the URL is the auth — and
    /// no retry: a second PUT to a consumed signature is a second failure.
    fn put_file(&self, url: &str, path: &Path) -> Result<(), ParseError> {
        check_transfer_url(url, "upload")?;
        let size = fs::metadata(path)
            .map_err(|e| ParseError::Io(format!("stat {}: {e}", path.display())))?
            .len();
        let file = fs::File::open(path)
            .map_err(|e| ParseError::Io(format!("open {}: {e}", path.display())))?;

        let agent = ureq::AgentBuilder::new()
            .timeout(UPLOAD_TIMEOUT)
            // A signed PUT that redirects has lost its signature; treat the
            // 3xx as the failure it is rather than replaying the body.
            .redirects(0)
            .build();
        match agent
            .put(url)
            .set("Content-Length", &size.to_string())
            .send(BufReader::with_capacity(UPLOAD_CHUNK, file))
        {
            Ok(response) if (200..300).contains(&response.status()) => Ok(()),
            Ok(response) => {
                Err(ParseError::Document { code: format!("upload-http-{}", response.status()) })
            }
            Err(ureq::Error::Status(status, _)) => {
                Err(ParseError::Document { code: format!("upload-http-{status}") })
            }
            Err(ureq::Error::Transport(transport)) => {
                Err(ParseError::Offline(transport_detail(&transport)))
            }
        }
    }

    fn download_zip(&self, url: &str, destination: &Path) -> Result<(), ParseError> {
        check_transfer_url(url, "result")?;
        let mut last = ParseError::Offline("result download failed".into());
        for attempt in 0..3u32 {
            let attempted = ureq::get(url).timeout(DOWNLOAD_TIMEOUT).call().and_then(|response| {
                Ok((response.status(), response.into_reader()))
            });
            match attempted {
                Ok((status, mut reader)) if (200..300).contains(&status) => {
                    let file = fs::File::create(destination).map_err(|e| {
                        ParseError::Io(format!("create {}: {e}", destination.display()))
                    })?;
                    let mut out = BufWriter::with_capacity(UPLOAD_CHUNK, file);
                    return std::io::copy(&mut reader, &mut out)
                        .map(|_| ())
                        .map_err(|e| ParseError::Io(format!("download: {e}")));
                }
                Ok((status, _)) => {
                    last = ParseError::Document { code: format!("result-http-{status}") }
                }
                Err(ureq::Error::Status(status, _)) => {
                    last = ParseError::Document { code: format!("result-http-{status}") }
                }
                Err(ureq::Error::Transport(transport)) => {
                    last = ParseError::Offline(transport_detail(&transport))
                }
            }
            if attempt < 2 {
                self.nap(Duration::from_secs(1 << attempt));
            }
        }
        Err(last)
    }

    // ── Splitting ────────────────────────────────────────────────────────────

    fn build_tasks(&self, index: usize, document: &CloudDocument) -> Result<Vec<Task>, ParseError> {
        let path = document.path().to_path_buf();
        // The Python sliced a >200 MB PDF into physically smaller files and
        // recursed when a slice was still too big. That was the fiddliest part
        // of the client and it existed for a document nobody has: coursework
        // does not reach 200 MB. It is a refusal now, made before anything is
        // uploaded.
        check_size(&path, MAX_FILE_BYTES)?;

        let total = page_count(&path)?;
        document.set_total_pages(total);

        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let suffix = path
            .extension()
            .map(|s| format!(".{}", s.to_string_lossy()))
            .unwrap_or_default();

        let mut tasks = Vec::new();
        let mut start = 0;
        while start < total {
            let end = (start + self.pages_per_task).min(total);
            tasks.push(Task {
                document: index,
                data_id: data_id(),
                // Unique per task, because `file_name` is the fallback key
                // when MinerU echoes a result without its `data_id`.
                upload_name: format!("{stem}__oculus_{}_{end}{suffix}", start + 1),
                source: path.clone(),
                page_offset: start,
                page_count: end - start,
                // Absent for a document that fits in one task: the server
                // treats "no range" as the whole file.
                page_ranges: (total > self.pages_per_task)
                    .then(|| format!("{}-{end}", start + 1)),
            });
            start = end;
        }
        Ok(tasks)
    }

    // ── A batch, end to end ──────────────────────────────────────────────────

    /// Parse every document in one batch, returning one result each, in order.
    pub fn extract_documents(
        &self,
        documents: &[Arc<CloudDocument>],
    ) -> Vec<Result<DocumentOutput, ParseError>> {
        let workspace = match Scratch::new() {
            Ok(workspace) => workspace,
            Err(error) => return documents.iter().map(|_| Err(error.clone())).collect(),
        };

        let mut failures: Vec<Option<ParseError>> = documents.iter().map(|_| None).collect();
        let mut tasks: Vec<Task> = Vec::new();
        for (index, document) in documents.iter().enumerate() {
            document.set_source_images(workspace.path().join(data_id()).join("images"));
            match self.build_tasks(index, document) {
                Ok(built) => tasks.extend(built),
                // Unreadable, empty or oversized: this file only.
                Err(error) => failures[index] = Some(error),
            }
        }

        let mut batch_error = None;
        let mut completed: HashSet<String> = HashSet::new();
        // The whole batch's reservation is checked before any of it is sent,
        // so a batch that cannot fit today does not half-send.
        if let Err(error) = self.ledger.ensure_available(tasks.len() as u64) {
            batch_error = Some(error);
        } else {
            for (chunk, slice) in tasks.chunks(MAX_FILES_PER_BATCH).enumerate() {
                let folder = workspace.path().join(format!("batch-{chunk}"));
                if let Err(error) =
                    self.run_batch(slice, documents, &mut failures, &mut completed, &folder)
                {
                    batch_error = Some(error);
                    break;
                }
            }
        }

        documents
            .iter()
            .enumerate()
            .map(|(index, document)| {
                if let Some(error) = failures[index].take() {
                    return Err(error);
                }
                let outstanding = tasks
                    .iter()
                    .any(|task| task.document == index && !completed.contains(&task.data_id));
                if outstanding {
                    return Err(batch_error
                        .clone()
                        .unwrap_or(ParseError::Document { code: "incomplete".into() }));
                }
                let total = document.total_pages();
                render::render(
                    &document.take_content(),
                    total,
                    &document.source_images(),
                    &document.images_dir,
                    &document.images_rel,
                )
                .map(|(pages, image_count)| DocumentOutput { pages, image_count, total_pages: total })
            })
            .collect()
    }

    /// Submit, upload and poll one chunk of up to `MAX_FILES_PER_BATCH` tasks.
    ///
    /// `Err` is a batch-level failure — every document that still has work
    /// outstanding fails with it. A document-level failure is written into
    /// `failures` and the rest of the chunk carries on.
    fn run_batch(
        &self,
        tasks: &[Task],
        documents: &[Arc<CloudDocument>],
        failures: &mut [Option<ParseError>],
        completed: &mut HashSet<String>,
        workspace: &Path,
    ) -> Result<(), ParseError> {
        fs::create_dir_all(workspace)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", workspace.display())))?;

        // Documents that already failed earlier in this batch are not sent.
        let live: Vec<&Task> =
            tasks.iter().filter(|task| failures[task.document].is_none()).collect();
        if live.is_empty() {
            return Ok(());
        }

        let body = json!({
            "files": live.iter().map(|task| task.api_entry()).collect::<Vec<_>>(),
            // Hardcoded for every document, as in the Python. `pipeline` is the
            // model this app's page records were built with; `language: "ch"`
            // is MinerU's own default and its multilingual model handles the
            // English coursework here, so changing it is a re-parse of the
            // whole library, not a setting.
            "model_version": "pipeline",
            "enable_formula": true,
            "enable_table": true,
            "language": "ch",
        });

        // Reserve *before* the network call. Nothing gives this back: a POST
        // that fails, an upload that dies, a kill halfway through all keep
        // their reservation, because the server may have counted the work and
        // there is no way to ask. See `ledger`.
        self.ledger.record(live.len() as u64, live.iter().map(|t| u64::from(t.page_count)).sum())?;

        let data = self.api_json("POST", "/file-urls/batch", Some(body), &self.submit)?;
        let urls: Vec<&str> = data
            .get("file_urls")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if urls.len() != live.len() {
            return Err(ParseError::Document { code: "upload-url-count".into() });
        }
        let batch_id = data.get("batch_id").and_then(Value::as_str).unwrap_or_default();
        if batch_id.is_empty() {
            return Err(ParseError::Document { code: "missing-batch-id".into() });
        }

        let mut remaining: HashMap<&str, &Task> = HashMap::new();
        for (task, url) in live.iter().copied().zip(urls) {
            // An earlier range of this document already failed, so its signed
            // URL is spent on nothing — but the file is not going to parse.
            if failures[task.document].is_some() {
                continue;
            }
            match self.put_file(url, &task.source) {
                Ok(()) => {
                    remaining.insert(task.data_id.as_str(), task);
                }
                Err(error) if scope_of(&error) == Scope::Batch => return Err(error),
                Err(error) => {
                    failures[task.document] = Some(error);
                    remaining.retain(|_, queued| queued.document != task.document);
                }
            }
        }

        let by_name: HashMap<&str, &Task> =
            live.iter().map(|task| (task.upload_name.as_str(), *task)).collect();
        let deadline = Instant::now() + POLL_DEADLINE.mul_f64(self.time_scale);
        let mut delay = FIRST_POLL_DELAY;
        while !remaining.is_empty() {
            if Instant::now() >= deadline {
                // Not `Document`: a stall is worth retrying later, and the
                // seam's retryable errors are the transport ones.
                return Err(ParseError::Offline("batch timed out after 60 minutes".into()));
            }
            let data = self.api_json(
                "GET",
                &format!("/extract-results/batch/{batch_id}"),
                None,
                &self.poll,
            )?;

            for result in data.get("extract_result").and_then(Value::as_array).cloned().unwrap_or_default() {
                let by_id = result.get("data_id").and_then(Value::as_str);
                let task = by_id
                    .and_then(|id| remaining.get(id).copied())
                    .or_else(|| {
                        result
                            .get("file_name")
                            .and_then(Value::as_str)
                            .and_then(|name| by_name.get(name).copied())
                    });
                let Some(task) = task else { continue };
                if !remaining.contains_key(task.data_id.as_str()) {
                    continue;
                }
                let document = &documents[task.document];
                let state = result.get("state").and_then(Value::as_str).unwrap_or_default();

                if state == "running" {
                    let extracted = result
                        .get("extract_progress")
                        .and_then(|value| value.get("extracted_pages"))
                        .and_then(Value::as_u64)
                        .unwrap_or_default() as u32;
                    document.report(&task.data_id, extracted, task.page_count);
                    continue;
                }
                if state == "failed" {
                    // The file is implied: this error reaches exactly the one
                    // document it belongs to, which is the whole point of the
                    // scoping. The Python named the file because its single
                    // exception was about to be handed to nineteen others.
                    fail_document(failures, &mut remaining, task, ParseError::Document {
                        code: "task-failed".into(),
                    });
                    continue;
                }
                if state != "done" {
                    continue;
                }

                let zip_url = result.get("full_zip_url").and_then(Value::as_str).unwrap_or_default();
                if zip_url.is_empty() {
                    fail_document(failures, &mut remaining, task, ParseError::Document {
                        code: "missing-result".into(),
                    });
                    continue;
                }
                match self.collect_result(task, document, zip_url, workspace) {
                    Ok(()) => {
                        completed.insert(task.data_id.clone());
                        remaining.remove(task.data_id.as_str());
                        // A finished task reports its full length, so the
                        // total is exact even when `extract_progress` never
                        // arrived for it.
                        document.report(&task.data_id, task.page_count, task.page_count);
                    }
                    Err(error) if scope_of(&error) == Scope::Batch => return Err(error),
                    Err(error) => fail_document(failures, &mut remaining, task, error),
                }
            }

            if !remaining.is_empty() {
                self.nap(delay);
                delay = delay.mul_f64(1.4).min(MAX_POLL_DELAY);
            }
        }
        Ok(())
    }

    /// Download one task's result, rebase its page indices onto the document,
    /// and stage the crops it references.
    fn collect_result(
        &self,
        task: &Task,
        document: &CloudDocument,
        zip_url: &str,
        workspace: &Path,
    ) -> Result<(), ParseError> {
        let result_dir = workspace.join(&task.data_id);
        let zip_path = workspace.join(format!("{}.zip", task.data_id));
        self.download_zip(zip_url, &zip_path)?;
        safe_extract(&zip_path, &result_dir)?;

        // Globbed, not named: the archive's layout has changed between MinerU
        // versions, and the flat `.md` beside it is not a substitute — it has
        // no page boundaries at all and drops every `header` item, which on
        // slides is the titles.
        let content_path = find_content_list(&result_dir)
            .ok_or(ParseError::Document { code: "no-content-list".into() })?;
        let content: Value = fs::read_to_string(&content_path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .ok_or(ParseError::Document { code: "unreadable-content-list".into() })?;
        let Some(items) = content.as_array() else {
            return Err(ParseError::Document { code: "invalid-content-list".into() });
        };

        let source_images = content_path.parent().unwrap_or(&result_dir).join("images");
        let staging = document.source_images();
        fs::create_dir_all(&staging)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", staging.display())))?;

        let mut renamed: HashMap<String, String> = HashMap::new();
        let mut rebased = Vec::with_capacity(items.len());
        for item in items {
            let Some(object) = item.as_object() else {
                return Err(ParseError::Document { code: "invalid-content-list".into() });
            };
            let mut absolute = object.clone();

            let page_idx = match object.get("page_idx") {
                None | Some(Value::Null) => 0,
                Some(value) => value
                    .as_i64()
                    .or_else(|| value.as_f64().map(|f| f as i64))
                    .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
                    .ok_or(ParseError::Document { code: "invalid-page-index".into() })?,
            };
            // Hard error rather than a silent drop. `ParseOutput::new` gap-fills
            // anything out of range, so swallowing this would turn a wrong
            // offset into a document that is quietly half empty.
            if page_idx < 0 || page_idx >= i64::from(task.page_count) {
                return Err(ParseError::Document { code: "page-index-out-of-range".into() });
            }
            absolute.insert(
                "page_idx".into(),
                Value::from(page_idx + i64::from(task.page_offset)),
            );

            if let Some(image) = object.get("img_path").and_then(Value::as_str).filter(|p| !p.is_empty())
            {
                let original = basename(image);
                if let Some(target) = renamed.get(&original) {
                    // The same crop cited twice in one task keeps one copy.
                    absolute.insert("img_path".into(), Value::from(format!("images/{target}")));
                    rebased.push(Value::Object(absolute));
                    continue;
                }
                let mut target = original.clone();
                let mut destination = staging.join(&target);
                if destination.exists() {
                    // Two tasks of the same document both named `4.jpg`. The
                    // page offset is the only thing guaranteed to differ.
                    target = format!("p{}_{original}", task.page_offset + 1);
                    destination = staging.join(&target);
                }
                let source = source_images.join(&original);
                // A crop the result named but did not ship leaves `img_path`
                // as it was; `render` skips links with no file behind them.
                if source.is_file() {
                    fs::copy(&source, &destination).map_err(|e| {
                        ParseError::Io(format!("stage {}: {e}", destination.display()))
                    })?;
                    absolute.insert("img_path".into(), Value::from(format!("images/{target}")));
                    renamed.insert(original, target);
                }
            }
            rebased.push(Value::Object(absolute));
        }

        document.push_content(rebased);
        Ok(())
    }
}

fn fail_document(
    failures: &mut [Option<ParseError>],
    remaining: &mut HashMap<&str, &Task>,
    task: &Task,
    error: ParseError,
) {
    failures[task.document] = Some(error);
    // Its other ranges are pointless now, and polling for them would hold the
    // batch open for a document that cannot be assembled.
    remaining.retain(|_, queued| queued.document != task.document);
}

impl BatchRun for MinerUCloud {
    fn run(&self, documents: &[Arc<CloudDocument>]) -> Vec<Result<DocumentOutput, ParseError>> {
        self.extract_documents(documents)
    }
}

impl Parser for MinerUCloud {
    fn parse(
        &self,
        pdf: &Path,
        images_dir: &Path,
        images_rel: &str,
        on_progress: &dyn Fn(Progress),
    ) -> Result<ParseOutput, ParseError> {
        // Refused here rather than in the batch: an oversized file should
        // never take a seat in a batch, and the limit is known from the
        // filesystem alone.
        check_size(pdf, MAX_FILE_BYTES)?;

        let document = CloudDocument::new(pdf, images_dir, images_rel);
        let runner: Arc<dyn BatchRun> = Arc::new(self.clone());
        let output =
            Batcher::shared().submit(self.batch_key(), document, runner, on_progress)?;
        Ok(ParseOutput::new(
            pdf,
            output.total_pages,
            output.pages,
            Some(BACKEND.to_string()),
            output.image_count,
        ))
    }

    /// The cloud is ready whenever it has a token, which `new` guarantees.
    /// Quota is not readiness — an exhausted allowance is a `QuotaExhausted`
    /// on the call that hits it, which says something true about waiting;
    /// `NotReady` would send the UI to "try again in a moment".
    fn health(&self) -> Health {
        Health { backend: BACKEND.to_string(), parser_version: PARSER_VERSION, ready: true }
    }
}

/// How much of the day's allowance is left, for the settings page.
pub fn usage_status() -> Value {
    let usage = UsageLedger::shared().snapshot();
    json!({
        "date": usage.date,
        "files": usage.files,
        "pages": usage.pages,
        "quota_exhausted": usage.quota_exhausted,
        "daily_file_limit": DAILY_FILES,
    })
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// A0211 is a token that has expired; A0202 is one MinerU never accepted.
/// Either way the fix is a new token, so this latches the run.
fn auth_error(body: &str) -> ParseError {
    let code = serde_json::from_str::<Value>(body).ok().and_then(|payload| {
        payload
            .get("msgCode")
            .or_else(|| payload.get("code"))
            .and_then(Value::as_str)
            .map(sanitise_code)
    });
    let expired = code.as_deref() == Some("A0211");
    ParseError::RejectedCredentials { code, expired }
}

/// A `code` field is supposed to be a short token, but it arrives from the
/// network and the seam promises errors carry no server prose — so it is
/// clipped to something that can only ever be a code.
fn sanitise_code(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect()
}

fn safe_code(code: Option<&Value>) -> String {
    match code {
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::String(text)) => sanitise_code(text),
        _ => "unknown".into(),
    }
}

/// One step of the shared retry ladder: 1s, doubling to a 10s ceiling.
fn backoff(client: &MinerUCloud, attempt: &mut u32, delay: &mut Duration) {
    *attempt += 1;
    client.nap(*delay);
    *delay = (*delay * 2).min(MAX_RETRY);
}

pub(super) fn transport_detail(transport: &ureq::Transport) -> String {
    match transport.message() {
        Some(message) => format!("{}: {message}", transport.kind()),
        None => transport.kind().to_string(),
    }
}

/// Signed URLs are always `https`. The loopback exemption exists so the tests
/// can drive the whole protocol against a local server; a loopback address
/// cannot carry a signature off this machine, which is what the rule protects.
fn check_transfer_url(url: &str, what: &str) -> Result<(), ParseError> {
    let parsed =
        Url::parse(url).map_err(|_| ParseError::Document { code: format!("{what}-url-invalid") })?;
    let host = parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or(ParseError::Document { code: format!("{what}-url-invalid") })?;
    let loopback = matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]");
    if parsed.scheme() != "https" && !loopback {
        return Err(ParseError::Document { code: format!("{what}-url-insecure") });
    }
    Ok(())
}

pub(super) fn page_count(pdf: &Path) -> Result<u32, ParseError> {
    let document = lopdf::Document::load(pdf)
        .map_err(|_| ParseError::Document { code: "unreadable-pdf".into() })?;
    let pages = document.get_pages().len() as u32;
    if pages == 0 {
        return Err(ParseError::Document { code: "empty-pdf".into() });
    }
    Ok(pages)
}

fn basename(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

/// A `data_id` only has to be unique inside one batch, and MinerU echoes it
/// back as the join key. The `oculus-` prefix is how a task is recognisable in
/// MinerU's own console.
fn data_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, AtomicOrdering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or_default();
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(nanos);
    hasher.write_u64(sequence);
    hasher.write_u32(std::process::id());
    format!("oculus-{:016x}{:016x}", nanos ^ (sequence << 40), hasher.finish())
}

/// A working directory that deletes itself, for the zips and the staged crops.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new() -> Result<Self, ParseError> {
        let root = std::env::temp_dir().join(format!("mineru-cloud-{}", data_id()));
        fs::create_dir_all(&root)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", root.display())))?;
        Ok(Self { root })
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

/// Extract with a zip-slip guard: every member has to resolve inside the
/// destination. `enclosed_name` already refuses `..` and absolute paths; the
/// prefix check is the second lock on the same door.
pub(super) fn safe_extract(zip_path: &Path, destination: &Path) -> Result<(), ParseError> {
    fs::create_dir_all(destination)
        .map_err(|e| ParseError::Io(format!("create {}: {e}", destination.display())))?;
    let file = fs::File::open(zip_path)
        .map_err(|e| ParseError::Io(format!("open {}: {e}", zip_path.display())))?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|_| ParseError::Document { code: "invalid-result-zip".into() })?;

    for index in 0..archive.len() {
        let mut member = archive
            .by_index(index)
            .map_err(|_| ParseError::Document { code: "invalid-result-zip".into() })?;
        let name = member
            .enclosed_name()
            .ok_or(ParseError::Document { code: "unsafe-zip-path".into() })?;
        let target = destination.join(&name);
        if !target.starts_with(destination) {
            return Err(ParseError::Document { code: "unsafe-zip-path".into() });
        }
        if member.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|e| ParseError::Io(format!("create {}: {e}", target.display())))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ParseError::Io(format!("create {}: {e}", parent.display())))?;
        }
        let mut out = fs::File::create(&target)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", target.display())))?;
        std::io::copy(&mut member, &mut out)
            .map_err(|e| ParseError::Io(format!("extract {}: {e}", target.display())))?;
    }
    Ok(())
}

/// The first `*_content_list.json` anywhere under the extracted result, in a
/// stable order so two runs over the same archive pick the same file.
pub(super) fn find_content_list(root: &Path) -> Option<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .map(|name| name.to_string_lossy().ends_with("_content_list.json"))
                .unwrap_or(false)
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::AtomicBool;

    use super::super::ledger::TokenBucket;

    // ── A MinerU that is not MinerU ──────────────────────────────────────────
    //
    // Every test below drives the real protocol — submit, signed PUT, poll,
    // download, collect — against a `tiny_http` server on loopback. Nothing
    // here ever reaches mineru.net; the base URL is configuration, which is
    // exactly why it is configuration.

    #[derive(Clone)]
    struct Hit {
        method: String,
        url: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl Hit {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(field, _)| field == name)
                .map(|(_, value)| value.as_str())
        }

        fn json(&self) -> Value {
            serde_json::from_slice(&self.body).unwrap_or(Value::Null)
        }
    }

    struct Reply {
        status: u16,
        body: Vec<u8>,
        headers: Vec<(String, String)>,
    }

    impl Reply {
        fn json(value: Value) -> Self {
            Self { status: 200, body: value.to_string().into_bytes(), headers: Vec::new() }
        }

        fn status(status: u16, value: Value) -> Self {
            Self { status, body: value.to_string().into_bytes(), headers: Vec::new() }
        }

        fn bytes(body: Vec<u8>) -> Self {
            Self { status: 200, body, headers: Vec::new() }
        }

        fn with_header(mut self, name: &str, value: &str) -> Self {
            self.headers.push((name.into(), value.into()));
            self
        }
    }

    struct Fake {
        port: u16,
        hits: Arc<Mutex<Vec<Hit>>>,
        stop: Arc<AtomicBool>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl Fake {
        fn start<H>(handler: H) -> Self
        where
            H: Fn(&Hit, usize, u16) -> Reply + Send + 'static,
        {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let port = server.server_addr().to_ip().unwrap().port();
            let hits: Arc<Mutex<Vec<Hit>>> = Arc::new(Mutex::new(Vec::new()));
            let stop = Arc::new(AtomicBool::new(false));

            let handle = {
                let hits = hits.clone();
                let stop = stop.clone();
                std::thread::spawn(move || {
                    while !stop.load(AtomicOrdering::SeqCst) {
                        let Ok(Some(mut request)) =
                            server.recv_timeout(Duration::from_millis(20))
                        else {
                            continue;
                        };
                        let mut body = Vec::new();
                        request.as_reader().read_to_end(&mut body).ok();
                        let hit = Hit {
                            method: request.method().as_str().to_string(),
                            url: request.url().to_string(),
                            headers: request
                                .headers()
                                .iter()
                                .map(|header| {
                                    (
                                        header.field.as_str().to_string().to_lowercase(),
                                        header.value.as_str().to_string(),
                                    )
                                })
                                .collect(),
                            body,
                        };
                        let index = {
                            let mut log = hold(&hits);
                            log.push(hit.clone());
                            log.len() - 1
                        };
                        let reply = handler(&hit, index, port);
                        let mut response = tiny_http::Response::from_data(reply.body)
                            .with_status_code(reply.status);
                        for (name, value) in reply.headers {
                            response.add_header(
                                tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes())
                                    .unwrap(),
                            );
                        }
                        request.respond(response).ok();
                    }
                })
            };
            Self { port, hits, stop, handle: Some(handle) }
        }

        fn origin(&self) -> String {
            format!("http://127.0.0.1:{}", self.port)
        }

        fn base(&self) -> String {
            format!("{}/api/v4", self.origin())
        }

        fn hits(&self) -> Vec<Hit> {
            hold(&self.hits).clone()
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            self.stop.store(true, AtomicOrdering::SeqCst);
            if let Some(handle) = self.handle.take() {
                handle.join().ok();
            }
        }
    }

    // ── Scratch files ────────────────────────────────────────────────────────

    struct Scratch {
        root: PathBuf,
    }

    impl Scratch {
        fn new(name: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            let root = std::env::temp_dir().join(format!("oculus-cloud-{name}-{stamp}"));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn join(&self, name: &str) -> PathBuf {
            self.root.join(name)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).ok();
        }
    }

    /// A real PDF with `pages` empty pages — `build_tasks` counts them with
    /// `lopdf`, so a stub file would not do.
    fn write_pdf(path: &Path, pages: usize) {
        use lopdf::{dictionary, Document, Object};
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let kids: Vec<Object> = (0..pages)
            .map(|_| {
                document
                    .add_object(dictionary! {
                        "Type" => "Page",
                        "Parent" => pages_id,
                    })
                    .into()
            })
            .collect();
        let count = kids.len() as i64;
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => kids,
                "Count" => count,
                "MediaBox" => vec![0.into(), 0.into(), 72.into(), 72.into()],
            }),
        );
        let catalog = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog);
        document.save(path).unwrap();
    }

    fn zip_of(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut archive = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (name, body) in entries {
                archive.start_file(*name, options).unwrap();
                archive.write_all(body).unwrap();
            }
            archive.finish().unwrap();
        }
        buffer.into_inner()
    }

    fn content_list(from: usize, pages: usize) -> Vec<u8> {
        let items: Vec<Value> = (0..pages)
            .map(|page| json!({ "type": "text", "text": format!("page {}", from + page + 1), "page_idx": page }))
            .collect();
        Value::Array(items).to_string().into_bytes()
    }

    fn client(fake: &Fake, ledger: Arc<UsageLedger>) -> MinerUCloud {
        MinerUCloud::new(&fake.base(), "test-only-token")
            .unwrap()
            .with_ledger(ledger)
            // Private buckets so one test cannot pace another, and a scale
            // that turns every retry ladder into milliseconds.
            .with_buckets(
                Arc::new(TokenBucket::new(60_000.0)),
                Arc::new(TokenBucket::new(60_000.0)),
            )
            .with_time_scale(0.005)
    }

    fn ledger(scratch: &Scratch) -> Arc<UsageLedger> {
        Arc::new(UsageLedger::at(scratch.join("mineru-usage.json")))
    }

    // ── The upload ───────────────────────────────────────────────────────────

    #[test]
    fn the_signed_put_carries_a_length_and_no_content_type() {
        let fake = Fake::start(|_, _, _| Reply::bytes(Vec::new()));
        let scratch = Scratch::new("put");
        let pdf = scratch.join("deck.pdf");
        fs::write(&pdf, vec![7u8; 3_000]).unwrap();

        let client = client(&fake, ledger(&scratch));
        client.put_file(&format!("{}/upload/0?signature=private", fake.origin()), &pdf).unwrap();

        let hits = fake.hits();
        assert_eq!(hits.len(), 1);
        let hit = &hits[0];
        assert_eq!(hit.method, "PUT");
        // The one requirement that made the Python hand-roll `http.client`.
        assert_eq!(hit.header("content-type"), None, "{:?}", hit.headers);
        assert_eq!(hit.header("content-length"), Some("3000"));
        // The signature in the URL is the auth; a bearer token here would be
        // a token handed to whatever storage host MinerU happens to use.
        assert_eq!(hit.header("authorization"), None);
        assert_eq!(hit.body.len(), 3_000);
    }

    #[test]
    fn an_upload_url_that_is_not_signed_https_is_refused() {
        assert!(check_transfer_url("http://mineru.net/upload", "upload").is_err());
        // Hostname-less, in the two shapes that reach this: nothing after the
        // scheme, and a scheme that never has an authority at all.
        assert!(check_transfer_url("https://", "upload").is_err());
        assert!(check_transfer_url("file:///etc/passwd", "upload").is_err());
        assert!(check_transfer_url("not a url", "upload").is_err());
        assert!(check_transfer_url("https://oss.example/upload?sig=x", "upload").is_ok());
    }

    // ── Credentials, quota, throttling ───────────────────────────────────────

    #[test]
    fn a_refused_token_is_never_retried() {
        for (code, expired) in [("A0211", true), ("A0202", false)] {
            let fake = Fake::start(move |_, _, _| {
                Reply::status(401, json!({ "msgCode": code, "msg": "user authenticate failed" }))
            });
            let scratch = Scratch::new("auth");
            let client = client(&fake, ledger(&scratch));
            let error = client
                .api_json("GET", "/extract/task/x", None, &client.poll.clone())
                .unwrap_err();

            match error {
                ParseError::RejectedCredentials { code: seen, expired: seen_expired } => {
                    assert_eq!(seen.as_deref(), Some(code));
                    assert_eq!(seen_expired, expired);
                }
                other => panic!("{other:?}"),
            }
            assert_eq!(fake.hits().len(), 1, "a rejected token cannot be retried into working");
        }
    }

    #[test]
    fn the_quota_code_latches_the_ledger() {
        let fake = Fake::start(|_, _, _| Reply::json(json!({ "code": -60018, "msg": "no quota" })));
        let scratch = Scratch::new("quota");
        let book = ledger(&scratch);
        let client = client(&fake, book.clone());

        let error =
            client.api_json("GET", "/extract-results/batch/x", None, &client.poll.clone()).unwrap_err();
        assert!(matches!(error, ParseError::QuotaExhausted));
        assert!(book.snapshot().quota_exhausted);
        // And it survives: a ledger reopened over the same file still refuses,
        // without going near the network.
        let reopened = UsageLedger::at(book.path());
        assert!(matches!(reopened.ensure_available(1), Err(ParseError::QuotaExhausted)));
    }

    #[test]
    fn a_429_waits_and_does_not_spend_an_attempt() {
        let fake = Fake::start(|_, index, _| {
            if index < 6 {
                Reply::status(429, json!({ "msg": "slow down" })).with_header("Retry-After", "1")
            } else {
                Reply::json(json!({ "code": 0, "data": { "batch_id": "late" } }))
            }
        });
        let scratch = Scratch::new("429");
        let client = client(&fake, ledger(&scratch));

        let data = client
            .api_json("GET", "/extract-results/batch/x", None, &client.poll.clone())
            .unwrap();
        assert_eq!(data["batch_id"], "late");
        // Six waits is well past the four attempts a failure gets: throttling
        // is a wait, not a failure.
        assert_eq!(fake.hits().len(), 7);
    }

    #[test]
    fn a_server_error_says_the_code_and_nothing_else() {
        let fake = Fake::start(|_, _, _| {
            Reply::json(json!({
                "code": -60099,
                "msg": "failed: https://oss.example/file?signature=secret-value",
            }))
        });
        let scratch = Scratch::new("codes");
        let client = client(&fake, ledger(&scratch));
        let error = client.api_json("GET", "/x", None, &client.poll.clone()).unwrap_err();

        let shown = error.to_string();
        assert!(shown.contains("-60099"), "{shown}");
        // The rule the whole error vocabulary exists for: MinerU's prose can
        // quote a signed URL, so none of it travels.
        assert!(!shown.contains("signature"), "{shown}");
        assert!(!shown.contains("oss.example"), "{shown}");
    }

    // ── Splitting ────────────────────────────────────────────────────────────

    #[test]
    fn a_long_document_becomes_server_side_page_ranges() {
        let scratch = Scratch::new("split");
        let pdf = scratch.join("long.pdf");
        write_pdf(&pdf, 401);
        let fake = Fake::start(|_, _, _| Reply::bytes(Vec::new()));
        let client = client(&fake, ledger(&scratch));

        let document = CloudDocument::new(&pdf, &scratch.join("out"), "out");
        let tasks = client.build_tasks(0, &document).unwrap();

        assert_eq!(
            tasks
                .iter()
                .map(|task| (task.page_offset, task.page_count, task.page_ranges.clone()))
                .collect::<Vec<_>>(),
            vec![
                (0, 200, Some("1-200".into())),
                (200, 200, Some("201-400".into())),
                (400, 1, Some("401-401".into())),
            ]
        );
        assert_eq!(document.total_pages(), 401);
        // Every task uploads the whole file; the range is the server's job.
        assert!(tasks.iter().all(|task| task.source == pdf));
        assert!(tasks.iter().all(|task| task.api_entry()["is_ocr"] == json!(false)));
        assert!(tasks.iter().all(|task| task.data_id.starts_with("oculus-")));

        // A document that fits in one task carries no range at all.
        let short = scratch.join("short.pdf");
        write_pdf(&short, 3);
        let document = CloudDocument::new(&short, &scratch.join("out"), "out");
        let tasks = client.build_tasks(0, &document).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].page_ranges, None);
        assert_eq!(tasks[0].api_entry().get("page_ranges"), None);
    }

    #[test]
    fn an_oversized_document_is_refused_before_anything_is_sent() {
        let scratch = Scratch::new("big");
        let pdf = scratch.join("huge.pdf");
        write_pdf(&pdf, 1);
        let fake = Fake::start(|_, _, _| Reply::bytes(Vec::new()));
        let client = client(&fake, ledger(&scratch));
        let document = CloudDocument::new(&pdf, &scratch.join("out"), "out");

        // The seam's own refusal, with the real ceiling standing in as a
        // 1-byte one — the Python sliced such a file up instead.
        assert!(matches!(
            check_size(&pdf, 1),
            Err(ParseError::TooLarge { limit_bytes: 1, .. })
        ));
        // Under the real limit the same file builds a task.
        assert_eq!(client.build_tasks(0, &document).unwrap().len(), 1);
    }

    // ── A whole batch ────────────────────────────────────────────────────────

    /// Submit → upload → poll → download → collect, with the two tasks of one
    /// document completing in the wrong order.
    #[test]
    fn a_batch_runs_end_to_end_and_sums_progress_monotonically() {
        let scratch = Scratch::new("e2e");
        let pdf = scratch.join("deck.pdf");
        write_pdf(&pdf, 3);

        let zips = vec![
            zip_of(&[("result/deck_content_list.json", content_list(0, 2))]),
            zip_of(&[("nested/deck_content_list.json", content_list(2, 1))]),
        ];
        let submitted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let polls = Arc::new(AtomicU64::new(0));

        let fake = {
            let submitted = submitted.clone();
            let polls = polls.clone();
            Fake::start(move |hit, _, port| {
                let origin = format!("http://127.0.0.1:{port}");
                if hit.method == "POST" {
                    assert_eq!(hit.url, "/api/v4/file-urls/batch");
                    let body = hit.json();
                    assert_eq!(body["language"], "ch");
                    assert_eq!(body["model_version"], "pipeline");
                    assert_eq!(body["enable_formula"], json!(true));
                    assert_eq!(body["enable_table"], json!(true));
                    let files = body["files"].as_array().unwrap().clone();
                    *hold(&submitted) = files
                        .iter()
                        .map(|file| file["data_id"].as_str().unwrap().to_string())
                        .collect();
                    return Reply::json(json!({
                        "code": 0,
                        "data": {
                            "batch_id": "batch-1",
                            "file_urls": [
                                format!("{origin}/upload/0"),
                                format!("{origin}/upload/1"),
                            ],
                        },
                    }));
                }
                if hit.method == "PUT" {
                    return Reply::bytes(Vec::new());
                }
                if hit.url.starts_with("/result/") {
                    let index: usize =
                        hit.url.trim_start_matches("/result/").trim_end_matches(".zip").parse().unwrap();
                    return Reply::bytes(zips[index].clone());
                }
                assert_eq!(hit.url, "/api/v4/extract-results/batch/batch-1");
                let ids = hold(&submitted).clone();
                let round = polls.fetch_add(1, AtomicOrdering::SeqCst);
                if round == 0 {
                    // The second task finishes first, and the first is still
                    // running: the sum must not go backwards when it lands.
                    return Reply::json(json!({ "code": 0, "data": { "extract_result": [
                        { "data_id": ids[1], "state": "done", "full_zip_url": format!("{origin}/result/1.zip") },
                        { "data_id": ids[0], "state": "running", "extract_progress": { "extracted_pages": 1 } },
                    ]}}));
                }
                Reply::json(json!({ "code": 0, "data": { "extract_result": [
                    { "data_id": ids[0], "state": "done", "full_zip_url": format!("{origin}/result/0.zip") },
                ]}}))
            })
        };

        let book = ledger(&scratch);
        let client = client(&fake, book.clone()).with_pages_per_task(2);
        let document = CloudDocument::new(&pdf, &scratch.join("deck_images"), "deck_images");

        // The batcher's job, done by hand: a worker parses while this thread
        // pumps progress out of `wait`, which is exactly the real split.
        let worker = {
            let client = client.clone();
            let document = document.clone();
            std::thread::spawn(move || {
                let mut results = client.extract_documents(&[document.clone()]);
                document.finish(results.remove(0));
            })
        };
        let seen: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
        let output = document
            .wait(&|progress| hold(&seen).push((progress.pages_done, progress.total_pages)))
            .unwrap();
        worker.join().unwrap();

        assert_eq!(output.total_pages, 3);
        assert_eq!(
            output.pages.iter().map(|page| page.markdown.as_str()).collect::<Vec<_>>(),
            vec!["page 1", "page 2", "page 3"]
        );
        assert_eq!(output.image_count, 0);

        let seen = hold(&seen).clone();
        assert!(!seen.is_empty(), "progress was never reported");
        assert!(
            seen.windows(2).all(|pair| pair[0].0 <= pair[1].0),
            "progress went backwards: {seen:?}"
        );
        assert_eq!(seen.last().copied(), Some((3, 3)), "{seen:?}");
        assert!(seen.iter().all(|(done, _)| *done <= 3));

        // Two tasks, three pages, reserved before the first POST.
        let usage = book.snapshot();
        assert_eq!(usage.files, 2);
        assert_eq!(usage.pages, 3);
    }

    /// The divergence from the Python: one bad document used to fail all
    /// twenty in its batch, which was survivable only while a local parser
    /// stood behind it.
    #[test]
    fn one_bad_document_does_not_take_the_batch_with_it() {
        let scratch = Scratch::new("isolation");
        let mut pdfs = Vec::new();
        for name in ["a.pdf", "b.pdf", "c.pdf"] {
            let path = scratch.join(name);
            write_pdf(&path, 1);
            pdfs.push(path);
        }

        // `a` comes back with a page index outside its own task; `b` fails
        // outright; `c` is fine.
        let broken = Value::Array(vec![json!({ "type": "text", "text": "stray", "page_idx": 9 })])
            .to_string()
            .into_bytes();
        let zips = vec![
            zip_of(&[("r/a_content_list.json", broken)]),
            zip_of(&[("r/c_content_list.json", content_list(0, 1))]),
        ];
        let submitted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let fake = {
            let submitted = submitted.clone();
            Fake::start(move |hit, _, port| {
                let origin = format!("http://127.0.0.1:{port}");
                if hit.method == "POST" {
                    let files = hit.json()["files"].as_array().unwrap().clone();
                    *hold(&submitted) = files
                        .iter()
                        .map(|file| file["data_id"].as_str().unwrap().to_string())
                        .collect();
                    return Reply::json(json!({ "code": 0, "data": {
                        "batch_id": "b",
                        "file_urls": (0..files.len()).map(|i| format!("{origin}/upload/{i}")).collect::<Vec<_>>(),
                    }}));
                }
                if hit.method == "PUT" {
                    return Reply::bytes(Vec::new());
                }
                if hit.url.starts_with("/result/") {
                    let index: usize =
                        hit.url.trim_start_matches("/result/").trim_end_matches(".zip").parse().unwrap();
                    return Reply::bytes(zips[index].clone());
                }
                let ids = hold(&submitted).clone();
                Reply::json(json!({ "code": 0, "data": { "extract_result": [
                    { "data_id": ids[0], "state": "done", "full_zip_url": format!("{origin}/result/0.zip") },
                    { "data_id": ids[1], "state": "failed", "err_msg": "unsupported" },
                    { "data_id": ids[2], "state": "done", "full_zip_url": format!("{origin}/result/1.zip") },
                ]}}))
            })
        };

        let client = client(&fake, ledger(&scratch));
        let documents: Vec<Arc<CloudDocument>> = pdfs
            .iter()
            .map(|pdf| CloudDocument::new(pdf, &scratch.join("images"), "images"))
            .collect();
        let results = client.extract_documents(&documents);

        assert!(
            matches!(&results[0], Err(ParseError::Document { code }) if code == "page-index-out-of-range"),
            "{:?}",
            results[0].as_ref().err().map(|e| e.to_string())
        );
        assert!(
            matches!(&results[1], Err(ParseError::Document { code }) if code == "task-failed"),
            "{:?}",
            results[1].as_ref().err().map(|e| e.to_string())
        );
        let good = results[2].as_ref().expect("the third document was unaffected");
        assert_eq!(good.pages.len(), 1);
        assert_eq!(good.pages[0].markdown, "page 1");
    }

    #[test]
    fn a_rejected_token_fails_the_whole_batch() {
        let scratch = Scratch::new("batch-auth");
        let mut documents = Vec::new();
        for name in ["a.pdf", "b.pdf"] {
            let path = scratch.join(name);
            write_pdf(&path, 1);
            documents.push(CloudDocument::new(&path, &scratch.join("images"), "images"));
        }
        let fake = Fake::start(|_, _, _| Reply::status(403, json!({ "msgCode": "A0202" })));
        let client = client(&fake, ledger(&scratch));

        let results = client.extract_documents(&documents);
        assert_eq!(results.len(), 2);
        for result in &results {
            let error = result.as_ref().err().expect("credentials condemn every file");
            assert!(matches!(error, ParseError::RejectedCredentials { expired: false, .. }));
            assert!(error.latching());
        }
    }

    #[test]
    fn a_failed_submit_still_burns_its_reservation() {
        let scratch = Scratch::new("burn");
        let pdf = scratch.join("a.pdf");
        write_pdf(&pdf, 5);
        let fake = Fake::start(|_, _, _| Reply::status(500, json!({ "msg": "server" })));
        let book = ledger(&scratch);
        let client = client(&fake, book.clone());
        let document = CloudDocument::new(&pdf, &scratch.join("images"), "images");

        let results = client.extract_documents(&[document]);
        assert!(results[0].is_err());
        // Deliberate: the POST may well have been counted server-side, and
        // there is no way to ask. Uncertain failures count against us.
        let usage = book.snapshot();
        assert_eq!(usage.files, 1);
        assert_eq!(usage.pages, 5);
    }

    // ── The result archive ───────────────────────────────────────────────────

    #[test]
    fn a_zip_that_climbs_out_of_its_directory_is_refused() {
        let scratch = Scratch::new("slip");
        let archive = scratch.join("bad.zip");
        fs::write(&archive, zip_of(&[("../escape.txt", b"bad".to_vec())])).unwrap();

        let error = safe_extract(&archive, &scratch.join("out")).unwrap_err();
        assert!(matches!(error, ParseError::Document { .. }), "{error}");
        assert!(!scratch.join("escape.txt").exists());
    }

    #[test]
    fn the_content_list_is_found_by_shape_not_by_name() {
        let scratch = Scratch::new("glob");
        let root = scratch.join("result");
        fs::create_dir_all(root.join("deep/nested")).unwrap();
        // The flat markdown beside it is never the source: it has no page
        // boundaries and drops every header item.
        fs::write(root.join("deck.md"), "# not this").unwrap();
        fs::write(root.join("deep/nested/whatever_content_list.json"), "[]").unwrap();

        assert_eq!(
            find_content_list(&root),
            Some(root.join("deep/nested/whatever_content_list.json"))
        );
        assert_eq!(find_content_list(&scratch.join("missing")), None);
    }
}
