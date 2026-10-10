//! The seam every PDF parser plugs into: MinerU cloud over HTTPS, or MinerU's
//! own server (installed and started by the user) over loopback. Nothing here
//! may assume the cloud; an API root, a token or an upload ceiling arrives as
//! configuration.
//!
//! The seam owns the contract, not the parsing: the on-disk artifact layout,
//! the version that marks a file done, the error vocabulary the failure UI
//! reads, and the order the artifacts hit the disk. See `docs/parsing.md`.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};

use serde::{Deserialize, Serialize};

/// The version stamped into every `.pages.json`. It moves only when the
/// artifacts themselves change shape — not when a backend or the app changes.
pub const PARSER_VERSION: u32 = 2;

/// The one parse tier. Records on disk carry the field, so it round-trips.
pub const MODE: &str = "quality";

// ── Artifact locations ───────────────────────────────────────────────────────
//
// The image directory's name is also the link prefix written into the markdown
// (`![](<stem>_images/x.jpg)`), so both are computed here, never by a backend.

pub fn md_path(pdf: &Path) -> PathBuf {
    pdf.with_extension("md")
}

pub fn pages_path(pdf: &Path) -> PathBuf {
    pdf.with_extension("pages.json")
}

pub fn images_dir_for(pdf: &Path) -> PathBuf {
    let stem = pdf
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    pdf.with_file_name(format!("{stem}_images"))
}

/// The record beside this PDF, read back as the type that wrote it.
pub fn read_record(pdf: &Path) -> Option<ParseOutput> {
    serde_json::from_str(&fs::read_to_string(pages_path(pdf)).ok()?).ok()
}

/// `Some("quality")` when this PDF is parsed, `None` when it still needs it —
/// the record missing, unreadable, or naming another mode all mean parse it.
///
/// * Never inferred from the images directory or the `.md`: both survive a
///   crash that never wrote the record.
/// * No `parser_version` check: a version bump must not re-parse the library.
pub fn parse_mode(pdf: &Path) -> Option<&'static str> {
    let text = fs::read_to_string(pages_path(pdf)).ok()?;
    let record: serde_json::Value = serde_json::from_str(&text).ok()?;
    (record.get("mode").and_then(|v| v.as_str()) == Some(MODE)).then_some(MODE)
}

// ── The parsed document ──────────────────────────────────────────────────────

/// One page's markdown, keyed by its 1-based page number — the join key
/// retrieval rests on, which is why `ParseOutput::new` normalises it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsePage {
    pub page_no: u32,
    pub markdown: String,
}

/// Exactly the `.pages.json` on disk, plus one field that never goes there.
/// Field names are the wire format, and existing records must keep reading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseOutput {
    /// The PDF's file name, not its path — the record travels with the folder.
    pub pdf: String,
    pub mode: String,
    pub parser_version: u32,
    pub page_count: u32,
    pub pages: Vec<ParsePage>,
    /// Omitted rather than null: a reader that sees the key can trust it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// For progress and logging; never written to the record.
    #[serde(skip)]
    pub image_count: u32,
}

impl ParseOutput {
    /// Exactly one entry per page `1..=page_count`, in order, `""` where a
    /// page yielded nothing — whatever order or gaps the backend returned.
    pub fn new(
        pdf: &Path,
        page_count: u32,
        pages: Vec<ParsePage>,
        backend: Option<String>,
        image_count: u32,
    ) -> Self {
        let mut slots = vec![String::new(); page_count as usize];
        for page in pages {
            if page.page_no >= 1 && page.page_no <= page_count {
                slots[(page.page_no - 1) as usize] = page.markdown;
            }
        }
        Self {
            pdf: pdf
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            mode: MODE.to_string(),
            parser_version: PARSER_VERSION,
            page_count,
            pages: slots
                .into_iter()
                .enumerate()
                .map(|(i, markdown)| ParsePage {
                    page_no: i as u32 + 1,
                    markdown,
                })
                .collect(),
            backend,
            image_count,
        }
    }

