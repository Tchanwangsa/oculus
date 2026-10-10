//! The MinerU cloud protocol, and the `Parser` the app parses through.
//!
//! A batch is submitted as a list of names, MinerU answers with one signed
//! upload URL per name, each file is `PUT` to its URL, and one endpoint is
//! polled until every task reports `done` with a result zip.
//!
//! * **Errors carry a code, never the server's text** (see `ParseError`).
//! * **Failures are scoped**: only credentials, quota and a dead poll channel
//!   condemn the batch. See `Scope`.
//! * **Progress is counted**: per-task page counts are summed, never inferred.
//!
//! The two API calls go through `oculus-keyd` when it is installed and the
//! API root is MinerU's own, so this process never holds the token; otherwise
//! straight to the API root with the keychain's token (`with_config`). Both
//! routes hand `api_json` the same `RawResponse`. The signed upload PUT and
//! the result download carry no token and always go direct.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use url::Url;

use crate::credentials::{Credentialed, KeydError, RawResponse};
use crate::parse::{
    check_size, parse_config, CredentialSource, Health, ParseConfig, ParseError, ParseOutput,
    ParsePage, Parser, Phase, Progress, Skips, CLOUD_BASE_URL, PARSER_VERSION,
};
use crate::ratelimit::{hold, nap, transport_detail, Retry, TokenBucket};

use super::batch::{BatchRun, Batcher};
use super::ledger::{
    poll_bucket, submit_bucket, UsageLedger, MAX_FILES_PER_BATCH, MAX_FILE_BYTES,
    MAX_PAGES_PER_TASK,
};
use super::{render, result_tls, WorkDir};

/// The `backend` stamped into every record this client writes.
pub const BACKEND: &str = "mineru-cloud";

/// `CLOUD_BASE_URL`'s path, which keyd's `forward` is given in place of a URL.
const CLOUD_PATH: &str = "/api/v4";

/// Attempts per API call. A 429 deliberately does not consume one.
const ATTEMPTS: u32 = 4;
const API_TIMEOUT: Duration = Duration::from_secs(30);
/// Transfers end on a stall, never on a deadline: a 40 MB PUT over a slow
/// link takes hours, and ureq's overall `timeout` would fail its response read
/// the moment the last byte went up. Result downloads: `result_tls::agent`.
pub(super) const TRANSFER_STALL: Duration = Duration::from_secs(120);
const UPLOAD_CHUNK: usize = 1024 * 1024;
/// How long a batch may stay unfinished. Also the only bound on the 429 loop
/// in `api_json`.
const POLL_DEADLINE: Duration = Duration::from_secs(60 * 60);
const FIRST_POLL_DELAY: Duration = Duration::from_millis(2_000);
const MAX_POLL_DELAY: Duration = Duration::from_secs(10);
/// How often a parked caller re-checks for a skip while nothing changes.
const SKIP_CHECK: Duration = Duration::from_millis(300);
/// At most two `uploading` events a second per file; the last byte always reports.
const UPLOAD_REPORT_EVERY: Duration = Duration::from_millis(500);

// ── One document in flight ───────────────────────────────────────────────────

/// What a batch worker fills in, and what the caller parked in `wait` reads
/// out. The progress callback is not `Send`, so the worker records numbers and
/// the caller's thread calls it.
pub struct CloudDocument {
    pdf: PathBuf,
    images_dir: PathBuf,
    images_rel: String,
    /// The PDF's size when queued: batching and upload order go by it.
    bytes: u64,
    /// Set when the caller stopped waiting on a skip, so the batch drops this
    /// document even after the mark is cleared for a fresh parse.
    abandoned: AtomicBool,
    state: Mutex<DocumentState>,
    changed: Condvar,
}