    /// The full-document markdown: pages joined by a blank line.
    pub fn document_markdown(&self) -> String {
        self.pages
            .iter()
            .map(|p| p.markdown.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Put the artifacts on disk. `.pages.json` is the only evidence a parse
    /// finished, so it lands last and atomically (temp file, fsync, rename).
    pub fn write(&self, pdf: &Path, images: ImageStaging) -> Result<(), ParseError> {
        images.commit()?;
        self.write_markdown(pdf)?;

        // serde_json leaves non-ASCII unescaped, as the existing records are.
        // The temp name is unique per write, so two writers never share one.
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let final_path = pages_path(pdf);
        let body = serde_json::to_vec(self)
            .map_err(|e| ParseError::Io(format!("encode {}: {e}", final_path.display())))?;
        let tmp = final_path.with_extension(format!(
            "json.tmp{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        crate::atomic_write::write(&final_path, &tmp, &body).map_err(ParseError::Io)
    }

    /// `<stem>.md`, derived from the record.
    fn write_markdown(&self, pdf: &Path) -> Result<(), ParseError> {
        let md = md_path(pdf);
        fs::write(&md, self.document_markdown())
            .map_err(|e| ParseError::Io(format!("write {}: {e}", md.display())))
    }

    /// Re-derive a missing `<stem>.md` from this record; true when written.
    /// The record is the evidence of the parse, so nothing is re-parsed.
    pub fn restore_markdown(&self, pdf: &Path) -> Result<bool, ParseError> {
        if md_path(pdf).is_file() {
            return Ok(false);
        }
        self.write_markdown(pdf).map(|()| true)
    }
}

// ── One parse per PDF at a time ──────────────────────────────────────────────

/// PDFs with a parse running in this process. A sync, the sweep and "Parse
/// now" can ask for the same file at once; a second caller waits here for the
/// first, then finds the record and skips. No timeout: a parse takes minutes.
pub struct InFlight {
    running: Mutex<BTreeSet<PathBuf>>,
    freed: Condvar,
}

/// Held for one parse; dropping it (unwinding included) frees the PDF.
pub struct InFlightClaim<'a> {
    owner: &'a InFlight,
    pdf: PathBuf,
}

impl InFlight {
    pub const fn new() -> Self {
        Self {
            running: Mutex::new(BTreeSet::new()),
            freed: Condvar::new(),
        }
    }

    /// The process-wide set every parse goes through.
    pub fn shared() -> &'static InFlight {
        static SHARED: InFlight = InFlight::new();
        &SHARED
    }

    /// Block until no other parse holds `pdf`, then hold it.
    pub fn claim(&self, pdf: &Path) -> InFlightClaim<'_> {
        let mut running = self.running.lock().unwrap_or_else(|p| p.into_inner());
        while running.contains(pdf) {
            running = self.freed.wait(running).unwrap_or_else(|p| p.into_inner());
        }
        running.insert(pdf.to_path_buf());
        InFlightClaim {
            owner: self,
            pdf: pdf.to_path_buf(),
        }
    }
}

impl Drop for InFlightClaim<'_> {
    fn drop(&mut self) {
        self.owner
            .running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.pdf);
        self.owner.freed.notify_all();
    }
}

// ── Image staging ────────────────────────────────────────────────────────────

/// A scratch directory for extracted images, swapped into place only when the
/// whole parse succeeded. Dropped without committing, it deletes itself and the
/// previous parse's artifacts are untouched: a failure never leaves a file
/// less parsed than it was.
pub struct ImageStaging {
    staged: PathBuf,
    destination: PathBuf,
    rel: String,
}

impl ImageStaging {
    /// Stage beside the PDF, never in the system temp dir: the swap is a
    /// rename, which fails across filesystems. The dot prefix keeps it out of
    /// file listings.
    pub fn begin(pdf: &Path) -> Result<Self, ParseError> {
        let destination = images_dir_for(pdf);
        let name = destination
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stamp = crate::clock::now_nanos();
        let scratch = pdf.with_file_name(format!(".{name}-staging-{}-{stamp}", std::process::id()));
        fs::create_dir_all(scratch.join(&name))
            .map_err(|e| ParseError::Io(format!("stage {}: {e}", scratch.display())))?;
        Ok(Self {
            staged: scratch.join(&name),
            destination,
            rel: name,
        })
    }

    /// Where the backend writes extracted images.
    pub fn dir(&self) -> &Path {
        &self.staged
    }

    /// The link prefix for the markdown: the *final* directory's name, not
    /// the scratch one.
    pub fn rel(&self) -> &str {
        &self.rel
    }

    /// Swap the staged images into place; `Drop` then clears the wrapper.
    /// Replaced, not merged: a re-parse renumbers crops.
    fn commit(self) -> Result<(), ParseError> {
        fs::remove_dir_all(&self.destination).ok();
        fs::rename(&self.staged, &self.destination)
            .map_err(|e| ParseError::Io(format!("swap in {}: {e}", self.destination.display())))
    }
}

impl Drop for ImageStaging {
    fn drop(&mut self) {
        if let Some(root) = self.staged.parent() {
            fs::remove_dir_all(root).ok();
        }
    }
}

// ── Progress and health ──────────────────────────────────────────────────────

/// Where a running parse is. Only the cloud uploads; the local engine and a
/// cloud file whose bytes are sent both report `Processing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Its batch was submitted; another file of the batch is uploading first.
    UploadWait,
    Uploading,
    /// Uploaded (or local): the engine is extracting.
    Processing,
}

impl Phase {
    /// The `phase` on the `parse-status` wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::UploadWait => "upload_wait",
            Phase::Uploading => "uploading",
            Phase::Processing => "processing",
        }
    }
}

/// Reported while a parse runs. `total_pages` is zero until the backend knows;
/// the byte counts are zero outside the upload phases.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Progress {
    pub pages_done: u32,
    pub total_pages: u32,
    pub backend: &'static str,
    pub phase: Phase,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

impl Progress {
    /// The engine is extracting: pages only.
    pub fn processing(pages_done: u32, total_pages: u32, backend: &'static str) -> Self {
        Self {
            pages_done,
            total_pages,
            backend,
            phase: Phase::Processing,
            bytes_done: 0,
            bytes_total: 0,
        }
    }
}

// ── Skipping a file's parse ──────────────────────────────────────────────────

/// PDFs the user skipped this session, keyed like `InFlight`. Memory only:
/// across restarts the frontend's `files.parse_status = 'skipped'` holds it.
/// Engines poll `is_marked` and end the parse as `ParseError::Cancelled`.
pub struct Skips {
    marked: Mutex<BTreeSet<PathBuf>>,
}

impl Skips {
    pub const fn new() -> Self {
        Self {
            marked: Mutex::new(BTreeSet::new()),
        }
    }

    pub fn shared() -> &'static Skips {
        static SHARED: Skips = Skips::new();
        &SHARED
    }

    pub fn mark(&self, pdf: &Path) {
        self.marked
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(pdf.to_path_buf());
    }

    pub fn clear(&self, pdf: &Path) {
        self.marked
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(pdf);
    }

    pub fn is_marked(&self, pdf: &Path) -> bool {
        self.marked
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(pdf)
    }
}

/// `Err(Cancelled)` when the user skipped this PDF.
pub fn check_skipped(pdf: &Path) -> Result<(), ParseError> {
    if Skips::shared().is_marked(pdf) {
        return Err(ParseError::Cancelled);
    }
    Ok(())
}

/// What a backend says about itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub backend: String,
    pub parser_version: u32,
    pub ready: bool,
}

impl Health {
    /// The version handshake: a backend stamping another `parser_version` is
    /// refused, not warned about. Both backends render through the same
    /// `render` today, so that arm guards a future backend; `NotReady` is the
    /// local server still loading, which retries rather than failing the PDF.
    pub fn check(&self) -> Result<(), ParseError> {
        if self.parser_version != PARSER_VERSION {
            return Err(ParseError::VersionMismatch {
                app: PARSER_VERSION,
                backend: self.parser_version,
            });
        }
        if !self.ready {
            return Err(ParseError::NotReady {
                backend: self.backend.clone(),
            });
        }
        Ok(())
    }
}

/// Ask a backend whether it can be used; every call site about to parse
/// goes through here.
pub fn preflight(parser: &dyn Parser) -> Result<Health, ParseError> {
    let health = parser.health();
    health.check()?;
    Ok(health)
}

// ── The trait ────────────────────────────────────────────────────────────────

pub trait Parser: Send + Sync {
    /// Parse one PDF. `images_dir` and `images_rel` come from `ImageStaging`,
    /// whose scratch directory's name differs from the prefix on purpose.
    /// `on_progress` may be called from any thread.
    fn parse(
        &self,
        pdf: &Path,
        images_dir: &Path,
        images_rel: &str,
        on_progress: &dyn Fn(Progress),
    ) -> Result<ParseOutput, ParseError>;

    fn health(&self) -> Health;
}

// ── Failure ──────────────────────────────────────────────────────────────────

/// The `Document` code for an Office file LibreOffice could not convert: there
/// is no PDF, so nothing was sent to any parser.
pub const CONVERSION_FAILED: &str = "office-conversion";

/// The `Document` code for a spreadsheet `crate::sheets` could not read: it
/// is converted to text in-process, never parsed.
pub const SHEET_UNREADABLE: &str = "sheet-unreadable";