#[derive(Default)]
struct DocumentState {
    /// `None` until its batch is submitted; nothing is reported before then.
    phase: Option<Phase>,
    /// Across all of this document's PUTs in the current submit.
    bytes_done: u64,
    bytes_total: u64,
    total_pages: u32,
    /// The PDF's page count once read, kept apart from `total_pages` so
    /// counting early reports no progress before the batch runs.
    counted_pages: Option<u32>,
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
        let bytes = fs::metadata(pdf).map(|m| m.len()).unwrap_or(0);
        Self::sized(pdf, images_dir, images_rel, bytes)
    }

    /// `new` with the size given rather than read, for the batching tests.
    pub(super) fn sized(pdf: &Path, images_dir: &Path, images_rel: &str, bytes: u64) -> Arc<Self> {
        Arc::new(Self {
            pdf: pdf.to_path_buf(),
            images_dir: images_dir.to_path_buf(),
            images_rel: images_rel.to_string(),
            bytes,
            abandoned: AtomicBool::new(false),
            state: Mutex::new(DocumentState::default()),
            changed: Condvar::new(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.pdf
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// The user skipped this PDF (`parse::Skips`), or its caller has left.
    pub fn cancelled(&self) -> bool {
        self.abandoned.load(AtomicOrdering::SeqCst) || Skips::shared().is_marked(&self.pdf)
    }

    /// Park until the batch this document travelled in has an answer for it,
    /// delivering progress on this thread as it changes. A skip ends the wait
    /// within `SKIP_CHECK`; the batch drops the document on its own.
    pub fn wait(&self, on_progress: &dyn Fn(Progress)) -> Result<DocumentOutput, ParseError> {
        let mut state = hold(&self.state);
        let mut last: Option<Progress> = None;
        loop {
            if let Some(outcome) = state.outcome.take() {
                return outcome;
            }
            if self.cancelled() {
                self.abandoned.store(true, AtomicOrdering::SeqCst);
                return Err(ParseError::Cancelled);
            }
            if let Some(now) = state.progress() {
                if last.map_or(true, |seen| !same_progress(&seen, &now)) {
                    // Events coalesce here, so an upload this thread last saw
                    // part-way still ends at 100% before processing.
                    let unfinished = last.map_or(true, |seen| match seen.phase {
                        Phase::UploadWait => true,
                        Phase::Uploading => seen.bytes_done < seen.bytes_total,
                        Phase::Processing => false,
                    });
                    last = Some(now);
                    drop(state);
                    if unfinished && now.phase == Phase::Processing && now.bytes_total > 0 {
                        on_progress(Progress {
                            phase: Phase::Uploading,
                            bytes_done: now.bytes_total,
                            ..now
                        });
                    }
                    on_progress(now);
                    state = hold(&self.state);
                    continue;
                }
            }
            state = self
                .changed
                .wait_timeout(state, SKIP_CHECK)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
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

    /// Read the PDF's page count once. The caller's thread does it before
    /// submitting, so a PDF that crashes `lopdf` fails alone, not its batch.
    fn count_pages(&self) -> Result<u32, ParseError> {
        if let Some(pages) = hold(&self.state).counted_pages {
            return Ok(pages);
        }
        let pages = page_count(&self.pdf)?;
        hold(&self.state).counted_pages = Some(pages);
        Ok(pages)
    }

    fn set_total_pages(&self, pages: u32) {
        hold(&self.state).total_pages = pages;
        self.changed.notify_all();
    }

    /// Submitted: this many bytes wait for their turn to upload.
    fn await_upload(&self, bytes_total: u64) {
        let mut state = hold(&self.state);
        state.phase = Some(Phase::UploadWait);
        state.bytes_done = 0;
        state.bytes_total = bytes_total;
        drop(state);
        self.changed.notify_all();
    }

    fn set_phase(&self, phase: Phase) {
        hold(&self.state).phase = Some(phase);
        self.changed.notify_all();
    }

    fn bytes_done(&self) -> u64 {
        hold(&self.state).bytes_done
    }

    fn set_bytes_done(&self, bytes: u64) {
        let mut state = hold(&self.state);
        state.bytes_done = bytes.min(state.bytes_total);
        drop(state);
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

    /// Monotonic per task, clamped to its length: MinerU's `extracted_pages`
    /// can go backwards between polls.
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
    /// The sum over this document's tasks, never more than its length.
    fn pages_done(&self) -> u32 {
        self.task_pages.values().sum::<u32>().min(self.total_pages)
    }

    fn progress(&self) -> Option<Progress> {
        Some(Progress {
            pages_done: self.pages_done(),
            total_pages: self.total_pages,
            backend: BACKEND,
            phase: self.phase?,
            bytes_done: self.bytes_done,
            bytes_total: self.bytes_total,
        })
    }
}

fn same_progress(a: &Progress, b: &Progress) -> bool {
    (
        a.phase,
        a.pages_done,
        a.total_pages,
        a.bytes_done,
        a.bytes_total,
    ) == (
        b.phase,
        b.pages_done,
        b.total_pages,
        b.bytes_done,
        b.bytes_total,
    )
}

// ── Tasks ────────────────────────────────────────────────────────────────────

/// One extraction task: a page range of one document. A document over
/// `MAX_PAGES_PER_TASK` becomes several tasks that each upload the whole PDF
/// and let the server take the range; the API has no "upload once".
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

/// Who a failure belongs to. Nothing falls back, so one bad file must never
/// fail the rest of its batch.
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

/// How an API call gets its token.
#[derive(Clone)]
enum Auth {
    /// This process holds the token: keyd is absent, or the API root is not
    /// MinerU's.
    Direct(Arc<String>),
    /// keyd adds it; the token never enters this process.
    Keyd(Arc<Credentialed>),
}

/// An API call that got no answer. `Transport` backs off and retries like a
/// dropped connection; `Fatal` ends the call.
enum Unanswered {
    Transport(String),
    Fatal(ParseError),
}

impl Unanswered {
    /// keyd's refusals in the seam's vocabulary. `upstream` and a broken
    /// socket are a failure to reach MinerU, so they back off and retry.
    fn from_keyd(error: KeydError) -> Self {
        match error {
            KeydError::Missing(_) | KeydError::NoSession(..) => {
                Unanswered::Fatal(ParseError::MissingCredentials)
            }
            KeydError::Keychain(detail) => {
                Unanswered::Fatal(ParseError::UnreadableCredentials(detail))
            }
            KeydError::Upstream(detail) | KeydError::Broken(detail) => {
                Unanswered::Transport(detail)
            }
            KeydError::Absent => Unanswered::Transport("oculus-keyd stopped listening".into()),
            other @ (KeydError::Request(_) | KeydError::Caller(_) | KeydError::Vault(_)) => {
                Unanswered::Fatal(ParseError::Broker(other.to_string()))
            }
        }
    }
}

#[derive(Clone)]
pub struct MinerUCloud {
    base_url: Arc<String>,
    auth: Auth,
    ledger: Arc<UsageLedger>,
    submit: Arc<TokenBucket>,
    poll: Arc<TokenBucket>,
    pages_per_task: u32,
    /// Every wait is multiplied by this: 1.0 in production, tiny in tests.
    time_scale: f64,
}

impl MinerUCloud {
    /// The client the app uses: engine and API root from the settings row,
    /// token through keyd or from the keychain (`with_config_in`).
    pub fn from_config() -> Result<Self, ParseError> {
        Self::with_config(&parse_config())
    }

    pub fn with_config(config: &ParseConfig) -> Result<Self, ParseError> {
        Self::with_config_in(config, &crate::paths::data_dir(), || {
            config.credentials.token()
        })
    }

    /// keyd when it is installed and `config` names MinerU's own API root, so
    /// keyd's fixed origin is the one meant; otherwise the token from
    /// `direct_token`. Only an absent keyd falls back. keyd's `has` keeps the
    /// early "no token saved" check.
    fn with_config_in(
        config: &ParseConfig,
        data_dir: &Path,
        direct_token: impl FnOnce() -> Result<Option<String>, ParseError>,
    ) -> Result<Self, ParseError> {
        let cloud_root = config.base_url.trim_end_matches('/') == CLOUD_BASE_URL;
        if config.credentials == CredentialSource::Keychain && cloud_root {
            let broker = Credentialed::at(data_dir);
            match broker.has(crate::mineru::SECRET) {
                Ok(true) => return Ok(Self::through_keyd(broker)),
                Ok(false) => return Err(ParseError::MissingCredentials),
                Err(KeydError::Absent) => {}
                Err(error) => {
                    return Err(match Unanswered::from_keyd(error) {
                        Unanswered::Fatal(error) => error,
                        Unanswered::Transport(detail) => ParseError::Broker(detail),
                    })
                }
            }
        }
        let token = direct_token()?.unwrap_or_default();
        Self::new(&config.base_url, &token)
    }

    /// `base_url` is passed in so tests can point the protocol at a local server.
    pub fn new(base_url: &str, token: &str) -> Result<Self, ParseError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(ParseError::MissingCredentials);
        }
        Ok(Self::with_auth(
            base_url,
            Auth::Direct(Arc::new(token.to_string())),
        ))
    }

    fn through_keyd(broker: Credentialed) -> Self {
        Self::with_auth(CLOUD_BASE_URL, Auth::Keyd(Arc::new(broker)))
    }

    fn with_auth(base_url: &str, auth: Auth) -> Self {
        Self {
            base_url: Arc::new(base_url.trim_end_matches('/').to_string()),
            auth,
            ledger: UsageLedger::shared(),
            submit: submit_bucket(),
            poll: poll_bucket(),
            pages_per_task: MAX_PAGES_PER_TASK,
            time_scale: 1.0,
        }
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

    /// Which batch this client's documents may travel in: same token, same API
    /// root. Hashed so nothing printable holds the token. Through keyd the
    /// token is keyd's, so every such client shares one route identity.
    pub fn batch_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.base_url.hash(&mut hasher);
        match &self.auth {
            Auth::Direct(token) => token.hash(&mut hasher),
            Auth::Keyd(_) => "keyd".hash(&mut hasher),
        }
        hasher.finish()
    }

    // ── Protocol ─────────────────────────────────────────────────────────────

    /// One request by this client's route. A status is an answer, whatever
    /// it is; `api_json` reads it.
    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<RawResponse, Unanswered> {
        let mut headers = vec![("Accept", "application/json")];
        if body.is_some() {
            headers.push(("Content-Type", "application/json"));
        }
        match &self.auth {
            Auth::Keyd(broker) => broker
                .send(
                    crate::mineru::SECRET,
                    method,
                    &format!("{CLOUD_PATH}{path}"),
                    &headers,
                    body.unwrap_or_default(),
                    Some(API_TIMEOUT),
                )
                .map_err(Unanswered::from_keyd),
            Auth::Direct(token) => {
                let mut request = ureq::request(method, &format!("{}{}", self.base_url, path))
                    .timeout(API_TIMEOUT)
                    .set("Authorization", &format!("Bearer {token}"));
                for (name, value) in &headers {
                    request = request.set(name, value);
                }
                let sent = match body {
                    Some(body) => request.send_bytes(body),
                    None => request.call(),
                };
                let response = match sent {
                    Ok(response) | Err(ureq::Error::Status(_, response)) => response,
                    Err(ureq::Error::Transport(transport)) => {
                        return Err(Unanswered::Transport(transport_detail(&transport)))
                    }
                };
                let status = response.status();
                let headers = response
                    .headers_names()
                    .into_iter()
                    .filter_map(|name| {
                        let value = response.header(&name)?.to_string();
                        Some((name, value))
                    })
                    .collect();
                let body = response.into_string().unwrap_or_default().into_bytes();
                Ok(RawResponse {
                    status,
                    headers,
                    body,
                    signin: None,
                })
            }
        }
    }

    /// One API call, with the shared retry policy. A 429 sleeps `Retry-After`
    /// (clamped to 1..60s) **without consuming an attempt** — per-minute
    /// pressure is a wait, not a failure — so only `POLL_DEADLINE` bounds it.
    fn api_json(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        bucket: &TokenBucket,
    ) -> Result<Value, ParseError> {
        // `ureq`'s `json` feature is off, so the body is encoded here.
        let encoded = body.map(|value| serde_json::to_vec(&value).unwrap_or_default());
        let mut retry = Retry::new(ATTEMPTS, self.time_scale);

        while retry.attempts_left() {
            bucket.acquire();
            let response = match self.request(method, path, encoded.as_deref()) {
                Ok(response) => response,
                Err(Unanswered::Fatal(error)) => return Err(error),
                Err(Unanswered::Transport(detail)) => {
                    if retry.back_off() {
                        continue;
                    }
                    return Err(ParseError::Offline(detail));
                }
            };

            match response.status {
                429 => {
                    let wait = response
                        .header("Retry-After")
                        .and_then(|value| value.trim().parse::<f64>().ok())
                        .unwrap_or(60.0)
                        .clamp(1.0, 60.0);
                    nap(Duration::from_secs_f64(wait), self.time_scale);
                    continue;
                }
                401 | 403 => return Err(auth_error(&String::from_utf8_lossy(&response.body))),
                status if status >= 400 => {
                    if status >= 500 && retry.back_off() {
                        continue;
                    }
                    return Err(ParseError::Document {
                        code: format!("http-{status}"),
                    });
                }
                _ => {}
            }

            let payload = match serde_json::from_slice::<Value>(&response.body).ok() {
                Some(payload) => payload,
                None if retry.back_off() => continue,
                None => return Err(ParseError::Offline("unreadable response".into())),
            };
            if !payload.is_object() {
                return Err(ParseError::Document {
                    code: "invalid-response".into(),
                });
            }

            let code = payload.get("code");
            if code.and_then(Value::as_i64) == Some(0) {
                return Ok(payload.get("data").cloned().unwrap_or_else(|| json!({})));
            }
            if matches!(code.and_then(Value::as_str), Some("A0202" | "A0211")) {
                return Err(auth_error(&payload.to_string()));
            }
            if code.and_then(Value::as_i64) == Some(-60018) {
                self.ledger.latch_exhausted();
                return Err(ParseError::QuotaExhausted);
            }
            if matches!(code.and_then(Value::as_i64), Some(-60009 | -10001 | -60007))
                && retry.back_off()
            {
                continue;
            }
            // Only the code. The message beside it can quote a signed URL.
            return Err(ParseError::Document {
                code: safe_code(code),
            });
        }
        Err(ParseError::Offline("request failed".into()))
    }

    /// `PUT` the file to a signed URL. **No `Content-Type`**: MinerU rejects
    /// the upload when one is present (`ureq` adds none unless asked). An
    /// explicit `Content-Length` avoids chunked encoding, which a signed PUT
    /// rejects. No `Authorization` and no retry: the signature is single-use.
    /// `report` gets the bytes sent so far; once `cancelled` holds, the body
    /// stops reading and the PUT ends as `Cancelled`.
    fn put_file(
        &self,
        url: &str,
        path: &Path,
        report: &dyn Fn(u64),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), ParseError> {
        check_transfer_url(url, "upload")?;
        let size = fs::metadata(path)
            .map_err(|e| ParseError::Io(format!("stat {}: {e}", path.display())))?
            .len();
        let file = fs::File::open(path)
            .map_err(|e| ParseError::Io(format!("open {}: {e}", path.display())))?;

        let agent = ureq::AgentBuilder::new()
            .timeout_connect(TRANSFER_STALL)
            .timeout_read(TRANSFER_STALL)
            .timeout_write(TRANSFER_STALL)
            // A redirected signed PUT has lost its signature.
            .redirects(0)
            .build();
        let body = UploadBody::new(
            BufReader::with_capacity(UPLOAD_CHUNK, file),
            size,
            UPLOAD_REPORT_EVERY,
            report,
            cancelled,
        );
        match agent
            .put(url)
            .set("Content-Length", &size.to_string())
            .send(body)
        {
            Err(_) if cancelled() => Err(ParseError::Cancelled),
            Ok(response) if (200..300).contains(&response.status()) => Ok(()),
            Ok(response) => Err(ParseError::Document {
                code: format!("upload-http-{}", response.status()),
            }),
            Err(ureq::Error::Status(status, _)) => Err(ParseError::Document {
                code: format!("upload-http-{status}"),
            }),
            Err(ureq::Error::Transport(transport)) => {
                Err(ParseError::Offline(transport_detail(&transport)))
            }
        }
    }

    fn download_zip(&self, url: &str, destination: &Path) -> Result<(), ParseError> {
        check_transfer_url(url, "result")?;
        let mut last = ParseError::Offline("result download failed".into());
        for attempt in 0..3u32 {
            let agent = result_tls::agent(parse_config().accept_expired_result_cert);
            let attempted = agent
                .get(url)
                .call()
                .and_then(|response| Ok((response.status(), response.into_reader())));
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
                    last = ParseError::Document {
                        code: format!("result-http-{status}"),
                    }
                }
                Err(ureq::Error::Status(status, _)) => {
                    last = ParseError::Document {
                        code: format!("result-http-{status}"),
                    }
                }
                Err(ureq::Error::Transport(transport)) => {
                    last = ParseError::Offline(transport_detail(&transport))
                }
            }
            if attempt < 2 {
                nap(Duration::from_secs(1 << attempt), self.time_scale);
            }
        }
        Err(last)
    }

    // ── Splitting ────────────────────────────────────────────────────────────

    fn build_tasks(&self, index: usize, document: &CloudDocument) -> Result<Vec<Task>, ParseError> {
        let path = document.path().to_path_buf();
        check_size(&path, MAX_FILE_BYTES)?;

        let total = document.count_pages()?;
        document.set_total_pages(total);

        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
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
                page_ranges: (total > self.pages_per_task).then(|| format!("{}-{end}", start + 1)),
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
        // For the zips and the staged crops.
        let workspace = match WorkDir::new(format!("mineru-cloud-{}", data_id())) {
            Ok(workspace) => workspace,
            Err(error) => return documents.iter().map(|_| Err(error.clone())).collect(),
        };

        let mut failures: Vec<Option<ParseError>> = documents.iter().map(|_| None).collect();
        let mut tasks: Vec<Task> = Vec::new();
        for (index, document) in documents.iter().enumerate() {
            if document.cancelled() {
                failures[index] = Some(ParseError::Cancelled);
                continue;
            }
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
                // Before `render`, which would write into a staging directory
                // its abandoned caller has already removed.
                if document.cancelled() {
                    return Err(ParseError::Cancelled);
                }
                if let Some(error) = failures[index].take() {
                    return Err(error);
                }
                let outstanding = tasks
                    .iter()
                    .any(|task| task.document == index && !completed.contains(&task.data_id));
                if outstanding {
                    return Err(batch_error.clone().unwrap_or(ParseError::Document {
                        code: "incomplete".into(),
                    }));
                }
                let total = document.total_pages();
                render::render(
                    &document.take_content(),
                    total,
                    &document.source_images(),
                    &document.images_dir,
                    &document.images_rel,
                )
                .map(|(pages, image_count)| DocumentOutput {
                    pages,
                    image_count,
                    total_pages: total,
                })
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

        // A skipped document is dropped before anything is reserved or sent.
        for task in tasks {
            if failures[task.document].is_none() && documents[task.document].cancelled() {
                failures[task.document] = Some(ParseError::Cancelled);
            }
        }
        // Documents that already failed earlier in this batch are not sent.
        let live: Vec<&Task> = tasks
            .iter()
            .filter(|task| failures[task.document].is_none())
            .collect();
        if live.is_empty() {
            return Ok(());
        }

        let body = json!({
            "files": live.iter().map(|task| task.api_entry()).collect::<Vec<_>>(),
            // Fixed: the library's records were built with these, so changing
            // one is a re-parse, not a setting. `"ch"` is MinerU's multilingual
            // default.
            "model_version": "pipeline",
            "enable_formula": true,
            "enable_table": true,
            "language": "ch",
        });

        // Reserved before the call and never given back: the server may have
        // counted the work. See `ledger`.
        self.ledger.record(
            live.len() as u64,
            live.iter().map(|t| u64::from(t.page_count)).sum(),
        )?;

        let data = self.api_json("POST", "/file-urls/batch", Some(body), &self.submit)?;
        let urls: Vec<&str> = data
            .get("file_urls")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if urls.len() != live.len() {
            return Err(ParseError::Document {
                code: "upload-url-count".into(),
            });
        }
        let batch_id = data
            .get("batch_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        // It goes into the poll's path, and keyd refuses a path with anything
        // else in it. Only this batch's documents fail; the next batch runs.
        if !is_batch_id(batch_id) {
            for task in &live {
                failures[task.document].get_or_insert(ParseError::Document {
                    code: "batch-id".into(),
                });
            }
            return Ok(());
        }

        // Smallest file first: extraction starts only once every PUT is in,
        // so the small ones must not queue behind a slow large upload.
        let mut uploads: Vec<(&Task, &str)> = live.iter().copied().zip(urls).collect();
        uploads.sort_by_key(|(task, _)| documents[task.document].bytes());
        // A document over several tasks uploads its whole file once per task.
        let mut puts_left: HashMap<usize, u32> = HashMap::new();
        for (task, _) in &uploads {
            *puts_left.entry(task.document).or_default() += 1;
        }
        for (&index, &puts) in &puts_left {
            documents[index].await_upload(documents[index].bytes() * u64::from(puts));
        }

        let mut remaining: HashMap<&str, &Task> = HashMap::new();
        for (task, url) in uploads {
            if failures[task.document].is_some() {
                continue;
            }
            let document = &documents[task.document];
            if document.cancelled() {
                fail_document(failures, &mut remaining, task, ParseError::Cancelled);
                continue;
            }
            document.set_phase(Phase::Uploading);
            let before = document.bytes_done();
            let uploaded = self.put_file(
                url,
                &task.source,
                &|sent| document.set_bytes_done(before + sent),
                &|| document.cancelled(),
            );
            match uploaded {
                Ok(()) => {
                    remaining.insert(task.data_id.as_str(), task);
                    let left = puts_left.entry(task.document).or_default();
                    *left = left.saturating_sub(1);
                    if *left == 0 {
                        document.set_phase(Phase::Processing);
                    }
                }
                Err(error) if scope_of(&error) == Scope::Batch => return Err(error),
                Err(error) => fail_document(failures, &mut remaining, task, error),
            }
        }

        let by_name: HashMap<&str, &Task> = live
            .iter()
            .map(|task| (task.upload_name.as_str(), *task))
            .collect();
        let deadline = Instant::now() + POLL_DEADLINE.mul_f64(self.time_scale);
        let mut delay = FIRST_POLL_DELAY;
        loop {
            // A document skipped mid-poll is dropped; its result is never fetched.
            let skipped: Vec<&Task> = remaining
                .values()
                .copied()
                .filter(|task| documents[task.document].cancelled())
                .collect();
            for task in skipped {
                fail_document(failures, &mut remaining, task, ParseError::Cancelled);
            }
            if remaining.is_empty() {
                break;
            }
            if Instant::now() >= deadline {
                // Not `Document`: a stall is worth retrying later.
                return Err(ParseError::Offline(
                    "batch timed out after 60 minutes".into(),
                ));
            }
            let data = self.api_json(
                "GET",
                &format!("/extract-results/batch/{batch_id}"),
                None,
                &self.poll,
            )?;

            for result in data
                .get("extract_result")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
            {
                let by_id = result.get("data_id").and_then(Value::as_str);
                let task = by_id.and_then(|id| remaining.get(id).copied()).or_else(|| {
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
                if document.cancelled() {
                    fail_document(failures, &mut remaining, task, ParseError::Cancelled);
                    continue;
                }
                let state = result
                    .get("state")
                    .and_then(Value::as_str)
                    .unwrap_or_default();

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
                    fail_document(
                        failures,
                        &mut remaining,
                        task,
                        ParseError::Document {
                            code: "task-failed".into(),
                        },
                    );
                    continue;
                }
                if state != "done" {
                    continue;
                }

                let zip_url = result
                    .get("full_zip_url")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if zip_url.is_empty() {
                    fail_document(
                        failures,
                        &mut remaining,
                        task,
                        ParseError::Document {
                            code: "missing-result".into(),
                        },
                    );
                    continue;
                }
                match self.collect_result(task, document, zip_url, workspace) {
                    Ok(()) => {
                        completed.insert(task.data_id.clone());
                        remaining.remove(task.data_id.as_str());
                        // Full length, even if `extract_progress` never came.
                        document.report(&task.data_id, task.page_count, task.page_count);
                    }
                    Err(error) if scope_of(&error) == Scope::Batch => return Err(error),
                    Err(error) => fail_document(failures, &mut remaining, task, error),
                }
            }

            if !remaining.is_empty() {
                nap(delay, self.time_scale);
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

        let (content_path, items) = super::content_list(&result_dir)?;

        let source_images = content_path.parent().unwrap_or(&result_dir).join("images");
        let staging = document.source_images();
        fs::create_dir_all(&staging)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", staging.display())))?;

        let mut renamed: HashMap<String, String> = HashMap::new();
        let mut rebased = Vec::with_capacity(items.len());
        for item in &items {
            let Some(object) = item.as_object() else {
                return Err(ParseError::Document {
                    code: "invalid-content-list".into(),
                });
            };
            let mut absolute = object.clone();

            let page_idx = match object.get("page_idx") {
                None | Some(Value::Null) => 0,
                Some(value) => value
                    .as_i64()
                    .or_else(|| value.as_f64().map(|f| f as i64))
                    .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
                    .ok_or(ParseError::Document {
                        code: "invalid-page-index".into(),
                    })?,
            };
            // Hard error: `ParseOutput::new` would gap-fill a wrong offset
            // into a quietly half-empty document.
            if page_idx < 0 || page_idx >= i64::from(task.page_count) {
                return Err(ParseError::Document {
                    code: "page-index-out-of-range".into(),
                });
            }
            absolute.insert(
                "page_idx".into(),
                Value::from(page_idx + i64::from(task.page_offset)),
            );

            if let Some(image) = object
                .get("img_path")
                .and_then(Value::as_str)
                .filter(|p| !p.is_empty())
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
                    // Two tasks both named `4.jpg`; the page offset differs.
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

/// A PUT body that counts what `ureq` has read, for the `uploading` phase,
/// and fails the next read once the file is skipped, which aborts the request.
struct UploadBody<'a, R> {
    inner: R,
    sent: u64,
    total: u64,
    every: Duration,
    reported: Option<(u64, Instant)>,
    report: &'a dyn Fn(u64),
    cancelled: &'a dyn Fn() -> bool,
}

impl<'a, R: Read> UploadBody<'a, R> {
    fn new(
        inner: R,
        total: u64,
        every: Duration,
        report: &'a dyn Fn(u64),
        cancelled: &'a dyn Fn() -> bool,
    ) -> Self {
        Self {
            inner,
            sent: 0,
            total,
            every,
            reported: None,
            report,
            cancelled,
        }
    }
}

impl<R: Read> Read for UploadBody<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if (self.cancelled)() {
            // Not `Interrupted`, which `io::copy` would retry.
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "upload skipped",
            ));
        }
        let read = self.inner.read(buf)?;
        self.sent += read as u64;
        let sent = self.sent.min(self.total);
        let finished = read == 0 || sent >= self.total;
        let due = match self.reported {
            None => true,
            Some((bytes, at)) => bytes != sent && (finished || at.elapsed() >= self.every),
        };
        if due {
            self.reported = Some((sent, Instant::now()));
            (self.report)(sent);
        }
        Ok(read)
    }
}

fn fail_document(
    failures: &mut [Option<ParseError>],
    remaining: &mut HashMap<&str, &Task>,
    task: &Task,
    error: ParseError,
) {
    failures[task.document] = Some(error);
    // Its other ranges are pointless now; stop polling for them.
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
        // Before the batch, so an oversized file never takes a seat.
        check_size(pdf, MAX_FILE_BYTES)?;

        let document = CloudDocument::new(pdf, images_dir, images_rel);
        document.count_pages()?;
        let runner: Arc<dyn BatchRun> = Arc::new(self.clone());
        let output = Batcher::shared().submit(self.batch_key(), document, runner, on_progress)?;
        Ok(ParseOutput::new(
            pdf,
            output.total_pages,
            output.pages,
            Some(BACKEND.to_string()),
            output.image_count,
        ))
    }

    /// Ready whenever it has a token, which construction checked (keyd's
    /// `has`, or the keychain's token). Quota is not
    /// readiness: it surfaces as `QuotaExhausted` on the call that hits it.
    fn health(&self) -> Health {
        Health {
            backend: BACKEND.to_string(),
            parser_version: PARSER_VERSION,
            ready: true,
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// A0211 is an expired token; A0202 one MinerU never accepted.
fn auth_error(body: &str) -> ParseError {
    let code = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|payload| {
            payload
                .get("msgCode")
                .or_else(|| payload.get("code"))
                .and_then(Value::as_str)
                .map(sanitise_code)
        });
    let expired = code.as_deref() == Some("A0211");
    ParseError::RejectedCredentials { code, expired }
}

/// A batch id that can sit in a path as it is: non-empty, unreserved
/// characters only.
fn is_batch_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._~-".contains(&b))
}

/// Clip a network-supplied `code` to something that can only be a code.
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

/// Signed URLs must be `https`; loopback is exempt so tests can drive the
/// protocol, and cannot carry a signature off this machine.
fn check_transfer_url(url: &str, what: &str) -> Result<(), ParseError> {
    let parsed = Url::parse(url).map_err(|_| ParseError::Document {
        code: format!("{what}-url-invalid"),
    })?;
    let host = parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or(ParseError::Document {
            code: format!("{what}-url-invalid"),
        })?;
    let loopback = matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]");
    if parsed.scheme() != "https" && !loopback {
        return Err(ParseError::Document {
            code: format!("{what}-url-insecure"),
        });
    }
    Ok(())
}

pub(super) fn page_count(pdf: &Path) -> Result<u32, ParseError> {
    let document = lopdf::Document::load(pdf).map_err(|_| ParseError::Document {
        code: "unreadable-pdf".into(),
    })?;
    let pages = document.get_pages().len() as u32;
    if pages == 0 {
        return Err(ParseError::Document {
            code: "empty-pdf".into(),
        });
    }
    Ok(pages)
}

fn basename(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

/// Unique within a batch; MinerU echoes it back as the join key.
fn data_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, AtomicOrdering::Relaxed);
    let nanos = crate::clock::now_nanos() as u64;
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(nanos);
    hasher.write_u64(sequence);
    hasher.write_u32(std::process::id());
    format!(
        "oculus-{:016x}{:016x}",
        nanos ^ (sequence << 40),
        hasher.finish()
    )
}

/// Extract with a zip-slip guard: every member must resolve inside the
/// destination.
pub(super) fn safe_extract(zip_path: &Path, destination: &Path) -> Result<(), ParseError> {
    fs::create_dir_all(destination)
        .map_err(|e| ParseError::Io(format!("create {}: {e}", destination.display())))?;
    let file = fs::File::open(zip_path)
        .map_err(|e| ParseError::Io(format!("open {}: {e}", zip_path.display())))?;
    let mut archive =
        zip::ZipArchive::new(BufReader::new(file)).map_err(|_| ParseError::Document {
            code: "invalid-result-zip".into(),
        })?;

    for index in 0..archive.len() {
        let mut member = archive.by_index(index).map_err(|_| ParseError::Document {
            code: "invalid-result-zip".into(),
        })?;
        let name = member.enclosed_name().ok_or(ParseError::Document {
            code: "unsafe-zip-path".into(),
        })?;
        let target = destination.join(&name);
        if !target.starts_with(destination) {
            return Err(ParseError::Document {
                code: "unsafe-zip-path".into(),
            });
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

/// The first `*_content_list.json` under the extracted result, in stable order.
pub(super) fn find_content_list(root: &Path) -> Option<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
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

    use crate::test_support::{write_pdf, FakeKeyd, FakeServer, Reply, Scratch};

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

    fn client(fake: &FakeServer, ledger: Arc<UsageLedger>) -> MinerUCloud {
        MinerUCloud::new(&format!("{}/api/v4", fake.origin()), "test-only-token")
            .unwrap()
            .with_ledger(ledger)
            // Private buckets so one test cannot pace another.
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
        let fake = FakeServer::start(|_| Reply::bytes(Vec::new()));
        let scratch = Scratch::new("cloud-put");
        let pdf = scratch.join("deck.pdf");
        fs::write(&pdf, vec![7u8; 3_000]).unwrap();

        let client = client(&fake, ledger(&scratch));
        let reported = Mutex::new(Vec::new());
        client
            .put_file(
                &format!("{}/upload/0?signature=private", fake.origin()),
                &pdf,
                &|sent| hold(&reported).push(sent),
                &|| false,
            )
            .unwrap();
        assert_eq!(
            hold(&reported).last().copied(),
            Some(3_000),
            "the last byte always reports"
        );

        let hits = fake.hits();
        assert_eq!(hits.len(), 1);
        let hit = &hits[0];
        assert_eq!(hit.method, "PUT");
        assert_eq!(hit.header("content-type"), None, "{:?}", hit.headers);
        assert_eq!(hit.header("content-length"), Some("3000"));
        // A bearer token here would go to whatever storage host MinerU uses.
        assert_eq!(hit.header("authorization"), None);
        assert_eq!(hit.body.len(), 3_000);
    }

    #[test]
    fn the_upload_body_reports_at_most_every_interval_and_always_the_end() {
        let reported = Mutex::new(Vec::new());
        let report = |sent| hold(&reported).push(sent);
        let mut body = UploadBody::new(
            std::io::Cursor::new(vec![1u8; 10_000]),
            10_000,
            Duration::from_secs(3600),
            &report,
            &|| false,
        );
        let mut chunk = [0u8; 1_000];
        while body.read(&mut chunk).unwrap() > 0 {}
        // The first read, then nothing until the last byte.
        assert_eq!(*hold(&reported), vec![1_000, 10_000]);
    }

    #[test]
    fn a_skip_fails_the_upload_body_on_its_next_read() {
        let skipped = AtomicBool::new(false);
        let cancelled = || skipped.load(AtomicOrdering::SeqCst);
        let mut body = UploadBody::new(
            std::io::Cursor::new(vec![1u8; 4_000]),
            4_000,
            Duration::ZERO,
            &|_| {},
            &cancelled,
        );
        let mut chunk = [0u8; 1_000];
        assert_eq!(body.read(&mut chunk).unwrap(), 1_000);
        skipped.store(true, AtomicOrdering::SeqCst);
        let error = body.read(&mut chunk).unwrap_err();
        // `Interrupted` would be retried by `io::copy` instead of ending the PUT.
        assert_ne!(error.kind(), std::io::ErrorKind::Interrupted);
    }

    #[test]
    fn a_skip_ends_the_wait_and_outlives_its_mark() {
        let scratch = Scratch::new("cloud-wait-skip");
        let pdf = scratch.join("waiting.pdf");
        let document = CloudDocument::new(&pdf, &scratch.join("images"), "images");
        let marker = {
            let pdf = pdf.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                Skips::shared().mark(&pdf);
            })
        };
        let started = Instant::now();
        let error = document.wait(&|_| {}).unwrap_err();
        marker.join().unwrap();
        assert!(matches!(error, ParseError::Cancelled), "{error}");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        // A re-parse clears the mark; this abandoned document stays dropped.
        Skips::shared().clear(&pdf);
        assert!(document.cancelled());
    }

    #[test]
    fn an_upload_url_that_is_not_signed_https_is_refused() {
        assert!(check_transfer_url("http://mineru.net/upload", "upload").is_err());
        // Hostname-less, in both shapes.
        assert!(check_transfer_url("https://", "upload").is_err());
        assert!(check_transfer_url("file:///etc/passwd", "upload").is_err());
        assert!(check_transfer_url("not a url", "upload").is_err());
        assert!(check_transfer_url("https://oss.example/upload?sig=x", "upload").is_ok());
    }

    // ── Credentials, quota, throttling ───────────────────────────────────────

    #[test]
    fn a_refused_token_is_never_retried() {
        for (code, expired) in [("A0211", true), ("A0202", false)] {
            let fake = FakeServer::start(move |_| {
                Reply::status(
                    401,
                    json!({ "msgCode": code, "msg": "user authenticate failed" }),
                )
            });
            let scratch = Scratch::new("cloud-auth");
            let client = client(&fake, ledger(&scratch));
            let error = client
                .api_json("GET", "/extract/task/x", None, &client.poll.clone())
                .unwrap_err();

            match error {
                ParseError::RejectedCredentials {
                    code: seen,
                    expired: seen_expired,
                } => {
                    assert_eq!(seen.as_deref(), Some(code));
                    assert_eq!(seen_expired, expired);
                }
                other => panic!("{other:?}"),
            }
            assert_eq!(
                fake.hits().len(),
                1,
                "a rejected token cannot be retried into working"
            );
        }
    }

    #[test]
    fn the_quota_code_latches_the_ledger() {
        let fake = FakeServer::start(|_| Reply::json(json!({ "code": -60018, "msg": "no quota" })));
        let scratch = Scratch::new("cloud-quota");
        let book = ledger(&scratch);
        let client = client(&fake, book.clone());

        let error = client
            .api_json(
                "GET",
                "/extract-results/batch/x",
                None,
                &client.poll.clone(),
            )
            .unwrap_err();
        assert!(matches!(error, ParseError::QuotaExhausted));
        assert!(book.snapshot().quota_exhausted);
        // A ledger reopened over the same file still refuses, offline.
        let reopened = UsageLedger::at(book.path());
        assert!(matches!(
            reopened.ensure_available(1),
            Err(ParseError::QuotaExhausted)
        ));
    }

    #[test]
    fn a_429_waits_and_does_not_spend_an_attempt() {
        let fake = FakeServer::start(|hit| {
            if hit.index < 6 {
                Reply::status(429, json!({ "msg": "slow down" })).with_header("Retry-After", "1")
            } else {
                Reply::json(json!({ "code": 0, "data": { "batch_id": "late" } }))
            }
        });
        let scratch = Scratch::new("cloud-429");
        let client = client(&fake, ledger(&scratch));

        let data = client
            .api_json(
                "GET",
                "/extract-results/batch/x",
                None,
                &client.poll.clone(),
            )
            .unwrap();
        assert_eq!(data["batch_id"], "late");
        // Six waits, past the four attempts a failure gets.
        assert_eq!(fake.hits().len(), 7);
    }

    #[test]
    fn a_server_error_says_the_code_and_nothing_else() {
        let fake = FakeServer::start(|_| {
            Reply::json(json!({
                "code": -60099,
                "msg": "failed: https://oss.example/file?signature=secret-value",
            }))
        });
        let scratch = Scratch::new("cloud-codes");
        let client = client(&fake, ledger(&scratch));
        let error = client
            .api_json("GET", "/x", None, &client.poll.clone())
            .unwrap_err();

        let shown = error.to_string();
        assert!(shown.contains("-60099"), "{shown}");
        assert!(!shown.contains("signature"), "{shown}");
        assert!(!shown.contains("oss.example"), "{shown}");
    }

    // ── Splitting ────────────────────────────────────────────────────────────

    #[test]
    fn a_long_document_becomes_server_side_page_ranges() {
        let scratch = Scratch::new("cloud-split");
        let pdf = scratch.join("long.pdf");
        write_pdf(&pdf, 401);
        let fake = FakeServer::start(|_| Reply::bytes(Vec::new()));
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
        assert!(tasks
            .iter()
            .all(|task| task.api_entry()["is_ocr"] == json!(false)));
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
        let scratch = Scratch::new("cloud-big");
        let pdf = scratch.join("huge.pdf");
        write_pdf(&pdf, 1);
        let fake = FakeServer::start(|_| Reply::bytes(Vec::new()));
        let client = client(&fake, ledger(&scratch));
        let document = CloudDocument::new(&pdf, &scratch.join("out"), "out");

        // A 1-byte ceiling stands in for the real one.
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
        let scratch = Scratch::new("cloud-e2e");
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
            FakeServer::start(move |hit| {
                let origin = hit.origin.clone();
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
                    let index: usize = hit
                        .url
                        .trim_start_matches("/result/")
                        .trim_end_matches(".zip")
                        .parse()
                        .unwrap();
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

        // The batcher's split, by hand: a worker parses, this thread `wait`s.
        let worker = {
            let client = client.clone();
            let document = document.clone();
            std::thread::spawn(move || {
                let mut results = client.extract_documents(&[document.clone()]);
                document.finish(results.remove(0));
            })
        };
        let seen: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
        let phases: Mutex<Vec<Progress>> = Mutex::new(Vec::new());
        let output = document
            .wait(&|progress| {
                hold(&seen).push((progress.pages_done, progress.total_pages));
                hold(&phases).push(progress);
            })
            .unwrap();
        worker.join().unwrap();

        // Phases only move forward, and the upload ends at 100% — the whole
        // file once per task — before processing, however events coalesced.
        let phases = hold(&phases).clone();
        let order = |phase| match phase {
            Phase::UploadWait => 0,
            Phase::Uploading => 1,
            Phase::Processing => 2,
        };
        assert!(phases
            .windows(2)
            .all(|pair| order(pair[0].phase) <= order(pair[1].phase)));
        let size = fs::metadata(&pdf).unwrap().len();
        assert!(
            phases.iter().any(|p| p.phase == Phase::Uploading
                && p.bytes_done == 2 * size
                && p.bytes_total == 2 * size),
            "{phases:?}"
        );
        assert_eq!(phases.last().map(|p| p.phase), Some(Phase::Processing));

        assert_eq!(output.total_pages, 3);
        assert_eq!(
            output
                .pages
                .iter()
                .map(|page| page.markdown.as_str())
                .collect::<Vec<_>>(),
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

    /// One bad document must not fail the rest of its batch.
    #[test]
    fn one_bad_document_does_not_take_the_batch_with_it() {
        let scratch = Scratch::new("cloud-isolation");
        let mut pdfs = Vec::new();
        for name in ["a.pdf", "b.pdf", "c.pdf"] {
            let path = scratch.join(name);
            write_pdf(&path, 1);
            pdfs.push(path);
        }

        // `a` comes back with a page index outside its own task; `b` fails
        // outright; `c` is fine.
        let broken = Value::Array(vec![
            json!({ "type": "text", "text": "stray", "page_idx": 9 }),
        ])
        .to_string()
        .into_bytes();
        let zips = vec![
            zip_of(&[("r/a_content_list.json", broken)]),
            zip_of(&[("r/c_content_list.json", content_list(0, 1))]),
        ];
        let submitted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let fake = {
            let submitted = submitted.clone();
            FakeServer::start(move |hit| {
                let origin = hit.origin.clone();
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
                    let index: usize = hit
                        .url
                        .trim_start_matches("/result/")
                        .trim_end_matches(".zip")
                        .parse()
                        .unwrap();
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
        let good = results[2]
            .as_ref()
            .expect("the third document was unaffected");
        assert_eq!(good.pages.len(), 1);
        assert_eq!(good.pages[0].markdown, "page 1");
    }

    /// Serves one batch: records the POSTed `data_id`s and the PUT order,
    /// and answers every poll with `done` and the same one-page result.
    fn recording_server(
        submitted: Arc<Mutex<Vec<String>>>,
        puts: Arc<Mutex<Vec<String>>>,
    ) -> FakeServer {
        let zip = zip_of(&[("r/x_content_list.json", content_list(0, 1))]);
        FakeServer::start(move |hit| {
            let origin = hit.origin.clone();
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
                hold(&puts).push(hit.url.clone());
                return Reply::bytes(Vec::new());
            }
            if hit.url.starts_with("/result/") {
                return Reply::bytes(zip.clone());
            }
            let results: Vec<Value> = hold(&submitted)
                .iter()
                .map(|id| json!({ "data_id": id, "state": "done", "full_zip_url": format!("{origin}/result/0.zip") }))
                .collect();
            Reply::json(json!({ "code": 0, "data": { "extract_result": results }}))
        })
    }

    #[test]
    fn the_smallest_file_is_uploaded_first() {
        let scratch = Scratch::new("cloud-order");
        let large = scratch.join("large.pdf");
        write_pdf(&large, 40);
        let small = scratch.join("small.pdf");
        write_pdf(&small, 1);
        assert!(fs::metadata(&large).unwrap().len() > fs::metadata(&small).unwrap().len());

        let puts = Arc::new(Mutex::new(Vec::new()));
        let fake = recording_server(Arc::new(Mutex::new(Vec::new())), puts.clone());
        let client = client(&fake, ledger(&scratch));
        let documents: Vec<Arc<CloudDocument>> = [&large, &small]
            .iter()
            .map(|pdf| CloudDocument::new(pdf, &scratch.join("images"), "images"))
            .collect();
        let results = client.extract_documents(&documents);
        assert!(results.iter().all(Result::is_ok));
        // The URLs come back in submit order: `large` is 0, `small` is 1.
        assert_eq!(*hold(&puts), vec!["/upload/1", "/upload/0"]);
    }

    #[test]
    fn a_skipped_document_is_never_reserved_submitted_or_uploaded() {
        let scratch = Scratch::new("cloud-skip");
        let skipped = scratch.join("skipped.pdf");
        write_pdf(&skipped, 4);
        let kept = scratch.join("kept.pdf");
        write_pdf(&kept, 1);
        Skips::shared().mark(&skipped);

        let submitted = Arc::new(Mutex::new(Vec::new()));
        let puts = Arc::new(Mutex::new(Vec::new()));
        let fake = recording_server(submitted.clone(), puts.clone());
        let book = ledger(&scratch);
        let client = client(&fake, book.clone());
        let documents: Vec<Arc<CloudDocument>> = [&skipped, &kept]
            .iter()
            .map(|pdf| CloudDocument::new(pdf, &scratch.join("images"), "images"))
            .collect();
        let results = client.extract_documents(&documents);
        Skips::shared().clear(&skipped);

        assert!(matches!(results[0], Err(ParseError::Cancelled)));
        assert!(results[1].is_ok());
        assert_eq!(hold(&submitted).len(), 1);
        assert_eq!(hold(&puts).len(), 1);
        let usage = book.snapshot();
        assert_eq!(
            (usage.files, usage.pages),
            (1, 1),
            "only the kept file was reserved"
        );
    }

    #[test]
    fn a_rejected_token_fails_the_whole_batch() {
        let scratch = Scratch::new("cloud-batch-auth");
        let mut documents = Vec::new();
        for name in ["a.pdf", "b.pdf"] {
            let path = scratch.join(name);
            write_pdf(&path, 1);
            documents.push(CloudDocument::new(&path, &scratch.join("images"), "images"));
        }
        let fake = FakeServer::start(|_| Reply::status(403, json!({ "msgCode": "A0202" })));
        let client = client(&fake, ledger(&scratch));

        let results = client.extract_documents(&documents);
        assert_eq!(results.len(), 2);
        for result in &results {
            let error = result
                .as_ref()
                .err()
                .expect("credentials condemn every file");
            assert!(matches!(
                error,
                ParseError::RejectedCredentials { expired: false, .. }
            ));
            assert!(error.latching());
        }
    }

    #[test]
    fn a_failed_submit_still_burns_its_reservation() {
        let scratch = Scratch::new("cloud-burn");
        let pdf = scratch.join("a.pdf");
        write_pdf(&pdf, 5);
        let fake = FakeServer::start(|_| Reply::status(500, json!({ "msg": "server" })));
        let book = ledger(&scratch);
        let client = client(&fake, book.clone());
        let document = CloudDocument::new(&pdf, &scratch.join("images"), "images");

        let results = client.extract_documents(&[document]);
        assert!(results[0].is_err());
        // The POST may have been counted server-side; uncertain failures count.
        let usage = book.snapshot();
        assert_eq!(usage.files, 1);
        assert_eq!(usage.pages, 5);
    }

    #[test]
    fn a_batch_id_that_cannot_sit_in_a_path_fails_only_its_own_batch() {
        let scratch = Scratch::new("cloud-batch-id");
        // One page per task: `first` fills the first batch, `second` the next.
        let first = scratch.join("first.pdf");
        write_pdf(&first, MAX_FILES_PER_BATCH);
        let second = scratch.join("second.pdf");
        write_pdf(&second, 1);
        let zip = zip_of(&[("r/x_content_list.json", content_list(0, 1))]);
        let submitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = submitted.clone();
        let fake = FakeServer::start(move |hit| {
            let origin = hit.origin.clone();
            if hit.method == "POST" {
                let files = hit.json()["files"].as_array().unwrap().clone();
                if files.len() == MAX_FILES_PER_BATCH {
                    return Reply::json(json!({ "code": 0, "data": {
                        "batch_id": "b/../admin",
                        "file_urls": (0..files.len()).map(|i| format!("{origin}/upload/{i}")).collect::<Vec<_>>(),
                    }}));
                }
                *hold(&seen) = files
                    .iter()
                    .map(|file| file["data_id"].as_str().unwrap().to_string())
                    .collect();
                return Reply::json(json!({ "code": 0, "data": {
                    "batch_id": "b-2",
                    "file_urls": [format!("{origin}/upload/0")],
                }}));
            }
            if hit.method == "PUT" {
                return Reply::bytes(Vec::new());
            }
            if hit.url.starts_with("/result/") {
                return Reply::bytes(zip.clone());
            }
            let results: Vec<Value> = hold(&seen)
                .iter()
                .map(|id| json!({ "data_id": id, "state": "done", "full_zip_url": format!("{origin}/result/0.zip") }))
                .collect();
            Reply::json(json!({ "code": 0, "data": { "extract_result": results }}))
        });
        let client = client(&fake, ledger(&scratch)).with_pages_per_task(1);
        let documents: Vec<Arc<CloudDocument>> = [&first, &second]
            .iter()
            .map(|pdf| CloudDocument::new(pdf, &scratch.join("images"), "images"))
            .collect();
        let results = client.extract_documents(&documents);

        match &results[0] {
            Err(error @ ParseError::Document { code }) => {
                assert_eq!(code, "batch-id");
                assert!(!error.latching());
            }
            other => panic!("{other:?}"),
        }
        assert!(results[1].is_ok(), "{:?}", results[1]);
        let hits = fake.hits();
        assert!(
            hits.iter().all(|hit| !hit.url.contains("admin")),
            "the bad id was never polled"
        );
        assert_eq!(hits.iter().filter(|hit| hit.method == "PUT").count(), 1);
        assert!(!hold(&submitted).is_empty());
    }

    #[test]
    fn a_batch_id_is_unreserved_characters_only() {
        for good in ["b-1", "1e2c4f.7_~", "AB12"] {
            assert!(is_batch_id(good), "{good}");
        }
        for bad in [
            "",
            "a/b",
            "..%2f",
            "a b",
            "a?x=1",
            "a&b",
            "ü",
            &"a".repeat(129),
        ] {
            assert!(!is_batch_id(bad), "{bad}");
        }
    }

    // ── Through oculus-keyd ──────────────────────────────────────────────────

    fn cloud() -> ParseConfig {
        ParseConfig {
            engine: crate::parse::Engine::Cloud,
            base_url: CLOUD_BASE_URL.to_string(),
            credentials: CredentialSource::Keychain,
            accept_expired_result_cert: true,
        }
    }

    /// A keyd holding a token that answers the `n`th `forward` with `answer(n)`.
    fn keyd_with<F>(scratch: &Scratch, answer: F) -> FakeKeyd
    where
        F: Fn(usize) -> (Value, Vec<u8>) + Send + 'static,
    {
        let forwards = std::sync::atomic::AtomicUsize::new(0);
        FakeKeyd::start(scratch, move |req, _| match req["op"].as_str().unwrap() {
            "has" => (json!({ "has": true }), vec![]),
            "forward" => answer(forwards.fetch_add(1, AtomicOrdering::SeqCst)),
            op => (
                json!({ "error": "request", "detail": format!("unexpected {op}") }),
                vec![],
            ),
        })
    }

    fn forwarded(status: u16, headers: Value, body: &Value) -> (Value, Vec<u8>) {
        let body = body.to_string().into_bytes();
        (
            json!({ "status": status, "headers": headers, "body_len": body.len() }),
            body,
        )
    }

    fn keyd_client(scratch: &Scratch) -> MinerUCloud {
        let Ok(client) = MinerUCloud::with_config_in(&cloud(), scratch, || {
            panic!("the keychain must not be read while keyd is installed")
        }) else {
            panic!("no client through keyd");
        };
        client
            .with_ledger(ledger(scratch))
            .with_buckets(
                Arc::new(TokenBucket::new(60_000.0)),
                Arc::new(TokenBucket::new(60_000.0)),
            )
            .with_time_scale(0.005)
    }

    fn config_error(config: &ParseConfig, scratch: &Scratch) -> ParseError {
        match MinerUCloud::with_config_in(config, scratch, || {
            Ok(Some("test-only-token".to_string()))
        }) {
            Ok(_) => panic!("expected an error"),
            Err(error) => error,
        }
    }

    fn poll(client: &MinerUCloud) -> Result<Value, ParseError> {
        client.api_json(
            "GET",
            "/extract-results/batch/b-1",
            None,
            &client.poll.clone(),
        )
    }

    #[test]
    fn the_keyd_path_is_the_cloud_roots_path() {
        assert!(CLOUD_BASE_URL.ends_with(CLOUD_PATH));
        assert_eq!(
            &CLOUD_BASE_URL[..CLOUD_BASE_URL.len() - CLOUD_PATH.len()],
            "https://mineru.net"
        );
    }

    #[test]
    fn through_keyd_both_calls_are_forwarded_without_the_token() {
        let scratch = Scratch::new("mk-ok");
        let keyd = keyd_with(&scratch, |_| {
            forwarded(200, json!([]), &json!({ "code": 0, "data": { "n": 1 } }))
        });
        let client = keyd_client(&scratch);
        assert_eq!(poll(&client).unwrap()["n"], 1);
        client
            .api_json(
                "POST",
                "/file-urls/batch",
                Some(json!({ "files": [] })),
                &client.submit.clone(),
            )
            .unwrap();

        assert_eq!(keyd.ops(), ["has", "forward", "forward"]);
        let requests = keyd.requests();
        let (get, body) = &requests[1];
        assert_eq!(get["secret"], "mineru");
        assert_eq!(get["method"], "GET");
        assert_eq!(get["path"], "/api/v4/extract-results/batch/b-1");
        assert_eq!(get["headers"], json!([["Accept", "application/json"]]));
        assert!(body.is_empty());
        let (post, body) = &requests[2];
        assert_eq!(post["method"], "POST");
        assert_eq!(post["path"], "/api/v4/file-urls/batch");
        assert_eq!(
            post["headers"],
            json!([
                ["Accept", "application/json"],
                ["Content-Type", "application/json"]
            ])
        );
        assert_eq!(
            serde_json::from_slice::<Value>(body).unwrap(),
            json!({ "files": [] })
        );
        for (header, _) in &requests {
            assert!(
                !header.to_string().to_lowercase().contains("authorization"),
                "{header}"
            );
        }
    }

    #[test]
    fn through_keyd_a_429_is_waited_out_without_spending_an_attempt() {
        let scratch = Scratch::new("mk-429");
        let keyd = keyd_with(&scratch, |n| {
            if n < 6 {
                forwarded(
                    429,
                    json!([["retry-after", "1"]]),
                    &json!({ "msg": "slow" }),
                )
            } else {
                forwarded(200, json!([]), &json!({ "code": 0, "data": {} }))
            }
        });
        poll(&keyd_client(&scratch)).unwrap();
        assert_eq!(keyd.ops().len(), 1 + 7);
    }

    #[test]
    fn through_keyd_a_refused_token_reads_as_it_does_direct() {
        let scratch = Scratch::new("mk-401");
        let keyd = keyd_with(&scratch, |_| {
            forwarded(401, json!([]), &json!({ "msgCode": "A0211" }))
        });
        let error = poll(&keyd_client(&scratch)).unwrap_err();
        assert!(
            matches!(error, ParseError::RejectedCredentials { expired: true, .. }),
            "{error:?}"
        );
        assert_eq!(
            keyd.ops(),
            ["has", "forward"],
            "a rejected token is not retried"
        );
    }

    #[test]
    fn keyd_without_a_token_is_missing_before_and_during_a_request() {
        let scratch = Scratch::new("mk-none");
        let _keyd = FakeKeyd::start(&scratch, |_, _| (json!({ "has": false }), vec![]));
        assert!(matches!(
            config_error(&cloud(), &scratch),
            ParseError::MissingCredentials
        ));

        // Deleted between the check and the request.
        let scratch = Scratch::new("mk-gone");
        let _keyd = keyd_with(&scratch, |_| {
            (
                json!({ "error": "missing", "detail": "no mineru key is stored" }),
                vec![],
            )
        });
        let error = poll(&keyd_client(&scratch)).unwrap_err();
        assert_eq!(error.kind(), "missing_credentials");
        assert!(error.latching());
    }

    #[test]
    fn keyd_reaching_no_origin_backs_off_then_reads_as_offline() {
        let scratch = Scratch::new("mk-upstream");
        let keyd = keyd_with(&scratch, |_| {
            (
                json!({ "error": "upstream", "detail": "Dns Failed: no such host" }),
                vec![],
            )
        });
        let error = poll(&keyd_client(&scratch)).unwrap_err();
        assert_eq!(error.kind(), "offline");
        assert!(error.to_string().contains("Dns Failed"), "{error}");
        assert_eq!(keyd.ops().len(), 1 + ATTEMPTS as usize);
    }

    #[test]
    fn keyds_own_refusals_surface_by_kind() {
        for (wire, kind) in [
            ("keychain", "unreadable_credentials"),
            ("caller", "credential_broker"),
            ("vault", "credential_broker"),
        ] {
            // Asked whether a token is saved.
            let scratch = Scratch::new("mk-refused");
            let _keyd = FakeKeyd::start(&scratch, move |_, _| {
                (json!({ "error": wire, "detail": "refused" }), vec![])
            });
            let error = config_error(&cloud(), &scratch);
            assert_eq!(error.kind(), kind, "{wire}");
            assert!(error.latching(), "{wire}");

            // Asked to send a request.
            let scratch = Scratch::new("mk-refused-send");
            let keyd = keyd_with(&scratch, move |_| {
                (json!({ "error": wire, "detail": "refused" }), vec![])
            });
            let error = poll(&keyd_client(&scratch)).unwrap_err();
            assert_eq!(error.kind(), kind, "{wire}");
            assert_eq!(keyd.ops().len(), 2, "{wire} is not retried");
        }
    }

    #[test]
    fn without_keyd_or_off_minerus_root_the_token_is_read_directly() {
        // No socket: the keychain's token.
        let scratch = Scratch::new("mk-absent");
        let Ok(client) = MinerUCloud::with_config_in(&cloud(), &scratch, || {
            Ok(Some("test-only-token".to_string()))
        }) else {
            panic!("a direct client");
        };
        assert!(matches!(&client.auth, Auth::Direct(token) if token.as_str() == "test-only-token"));
        assert!(matches!(
            MinerUCloud::with_config_in(&cloud(), &scratch, || Ok(None)).err(),
            Some(ParseError::MissingCredentials)
        ));
        let refused = MinerUCloud::with_config_in(&cloud(), &scratch, || {
            Err(ParseError::UnreadableCredentials("denied".into()))
        });
        assert_eq!(refused.err().unwrap().kind(), "unreadable_credentials");

        // A hand-edited `engineUrl` never goes through keyd, even when it is there.
        let scratch = Scratch::new("mk-elsewhere");
        let keyd = keyd_with(&scratch, |_| forwarded(500, json!([]), &json!({})));
        let fake = FakeServer::start(|_| Reply::json(json!({ "code": 0, "data": {} })));
        let config = ParseConfig {
            base_url: format!("{}/api/v4", fake.origin()),
            ..cloud()
        };
        let Ok(client) = MinerUCloud::with_config_in(&config, &scratch, || {
            Ok(Some("test-only-token".to_string()))
        }) else {
            panic!("a direct client");
        };
        poll(&client.with_ledger(ledger(&scratch))).unwrap();
        assert!(keyd.requests().is_empty());
        assert_eq!(
            fake.hits()[0].header("authorization"),
            Some("Bearer test-only-token")
        );
    }

    #[test]
    fn clients_through_keyd_share_a_batch_and_direct_ones_go_by_token() {
        let scratch = Scratch::new("mk-batch");
        let _keyd = keyd_with(&scratch, |_| forwarded(500, json!([]), &json!({})));
        assert_eq!(
            keyd_client(&scratch).batch_key(),
            keyd_client(&scratch).batch_key()
        );
        let direct = |token: &str| MinerUCloud::new(CLOUD_BASE_URL, token).unwrap().batch_key();
        assert_eq!(direct("a"), direct("a"));
        assert_ne!(direct("a"), direct("b"));
        assert_ne!(direct("a"), keyd_client(&scratch).batch_key());
    }

    // ── The result archive ───────────────────────────────────────────────────

    #[test]
    fn a_zip_that_climbs_out_of_its_directory_is_refused() {
        let scratch = Scratch::new("cloud-slip");
        let archive = scratch.join("bad.zip");
        fs::write(&archive, zip_of(&[("../escape.txt", b"bad".to_vec())])).unwrap();

        let error = safe_extract(&archive, &scratch.join("out")).unwrap_err();
        assert!(matches!(error, ParseError::Document { .. }), "{error}");
        assert!(!scratch.join("escape.txt").exists());
    }

    #[test]
    fn the_content_list_is_found_by_shape_not_by_name() {
        let scratch = Scratch::new("cloud-glob");
        let root = scratch.join("result");
        fs::create_dir_all(root.join("deep/nested")).unwrap();
        // Never the flat markdown beside it.
        fs::write(root.join("deck.md"), "# not this").unwrap();
        fs::write(root.join("deep/nested/whatever_content_list.json"), "[]").unwrap();

        assert_eq!(
            find_content_list(&root),
            Some(root.join("deep/nested/whatever_content_list.json"))
        );
        assert_eq!(find_content_list(&scratch.join("missing")), None);
    }
}