/// Why a parse did not happen. Nothing falls back (see `docs/parsing.md`), so the
/// variants keep "wait", "retry" and "fix a setting" distinguishable.
///
/// **No variant carries server response text**: MinerU's error bodies can hold
/// the signed upload URLs. Errors carry the code, never the body.
#[derive(Debug, Clone)]
pub enum ParseError {
    /// No token is stored. Nothing will parse until one is.
    MissingCredentials,
    /// A token may be stored, but the keychain refused to hand it over (a
    /// denied prompt, or a sandboxed process). Holds the keychain's own error.
    UnreadableCredentials(String),
    /// `oculus-keyd`, which holds the token, refused this process or could not
    /// use its vault. Latching: every request goes through it.
    Broker(String),
    /// The backend refused the token. Latching: every other file would too.
    RejectedCredentials { code: Option<String>, expired: bool },
    /// The daily allowance is spent; repairs itself at the next reset.
    QuotaExhausted,
    /// Could not reach the backend. Holds the local transport error only.
    Offline(String),
    /// Over the backend's upload ceiling, refused before anything is sent.
    TooLarge { bytes: u64, limit_bytes: u64 },
    /// The backend could not read this document; the rest of the queue goes on.
    Document { code: String },
    /// The backend writes a different artifact version than this app reads.
    VersionMismatch { app: u32, backend: u32 },
    /// The backend answered but is not accepting work yet.
    NotReady { backend: String },
    /// Writing the artifacts failed; the parse itself may have succeeded.
    Io(String),
    /// The user skipped this file (`Skips`). Not a failure: nothing to retry
    /// until they ask for the parse again.
    Cancelled,
}

impl ParseError {
    /// The frozen discriminant the failure UI branches on (`Display` may be
    /// reworded). `app/src/lib/parseState.ts` matches `/credential|token/i`
    /// against it, so every credential variant keeps that word.
    pub fn kind(&self) -> &'static str {
        match self {
            ParseError::MissingCredentials => "missing_credentials",
            ParseError::UnreadableCredentials(_) => "unreadable_credentials",
            ParseError::Broker(_) => "credential_broker",
            ParseError::RejectedCredentials { .. } => "rejected_credentials",
            ParseError::QuotaExhausted => "quota_exhausted",
            ParseError::Offline(_) => "offline",
            ParseError::TooLarge { .. } => "too_large",
            ParseError::Document { .. } => "document",
            ParseError::VersionMismatch { .. } => "version_mismatch",
            ParseError::NotReady { .. } => "not_ready",
            ParseError::Io(_) => "io",
            ParseError::Cancelled => "cancelled",
        }
    }

    /// Could retrying *this file*, unchanged, ever succeed? Drives whether a
    /// failure offers a retry at all.
    pub fn retryable(&self) -> bool {
        match self {
            ParseError::Offline(_)
            | ParseError::Io(_)
            | ParseError::QuotaExhausted
            | ParseError::NotReady { .. }
            | ParseError::UnreadableCredentials(_)
            | ParseError::Broker(_) => true,
            ParseError::MissingCredentials
            | ParseError::RejectedCredentials { .. }
            | ParseError::TooLarge { .. }
            | ParseError::Document { .. }
            | ParseError::VersionMismatch { .. }
            | ParseError::Cancelled => false,
        }
    }

    /// Does this condemn every other file too? A latching failure stops the run.
    pub fn latching(&self) -> bool {
        matches!(
            self,
            ParseError::MissingCredentials
                | ParseError::UnreadableCredentials(_)
                | ParseError::Broker(_)
                | ParseError::RejectedCredentials { .. }
                | ParseError::QuotaExhausted
                | ParseError::VersionMismatch { .. }
        )
    }
}

/// Shown to a student: what happened and what would change it — never a URL,
/// a token or anything the server said.
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::MissingCredentials => {
                write!(
                    f,
                    "No MinerU API token is saved — add one in Settings to parse PDFs."
                )
            }
            ParseError::UnreadableCredentials(detail) => write!(
                f,
                "The keychain refused to give out the MinerU API token ({detail}). The token \
                 is not missing — macOS denied this process access to it."
            ),
            ParseError::Broker(detail) => write!(
                f,
                "oculus-keyd, which holds the MinerU token, could not send this request: \
                 {detail}"
            ),
            ParseError::RejectedCredentials { code, expired } => {
                let code = code
                    .as_deref()
                    .map(|c| format!(" ({c})"))
                    .unwrap_or_default();
                if *expired {
                    write!(
                        f,
                        "The MinerU API token has expired{code} — create a new one and paste it \
                         into Settings."
                    )
                } else {
                    write!(
                        f,
                        "MinerU rejected the API token{code} — check it was copied in full, or \
                         create a new one in Settings."
                    )
                }
            }
            ParseError::QuotaExhausted => write!(
                f,
                "MinerU's daily quota is used up. Parsing resumes on its own after the quota \
                 resets."
            ),
            ParseError::Offline(detail) => write!(f, "Could not reach MinerU: {detail}"),
            ParseError::TooLarge { bytes, limit_bytes } => write!(
                f,
                "This PDF is {} and MinerU accepts files up to {}, so it was not sent.",
                megabytes(*bytes),
                megabytes(*limit_bytes)
            ),
            ParseError::Document { code } if code == CONVERSION_FAILED => write!(
                f,
                "This file could not be converted to PDF, so there is nothing to parse. The \
                 next sync tries the conversion again."
            ),
            ParseError::Document { code } if code == SHEET_UNREADABLE => write!(
                f,
                "This spreadsheet could not be read, so it has no text. Other files are \
                 unaffected."
            ),
            ParseError::Document { code } => write!(
                f,
                "MinerU could not read this PDF (error {code}). Other files are unaffected."
            ),
            ParseError::VersionMismatch { app, backend } => write!(
                f,
                "The parse backend writes version {backend} files but this app reads version \
                 {app}. Update whichever is older before parsing."
            ),
            ParseError::NotReady { backend } => {
                write!(
                    f,
                    "The {backend} parser is not ready yet. Try again in a moment."
                )
            }
            ParseError::Io(detail) => write!(f, "Could not save the parsed output: {detail}"),
            ParseError::Cancelled => write!(f, "Skipped — parse it again from File Activity."),
        }
    }
}

impl std::error::Error for ParseError {}

fn megabytes(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / (1024.0 * 1024.0))
}

/// Refuse an oversized file before uploading it, against the backend's limit.
pub fn check_size(pdf: &Path, limit_bytes: u64) -> Result<u64, ParseError> {
    let bytes = fs::metadata(pdf)
        .map_err(|e| ParseError::Io(format!("stat {}: {e}", pdf.display())))?
        .len();
    if bytes > limit_bytes {
        return Err(ParseError::TooLarge { bytes, limit_bytes });
    }
    Ok(bytes)
}

// ── Configuration ────────────────────────────────────────────────────────────

/// Which backend *is* the parser. Only the `engine` key selects one; the
/// blob's legacy `backend` key (`local | cloud | auto`, a fallback policy) is
/// ignored, because its "local" named a different program. Absent means
/// `Cloud`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Cloud,
    Local,
}

impl Engine {
    pub fn as_str(self) -> &'static str {
        match self {
            Engine::Cloud => "cloud",
            Engine::Local => "local",
        }
    }

    /// Where this engine lives when nothing overrides it; also the settings
    /// page's placeholder, so the two cannot differ.
    pub fn default_base_url(self) -> &'static str {
        match self {
            Engine::Cloud => CLOUD_BASE_URL,
            Engine::Local => LOCAL_BASE_URL,
        }
    }

    /// Anything that is not one of the two names — `"auto"` included — is not
    /// an engine, and is treated as if the field were absent.
    fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "cloud" => Some(Engine::Cloud),
            "local" => Some(Engine::Local),
            _ => None,
        }
    }
}

/// Where a backend's token comes from, if it needs one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSource {
    /// `oculus-keyd` when it is installed and the API root is MinerU's own,
    /// else the macOS keychain via `crate::mineru` (`MinerUCloud::with_config`).
    /// The token never enters SQLite or the WebView.
    Keychain,
    /// Loopback to a server on this machine: nothing to authenticate.
    None,
}

impl CredentialSource {
    /// The keychain's token, for the path that runs with keyd absent. A
    /// refusal is `UnreadableCredentials`, never "no token".
    pub fn token(self) -> Result<Option<String>, ParseError> {
        match self {
            CredentialSource::Keychain => {
                crate::mineru::fetch_api_key().map_err(ParseError::UnreadableCredentials)
            }
            CredentialSource::None => Ok(None),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseConfig {
    pub engine: Engine,
    /// API root for the chosen engine, overridable from the settings blob.
    pub base_url: String,
    pub credentials: CredentialSource,
    /// Download results through the CDN's expired certificate (see
    /// `mineru::result_tls`). Defaults to on.
    pub accept_expired_result_cert: bool,
}

/// MinerU's published API root.
pub const CLOUD_BASE_URL: &str = "https://mineru.net/api/v4";

/// The local parse server's default origin: where MinerU's own server binds
/// by default. `engineUrl` overrides it.
pub const LOCAL_BASE_URL: &str = "http://127.0.0.1:8000";

/// The `parse` row, as far as this seam cares. Every field is optional; the
/// blob's other keys are ignored (see the note in `app/src/lib/db.ts`).
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct StoredParseSettings {
    /// A string, not `Engine`: a stale value costs this field, not the row.
    engine: Option<String>,
    engine_url: Option<String>,
    accept_expired_result_cert: Option<bool>,
}

/// The backend selection from the `parse` settings row. Unreadable, missing
/// or nonsense settings resolve to the cloud default.
pub fn parse_config() -> ParseConfig {
    let stored = stored_settings().unwrap_or_default();
    let engine = stored
        .engine
        .as_deref()
        .and_then(Engine::parse)
        .unwrap_or(Engine::Cloud);
    let base_url = stored
        .engine_url
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| engine.default_base_url().to_string());
    let credentials = match engine {
        Engine::Cloud => CredentialSource::Keychain,
        Engine::Local => CredentialSource::None,
    };
    let accept_expired_result_cert = stored.accept_expired_result_cert.unwrap_or(true);
    ParseConfig {
        engine,
        base_url,
        credentials,
        accept_expired_result_cert,
    }
}

/// The parser this install is configured for; every caller about to parse
/// comes through here. `MinerUCloud` refuses to exist without a token, so
/// `MissingCredentials` surfaces before a file is touched. A local server that
/// is not running is the first parse's `Offline`, not a construction error.
pub fn backend() -> Result<Box<dyn Parser>, ParseError> {
    match parse_config().engine {
        Engine::Cloud => Ok(Box::new(mineru::client::MinerUCloud::from_config()?)),
        Engine::Local => Ok(Box::new(mineru::local::MinerULocal::from_config())),
    }
}

/// The `parse` row, decoded — never a `block_on`: see `store::setting_blocking`.
fn stored_settings() -> Option<StoredParseSettings> {
    serde_json::from_str(&crate::store::setting_blocking("parse")?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    /// A `block_on` on a runtime worker panics the task; surviving the call is
    /// what is under test, so it passes with no database.
    #[test]
    fn the_config_is_readable_from_inside_an_async_runtime() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let base = runtime.block_on(async { parse_config().base_url });
        assert!(
            !base.is_empty(),
            "a backend always resolves to some API root"
        );
    }

    /// A real scratch folder: the staging rename must be same-filesystem.
    fn scratch(name: &str) -> Scratch {
        Scratch::new(&format!("parse-{name}"))
    }

    fn sample_pdf(dir: &Path) -> PathBuf {
        let pdf = dir.join("Lecture 3.pdf");
        fs::write(&pdf, b"%PDF-1.4").unwrap();
        pdf
    }

    #[test]
    fn artifact_names_match_the_records_already_on_disk() {
        let pdf = Path::new("/library/subj/Lecture 3.pdf");
        assert_eq!(md_path(pdf), Path::new("/library/subj/Lecture 3.md"));
        assert_eq!(
            pages_path(pdf),
            Path::new("/library/subj/Lecture 3.pages.json")
        );
        assert_eq!(
            images_dir_for(pdf),
            Path::new("/library/subj/Lecture 3_images")
        );
    }

    #[test]
    fn pages_are_ordered_and_gap_filled() {
        let pdf = Path::new("/library/Lecture 3.pdf");
        let out = ParseOutput::new(
            pdf,
            4,
            vec![
                ParsePage {
                    page_no: 3,
                    markdown: "three".into(),
                },
                ParsePage {
                    page_no: 1,
                    markdown: "one".into(),
                },
                // Out of range: a backend that split the document and got its
                // offsets wrong must not be able to corrupt the join key.
                ParsePage {
                    page_no: 9,
                    markdown: "nine".into(),
                },
            ],
            Some("mineru-cloud".into()),
            2,
        );
        assert_eq!(out.page_count, 4);
        assert_eq!(
            out.pages.iter().map(|p| p.page_no).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(out.pages[1].markdown, "");
        // Blank pages keep their slot, so the `.md` and the records stay in step.
        assert_eq!(out.document_markdown(), "one\n\n\n\nthree\n\n");
    }

    #[test]
    fn record_keeps_the_python_wire_shape() {
        let pdf = Path::new("/library/Lecture 3.pdf");
        let out = ParseOutput::new(
            pdf,
            1,
            vec![ParsePage {
                page_no: 1,
                markdown: "ψ".into(),
            }],
            None,
            0,
        );
        let json = serde_json::to_string(&out).unwrap();
        // Non-ASCII unescaped, as `ensure_ascii=False` wrote it.
        assert!(json.contains("\"ψ\""), "{json}");
        assert!(!json.contains("backend"), "{json}");
        assert!(!json.contains("image_count"), "{json}");
        assert!(json.contains("\"parser_version\":2"), "{json}");
    }

    #[test]
    fn write_swaps_images_and_lands_the_record_last() {
        let dir = scratch("write");
        let pdf = sample_pdf(&dir);

        // A previous parse's artifacts, which this one replaces.
        fs::create_dir_all(images_dir_for(&pdf)).unwrap();
        fs::write(images_dir_for(&pdf).join("old.jpg"), b"old").unwrap();

        let staging = ImageStaging::begin(&pdf).unwrap();
        assert_eq!(staging.rel(), "Lecture 3_images");
        fs::write(staging.dir().join("new.jpg"), b"new").unwrap();

        let out = ParseOutput::new(
            &pdf,
            1,
            vec![ParsePage {
                page_no: 1,
                markdown: "![](Lecture 3_images/new.jpg)".into(),
            }],
            Some("mineru-cloud".into()),
            1,
        );
        out.write(&pdf, staging).unwrap();

        assert!(images_dir_for(&pdf).join("new.jpg").is_file());
        assert!(!images_dir_for(&pdf).join("old.jpg").exists());
        assert!(md_path(&pdf).is_file());
        assert_eq!(read_record(&pdf).unwrap().parser_version, PARSER_VERSION);
        assert_eq!(parse_mode(&pdf), Some(MODE));
        // No temp record left behind.
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".tmp") || n.starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn a_failed_parse_leaves_the_previous_artifacts_alone() {
        let dir = scratch("abandon");
        let pdf = sample_pdf(&dir);
        fs::create_dir_all(images_dir_for(&pdf)).unwrap();
        fs::write(images_dir_for(&pdf).join("old.jpg"), b"old").unwrap();
        fs::write(md_path(&pdf), "previous").unwrap();

        {
            let staging = ImageStaging::begin(&pdf).unwrap();
            fs::write(staging.dir().join("half.jpg"), b"half").unwrap();
            // Dropped without committing, as an `Err` out of `parse` would.
        }

        assert!(images_dir_for(&pdf).join("old.jpg").is_file());
        assert_eq!(fs::read_to_string(md_path(&pdf)).unwrap(), "previous");
        let entries: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with('.'))
            .collect();
        assert!(entries.is_empty(), "staging survived: {entries:?}");
    }

    #[test]
    fn version_mismatch_is_refused_and_names_both_sides() {
        let health = Health {
            backend: "oculus-local".into(),
            parser_version: PARSER_VERSION + 1,
            ready: true,
        };
        let error = health.check().unwrap_err();
        assert!(matches!(error, ParseError::VersionMismatch { .. }));
        let shown = error.to_string();
        assert!(shown.contains(&(PARSER_VERSION + 1).to_string()), "{shown}");
        assert!(shown.contains(&PARSER_VERSION.to_string()), "{shown}");
        assert!(!error.retryable());
        assert!(error.latching());
    }

    #[test]
    fn only_a_quality_record_counts_as_parsed() {
        let dir = scratch("mode");
        let pdf = sample_pdf(&dir);

        // No record at all.
        assert_eq!(parse_mode(&pdf), None);

        // Markdown and an images directory are not evidence: both survive a
        // crash that never wrote a record.
        fs::write(md_path(&pdf), "half a parse").unwrap();
        fs::create_dir_all(images_dir_for(&pdf)).unwrap();
        assert_eq!(parse_mode(&pdf), None);

        // The retired tier, and unreadable JSON, both mean "parse it".
        fs::write(pages_path(&pdf), r#"{"mode":"fast","parser_version":2}"#).unwrap();
        assert_eq!(parse_mode(&pdf), None);
        fs::write(pages_path(&pdf), "{ not json").unwrap();
        assert_eq!(parse_mode(&pdf), None);

        // Done regardless of the version that wrote it.
        fs::write(pages_path(&pdf), r#"{"mode":"quality","parser_version":1}"#).unwrap();
        assert_eq!(parse_mode(&pdf), Some("quality"));
    }

    #[test]
    fn a_second_parse_of_the_same_pdf_waits_for_the_first() {
        use std::sync::atomic::AtomicUsize;
        use std::sync::Arc;
        use std::time::Duration;

        let in_flight: &'static InFlight = Box::leak(Box::new(InFlight::new()));
        let live = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..3)
            .map(|_| {
                let (live, peak) = (live.clone(), peak.clone());
                std::thread::spawn(move || {
                    let _claim = in_flight.claim(Path::new("/library/a.pdf"));
                    let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(50));
                    live.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 1);

        // Another PDF is not held up, and a panicking holder still frees its PDF.
        let _a = in_flight.claim(Path::new("/library/a.pdf"));
        let _b = in_flight.claim(Path::new("/library/b.pdf"));
        let crashed = std::thread::spawn(move || {
            let _claim = in_flight.claim(Path::new("/library/c.pdf"));
            panic!("parser crashed");
        });
        assert!(crashed.join().is_err());
        let _c = in_flight.claim(Path::new("/library/c.pdf"));
    }

    #[test]
    fn a_lost_markdown_is_rebuilt_from_the_record() {
        let dir = scratch("restore-md");
        let pdf = sample_pdf(&dir);
        let out = ParseOutput::new(
            &pdf,
            2,
            vec![
                ParsePage {
                    page_no: 1,
                    markdown: "one".into(),
                },
                ParsePage {
                    page_no: 2,
                    markdown: "two".into(),
                },
            ],
            None,
            0,
        );
        out.write(&pdf, ImageStaging::begin(&pdf).unwrap()).unwrap();
        fs::remove_file(md_path(&pdf)).unwrap();

        let record = read_record(&pdf).unwrap();
        assert!(record.restore_markdown(&pdf).unwrap());
        assert_eq!(fs::read_to_string(md_path(&pdf)).unwrap(), "one\n\ntwo");
        // Present already: left alone.
        fs::write(md_path(&pdf), "edited").unwrap();
        assert!(!record.restore_markdown(&pdf).unwrap());
        assert_eq!(fs::read_to_string(md_path(&pdf)).unwrap(), "edited");
    }

    #[test]
    fn a_failed_conversion_is_a_document_failure_that_does_not_blame_mineru() {
        let error = ParseError::Document {
            code: CONVERSION_FAILED.into(),
        };
        assert_eq!(error.kind(), "document");
        assert!(!error.retryable());
        assert!(!error.latching());
        assert!(!error.to_string().contains("MinerU"), "{error}");
    }

    #[test]
    fn an_unreadable_spreadsheet_is_a_document_failure_that_does_not_blame_mineru() {
        let error = ParseError::Document {
            code: SHEET_UNREADABLE.into(),
        };
        assert_eq!(error.kind(), "document");
        assert!(!error.retryable());
        assert!(!error.to_string().contains("MinerU"), "{error}");
    }

    #[test]
    fn a_skip_is_neither_retried_nor_latching() {
        let error = ParseError::Cancelled;
        assert_eq!(error.kind(), "cancelled");
        assert!(!error.retryable());
        assert!(!error.latching());
        assert!(error.to_string().starts_with("Skipped"), "{error}");
    }

    #[test]
    fn credential_failures_keep_the_word_the_failure_ui_matches_on() {
        for error in [
            ParseError::MissingCredentials,
            ParseError::RejectedCredentials {
                code: None,
                expired: false,
            },
        ] {
            assert!(error.kind().contains("credential"), "{}", error.kind());
            assert!(!error.retryable());
            assert!(error.latching());
        }
        // Retrying asks the keychain again, and that prompt can be allowed.
        let unreadable = ParseError::UnreadableCredentials("denied".into());
        assert_eq!(unreadable.kind(), "unreadable_credentials");
        assert!(unreadable.retryable());
        assert!(unreadable.latching());
        let shown = unreadable.to_string();
        assert!(!shown.contains("No MinerU API token"), "{shown}");
        assert!(
            shown.contains("MinerU") && shown.contains("denied"),
            "{shown}"
        );
        let broker = ParseError::Broker("caller refused".into());
        assert_eq!(broker.kind(), "credential_broker");
        assert!(broker.retryable());
        assert!(broker.latching());
        let shown = broker.to_string();
        assert!(!shown.contains("keychain refused"), "{shown}");
        assert!(
            shown.contains("MinerU token") && shown.contains("caller refused"),
            "{shown}"
        );
    }

    #[test]
    fn a_skip_mark_holds_until_cleared() {
        let skips = Skips::new();
        let pdf = Path::new("/library/skip-me.pdf");
        assert!(!skips.is_marked(pdf));
        skips.mark(pdf);
        skips.mark(pdf);
        assert!(skips.is_marked(pdf));
        assert!(!skips.is_marked(Path::new("/library/other.pdf")));
        skips.clear(pdf);
        assert!(!skips.is_marked(pdf));
    }

    #[test]
    fn a_stale_fallback_policy_is_not_an_engine() {
        assert_eq!(Engine::parse("auto"), None);
        assert_eq!(Engine::parse("cloud"), Some(Engine::Cloud));
        assert_eq!(Engine::parse("local"), Some(Engine::Local));
    }

    #[test]
    fn oversized_files_are_refused_with_both_numbers() {
        let dir = scratch("size");
        let pdf = dir.join("big.pdf");
        fs::write(&pdf, vec![0u8; 2048]).unwrap();
        let error = check_size(&pdf, 1024).unwrap_err();
        assert!(matches!(
            error,
            ParseError::TooLarge {
                bytes: 2048,
                limit_bytes: 1024
            }
        ));
        assert!(!error.retryable());
        assert!(error.to_string().contains("MB"), "{error}");
        assert_eq!(check_size(&pdf, 4096).unwrap(), 2048);
    }
}

pub mod commands;
pub mod events;
pub mod mineru;
