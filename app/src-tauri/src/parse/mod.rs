//! The seam every PDF parser plugs into.
//!
//! Parsing used to live behind the Python sidecar's HTTP port, which meant the
//! contract was a URL and a JSON shape nobody could typecheck. It is a trait
//! now: MinerU cloud implements it in-process, and a local parse server —
//! shipped as its own repo, spoken to over loopback — implements the same one.
//! Nothing here may assume the cloud; anything cloud-shaped (an API root, a
//! token, an upload ceiling) arrives as configuration or as a parameter.
//!
//! What the seam owns is the *contract*, not the parsing: the on-disk artifact
//! layout, the version that decides whether a file is already done, the error
//! vocabulary the failure UI reads, and the order the artifacts hit the disk.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The version stamped into every `.pages.json`, and the only thing standing
/// between the user's 166 already-parsed files and a full re-parse: a file
/// whose record carries this number is done, whatever produced it. It moves
/// only when the artifacts themselves change shape — not when a backend
/// changes, and not when this app is released.
///
/// The Python parser held the same constant (`sidecar/parser.py`). When that
/// process goes, this is the single source of truth, so it stays at 2.
pub const PARSER_VERSION: u32 = 2;

/// Every parse this app performs is the quality tier now. The field survives
/// because the records on disk have it and the reader must round-trip them;
/// the fast tier that gave it meaning is gone.
pub const MODE: &str = "quality";

// ── Artifact locations ───────────────────────────────────────────────────────
//
// All four names derive from the PDF's stem, and the image directory's *name*
// is also the link prefix written into the markdown (`![](<stem>_images/x.jpg)`
// resolves relative to the `.md` beside it). That coupling is why the two are
// computed here rather than by each backend: a backend that invented its own
// prefix would produce markdown whose images silently fail to load.

pub fn md_path(pdf: &Path) -> PathBuf {
    pdf.with_extension("md")
}

pub fn pages_path(pdf: &Path) -> PathBuf {
    pdf.with_extension("pages.json")
}

pub fn images_dir_for(pdf: &Path) -> PathBuf {
    let stem = pdf.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    pdf.with_file_name(format!("{stem}_images"))
}

/// The link prefix for `images_dir_for(pdf)` — the directory's own name, so
/// markdown written beside the PDF resolves it.
pub fn images_rel_for(pdf: &Path) -> String {
    images_dir_for(pdf)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The `parser_version` recorded beside this PDF, or `None` if it has never
/// been parsed (or the record is unreadable, which amounts to the same thing).
pub fn parsed_version(pdf: &Path) -> Option<u32> {
    let text = fs::read_to_string(pages_path(pdf)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    // A record written before the field existed is version 1, as the Python
    // reader also assumed.
    Some(value.get("parser_version").and_then(|v| v.as_u64()).unwrap_or(1) as u32)
}

/// The record beside this PDF, read back as the type that wrote it.
///
/// Records written by the Python parser deserialise too — the wire shape is
/// deliberately unchanged — which is what lets an already-parsed file be
/// folded into the database without re-parsing it.
pub fn read_record(pdf: &Path) -> Option<ParseOutput> {
    serde_json::from_str(&fs::read_to_string(pages_path(pdf)).ok()?).ok()
}

/// True when this PDF's artifacts are current and it can be skipped.
pub fn is_parsed(pdf: &Path) -> bool {
    parsed_version(pdf).is_some_and(|v| v >= PARSER_VERSION)
}

/// What tier last parsed this PDF: `Some("quality")`, or `None` when it still
/// needs parsing.
///
/// There used to be two disagreeing implementations of this — `paths.rs` read
/// only `mode`, `sidecar/parser.py` also weighed `parser_version` — and both
/// spoke a three-value vocabulary (`quality | fast | none`) that the retired
/// fast tier gave meaning to. With one tier left, the question is binary:
/// `mode == "quality"` means done, and **anything else — the record missing,
/// unreadable, or naming another mode — means parse it**, because there is no
/// weaker output left to prefer over doing the work.
///
/// Two rules here are hard-won and must survive:
///
/// * **Parse state is never inferred from the images directory**, and not from
///   the `.md` either. The images directory is created *before* a parse runs
///   and survives a crash, so an interrupted pass used to leave an empty folder
///   that read as "already done" and pinned the file to fast markdown forever.
///   The `.md` is derived from the record, not evidence about it — markdown
///   with no record is markdown nothing finished writing.
/// * **No `parser_version` check on the quality path.** The old version branch
///   guarded *fast* output only; quality output is accepted whatever version
///   wrote it, deliberately. Re-running MinerU across the library costs hours,
///   so a version check here would invalidate all 166 already-parsed files on
///   the next bump — the exact outcome `parser_version` exists to avoid.
pub fn parse_mode(pdf: &Path) -> Option<&'static str> {
    let text = fs::read_to_string(pages_path(pdf)).ok()?;
    let record: serde_json::Value = serde_json::from_str(&text).ok()?;
    (record.get("mode").and_then(|v| v.as_str()) == Some(MODE)).then_some(MODE)
}

// ── The parsed document ──────────────────────────────────────────────────────

/// One page's markdown, keyed by its 1-based page number.
///
/// `page_no` is the join key retrieval rests on: a hit in one system resolves
/// to a page in the other only because both sides agree on this number. A
/// backend that returned pages in its own order, or skipped the empty ones,
/// would break that join — which is why `ParseOutput::new` normalises rather
/// than trusting what it is handed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsePage {
    pub page_no: u32,
    pub markdown: String,
}

/// Exactly the `.pages.json` on disk, plus one field that never goes there.
///
/// Field names are the wire format: snake_case, no rename attributes. The
/// records already in the library were written by Python and must keep
/// deserialising.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseOutput {
    /// The PDF's file name, not its path — the record travels with the folder.
    pub pdf: String,
    pub mode: String,
    pub parser_version: u32,
    pub page_count: u32,
    pub pages: Vec<ParsePage>,
    /// Absent in records from the retired fast tier, so it is omitted rather
    /// than written as null — a reader that sees the key can trust it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// How many images the parse extracted. Progress and logging want it; the
    /// record on disk never had it, and adding it would make files written by
    /// this app differ from the ones already in the library.
    #[serde(skip)]
    pub image_count: u32,
}

impl ParseOutput {
    /// Build a record that satisfies the contract regardless of what the
    /// backend produced: exactly one entry per page `1..=page_count`, in
    /// order, with `""` where a page yielded nothing. Backends that batch,
    /// split or parallelise return pages out of order and drop the blank ones;
    /// normalising here means every one of them can be sloppy in the same way.
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
            pdf: pdf.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            mode: MODE.to_string(),
            parser_version: PARSER_VERSION,
            page_count,
            pages: slots
                .into_iter()
                .enumerate()
                .map(|(i, markdown)| ParsePage { page_no: i as u32 + 1, markdown })
                .collect(),
            backend,
            image_count,
        }
    }

    /// The full-document markdown: pages joined by a blank line, which is what
    /// the `.md` beside the PDF has always contained.
    pub fn document_markdown(&self) -> String {
        self.pages.iter().map(|p| p.markdown.as_str()).collect::<Vec<_>>().join("\n\n")
    }

    /// Put the artifacts on disk, in the one order that is safe.
    ///
    /// The Python wrote the `.md` and then the `.pages.json`, both in place and
    /// neither atomically (`sidecar/parser.py:367-369`). A crash in that window
    /// left markdown with no record: the file looked parsed to a human reading
    /// the folder and unparsed to the app, and the next run overwrote it.
    ///
    /// `.pages.json` is the *only* evidence a parse finished, so it lands last
    /// and it lands atomically — temp file, fsync, rename. Everything before it
    /// can be re-done; once it exists, everything it describes is already
    /// there.
    pub fn write(&self, pdf: &Path, images: ImageStaging) -> Result<(), ParseError> {
        // 1. Images first: swap the staged directory in, discarding whatever a
        //    previous parse left. Same filesystem by construction (see
        //    `ImageStaging::begin`), so the rename is atomic and cheap.
        images.commit()?;

        // 2. The markdown the images belong to.
        let md = md_path(pdf);
        fs::write(&md, self.document_markdown())
            .map_err(|e| ParseError::Io(format!("write {}: {e}", md.display())))?;

        // 3. The record, last and atomic.
        //    serde_json emits UTF-8 without escaping non-ASCII, matching the
        //    Python's `ensure_ascii=False`; the records on disk are mixed
        //    Greek, CJK and maths and must stay byte-comparable.
        let final_path = pages_path(pdf);
        let body = serde_json::to_vec(self)
            .map_err(|e| ParseError::Io(format!("encode {}: {e}", final_path.display())))?;
        let tmp = final_path.with_extension(format!("json.tmp{}", std::process::id()));
        let mut file = fs::File::create(&tmp)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", tmp.display())))?;
        let staged = file
            .write_all(&body)
            .and_then(|()| file.sync_all())
            .map_err(|e| ParseError::Io(format!("write {}: {e}", tmp.display())));
        drop(file);
        if let Err(error) = staged {
            fs::remove_file(&tmp).ok();
            return Err(error);
        }
        fs::rename(&tmp, &final_path).map_err(|e| {
            fs::remove_file(&tmp).ok();
            ParseError::Io(format!("rename into {}: {e}", final_path.display()))
        })
    }
}

// ── Image staging ────────────────────────────────────────────────────────────

/// A scratch directory for extracted images, swapped into place only when the
/// whole parse succeeded.
///
/// The caller opens one, hands `dir()` and `rel()` to `Parser::parse`, and
/// passes it to `ParseOutput::write`. Dropped without committing — a failed
/// parse, an error on the way out, a panic — it deletes itself and the
/// previous parse's artifacts are untouched. That is the whole point: a failure
/// must never leave a file *less* parsed than it was.
pub struct ImageStaging {
    staged: PathBuf,
    destination: PathBuf,
    rel: String,
}

impl ImageStaging {
    /// Stage beside the PDF, never in the system temp dir: the final move has
    /// to be a rename, and a rename across filesystems fails. The library can
    /// sit on an external drive, so "same directory" is the only guarantee
    /// available. The dot prefix keeps a half-written parse out of the file
    /// listings the app builds from this folder.
    pub fn begin(pdf: &Path) -> Result<Self, ParseError> {
        let destination = images_dir_for(pdf);
        let name = destination
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let scratch = pdf.with_file_name(format!(".{name}-staging-{}-{stamp}", std::process::id()));
        fs::create_dir_all(scratch.join(&name))
            .map_err(|e| ParseError::Io(format!("stage {}: {e}", scratch.display())))?;
        Ok(Self { staged: scratch.join(&name), destination, rel: name })
    }

    /// Where the backend writes extracted images.
    pub fn dir(&self) -> &Path {
        &self.staged
    }

    /// The link prefix the backend writes into the markdown. It is the *final*
    /// directory's name, not the scratch one — the markdown is written after
    /// the swap and must point at where the images end up.
    pub fn rel(&self) -> &str {
        &self.rel
    }

    /// Swap the staged images into place. Consumes the staging, so `Drop`
    /// runs immediately afterwards and clears the scratch wrapper either way —
    /// on success it is empty, on failure it still holds the images nobody
    /// wants.
    fn commit(self) -> Result<(), ParseError> {
        // A re-parse renumbers crops, so the old directory is replaced rather
        // than merged into: anything it still held would be unreferenced.
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

/// Reported while a parse runs. `total_pages` can be zero before the backend
/// knows how long the document is; the UI shows a count, not a fraction, until
/// it isn't.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Progress {
    pub pages_done: u32,
    pub total_pages: u32,
    pub backend: &'static str,
}

/// What a backend says about itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub backend: String,
    pub parser_version: u32,
    pub ready: bool,
}

impl Health {
    /// The version handshake. A backend that stamps a different
    /// `parser_version` writes artifacts this app cannot read as its own, so it
    /// is **refused, not warned about** — a warning here means a library of
    /// files that each look parsed and none of which are.
    ///
    /// This looks redundant today and is not dead code: the only backend in
    /// this process is the in-process cloud client, which compares its own
    /// constant against itself and can never disagree. The check exists for the
    /// *other* implementation — the local parse server, which ships from a
    /// separate repo, updates on its own schedule, and absolutely can be a
    /// version ahead of or behind the app talking to it. Deleting it would
    /// remove the only thing that catches that.
    pub fn check(&self) -> Result<(), ParseError> {
        if self.parser_version != PARSER_VERSION {
            return Err(ParseError::VersionMismatch {
                app: PARSER_VERSION,
                backend: self.parser_version,
            });
        }
        if !self.ready {
            return Err(ParseError::NotReady { backend: self.backend.clone() });
        }
        Ok(())
    }
}

/// Ask a backend whether it can be used, and refuse it if it disagrees about
/// the artifact version. Every call site that is about to parse goes through
/// here rather than calling `health()` and reading the fields itself.
pub fn preflight(parser: &dyn Parser) -> Result<Health, ParseError> {
    let health = parser.health();
    health.check()?;
    Ok(health)
}

// ── The trait ────────────────────────────────────────────────────────────────

pub trait Parser: Send + Sync {
    /// Parse one PDF.
    ///
    /// `images_dir` is where extracted images go and `images_rel` is the prefix
    /// to write into the markdown that references them. Both are passed in
    /// rather than derived from `pdf`, because the caller — not the backend —
    /// decides the staging strategy: `ImageStaging` hands over a scratch
    /// directory whose name deliberately differs from the prefix, so a parse
    /// that dies halfway cannot have overwritten anything.
    ///
    /// `on_progress` may be called from whatever thread the backend is using.
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

/// Why a parse did not happen.
///
/// There is no fallback tier any more: a failure here means that file has no
/// markdown until somebody retries it, so the UI has to say something true
/// about *why*, and whether waiting, retrying, or fixing a setting is the move.
/// The variants exist to keep those three answers distinguishable.
///
/// **No variant carries server response text.** MinerU's error bodies can
/// contain the signed URLs it issued for the upload; those would end up in the
/// UI, in logs, and in whatever the user pastes into a bug report. Errors carry
/// the *code*, never the body — the Python client had the same rule
/// (`sidecar/mineru_cloud.py`).
#[derive(Debug, Clone)]
pub enum ParseError {
    /// No token is stored. Nothing will parse until one is.
    MissingCredentials,
    /// The backend refused the token itself. Latching: every other file in the
    /// queue would hit the identical rejection, so a run that sees this stops
    /// rather than marching the whole library into the same wall.
    RejectedCredentials { code: Option<String>, expired: bool },
    /// The daily allowance is spent. Unlike a rejected token this repairs
    /// itself at the next reset, so it is worth retrying — later.
    QuotaExhausted,
    /// Could not reach the backend at all. Holds the *local* transport error
    /// (a connect/timeout message), never a response body.
    Offline(String),
    /// Over the backend's upload ceiling. Permanent for this file, and the
    /// limit is known before anything is sent — so this is a refusal with a
    /// number in it, not a failed attempt.
    TooLarge { bytes: u64, limit_bytes: u64 },
    /// The backend ran and could not make sense of this document. Scoped to
    /// this file: the rest of the queue is fine and should keep going.
    Document { code: String },
    /// The backend writes a different artifact version than this app reads.
    VersionMismatch { app: u32, backend: u32 },
    /// The backend answered but is not accepting work yet.
    NotReady { backend: String },
    /// Writing the artifacts failed — disk full, permissions, a vanished
    /// directory. The parse itself may well have succeeded.
    Io(String),
}

impl ParseError {
    /// The machine-readable discriminant that travels with a failure event.
    ///
    /// The prose in `Display` is for a student and may be reworded at any time;
    /// this is what the failure UI branches on, so it is a frozen vocabulary.
    /// `app/src/lib/parseState.ts` matches `/credential|token/i` against it to
    /// decide whether a failure is worth pointing at Settings, which is why
    /// both credential variants keep that word in their name.
    pub fn kind(&self) -> &'static str {
        match self {
            ParseError::MissingCredentials => "missing_credentials",
            ParseError::RejectedCredentials { .. } => "rejected_credentials",
            ParseError::QuotaExhausted => "quota_exhausted",
            ParseError::Offline(_) => "offline",
            ParseError::TooLarge { .. } => "too_large",
            ParseError::Document { .. } => "document",
            ParseError::VersionMismatch { .. } => "version_mismatch",
            ParseError::NotReady { .. } => "not_ready",
            ParseError::Io(_) => "io",
        }
    }

    /// Could retrying *this file*, unchanged, ever succeed?
    ///
    /// False means retrying is pointless until something else changes: a new
    /// token, a different file, a matching backend. The distinction drives
    /// whether a failure offers a retry at all.
    pub fn retryable(&self) -> bool {
        match self {
            // Transport hiccups and local write failures are the ordinary
            // retry cases; quota is a retry once the day rolls over, and a
            // backend that isn't ready may be in a moment.
            ParseError::Offline(_)
            | ParseError::Io(_)
            | ParseError::QuotaExhausted
            | ParseError::NotReady { .. } => true,
            // A missing or refused token, an oversized or malformed document,
            // a backend from another version: all need an intervention first.
            ParseError::MissingCredentials
            | ParseError::RejectedCredentials { .. }
            | ParseError::TooLarge { .. }
            | ParseError::Document { .. }
            | ParseError::VersionMismatch { .. } => false,
        }
    }

    /// Does this condemn every other file too?
    ///
    /// A latching failure should stop the run: the alternative is a hundred
    /// identical errors and a hundred files marked failed for a reason that had
    /// nothing to do with them.
    pub fn latching(&self) -> bool {
        matches!(
            self,
            ParseError::MissingCredentials
                | ParseError::RejectedCredentials { .. }
                | ParseError::QuotaExhausted
                | ParseError::VersionMismatch { .. }
        )
    }
}

/// These strings are shown to a student, not to a developer: they say what
/// happened and what would change it, and they never contain a URL, a token or
/// anything the server said.
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::MissingCredentials => {
                write!(f, "No MinerU API token is saved — add one in Settings to parse PDFs.")
            }
            ParseError::RejectedCredentials { code, expired } => {
                let code = code.as_deref().map(|c| format!(" ({c})")).unwrap_or_default();
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
            ParseError::Offline(_) => {
                write!(f, "Could not reach MinerU. Check your connection, then try again.")
            }
            ParseError::TooLarge { bytes, limit_bytes } => write!(
                f,
                "This PDF is {} and MinerU accepts files up to {}, so it was not sent.",
                megabytes(*bytes),
                megabytes(*limit_bytes)
            ),
            ParseError::Document { code } => write!(
                f,
                "MinerU could not read this PDF (error {code}). Other files are unaffected."
            ),
            // Both numbers, always: "version mismatch" alone tells the user
            // nothing about which side to update.
            ParseError::VersionMismatch { app, backend } => write!(
                f,
                "The parse backend writes version {backend} files but this app reads version \
                 {app}. Update whichever is older before parsing."
            ),
            ParseError::NotReady { backend } => {
                write!(f, "The {backend} parser is not ready yet. Try again in a moment.")
            }
            ParseError::Io(detail) => write!(f, "Could not save the parsed output: {detail}"),
        }
    }
}

impl std::error::Error for ParseError {}

fn megabytes(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / (1024.0 * 1024.0))
}

/// Refuse an oversized file before uploading it. The limit is the *backend's*,
/// passed in — nothing here knows what MinerU's ceiling is.
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

/// Which backend *is* the parser.
///
/// Note this is not the old vocabulary. The `backend` field in the same blob
/// took `local | cloud | auto`, and all three named a *fallback policy* over
/// the Python sidecar: "auto" meant "cloud, dropping to the local MinerU when
/// the cloud is unavailable", and "local" meant that Python parser specifically.
/// There is no fallback and no Python any more, so those values cannot be
/// reinterpreted — "local" now means a completely different program, and
/// silently promoting a stale "local" would point the app at a parse server
/// that is not installed.
///
/// So the two live under different keys and a stale `backend` value — `"auto"`
/// included — is **ignored entirely**: only `engine` selects a parser, and its
/// absence (which is every install today) means `Cloud`. See `parse_config`.
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
    /// The macOS keychain, via `crate::mineru`. The token never enters SQLite
    /// or the WebView, so it is named here rather than carried here.
    Keychain,
    /// Loopback to a server on this machine: nothing to authenticate.
    None,
}

impl CredentialSource {
    pub fn token(self) -> Option<String> {
        match self {
            CredentialSource::Keychain => crate::mineru::stored_api_key(),
            CredentialSource::None => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseConfig {
    pub engine: Engine,
    /// API root for the chosen engine, overridable from the settings blob so a
    /// local server on a non-default port can be reached without a release.
    pub base_url: String,
    pub credentials: CredentialSource,
}

/// MinerU's published API root — the same one the Python client used.
pub const CLOUD_BASE_URL: &str = "https://mineru.net/api/v4";

/// The local parse server's default origin. It is the port the Python sidecar
/// is vacating, which keeps one number in the user's firewall rules rather
/// than two.
pub const LOCAL_BASE_URL: &str = "http://127.0.0.1:9547";

/// The `parse` row, as far as this seam cares about it. Every field is
/// optional: the blob still carries two dead keys from the Python sidecar
/// (`memoryCapMb` and the legacy `backend`), left there deliberately rather
/// than migrated out, and they must survive being read here. See the note in
/// `app/src/lib/db.ts`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct StoredParseSettings {
    /// Read as a string, not as `Engine`: a typo or a stale value must cost us
    /// this one field, not the whole row.
    engine: Option<String>,
    engine_url: Option<String>,
}

/// Read the backend selection out of the `settings` table's `parse` row.
///
/// Unreadable, missing or nonsense settings all resolve to the cloud default —
/// a user whose database is in an odd state should still get their PDFs parsed.
pub fn parse_config() -> ParseConfig {
    let stored = stored_settings().unwrap_or_default();
    let engine = stored.engine.as_deref().and_then(Engine::parse).unwrap_or(Engine::Cloud);
    let base_url = stored
        .engine_url
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| {
            match engine {
                Engine::Cloud => CLOUD_BASE_URL,
                Engine::Local => LOCAL_BASE_URL,
            }
            .to_string()
        });
    let credentials = match engine {
        Engine::Cloud => CredentialSource::Keychain,
        Engine::Local => CredentialSource::None,
    };
    ParseConfig { engine, base_url, credentials }
}

/// The parser this install is configured for.
///
/// Every caller that is about to parse comes through here rather than naming a
/// backend: the engine is a setting, and the one place that reads it is this
/// function. Construction is where a missing token surfaces — `MinerUCloud`
/// refuses to exist without one — so the caller gets `MissingCredentials`
/// before a file has been touched.
pub fn backend() -> Result<Box<dyn Parser>, ParseError> {
    match parse_config().engine {
        Engine::Cloud => Ok(Box::new(mineru::client::MinerUCloud::from_config()?)),
        // The local parse server ships from its own repo and has no client in
        // this process yet. Nothing selects it today (`engine` is absent on
        // every install, which means `Cloud`), so this is a hand-edited
        // setting pointing at something that is not here.
        Engine::Local => Err(ParseError::NotReady { backend: "local".into() }),
    }
}

/// The `parse` row, decoded. One `SELECT`, through the one reader that is safe
/// to call from a synchronous function no matter who is above it.
fn stored_settings() -> Option<StoredParseSettings> {
    // Shared with the embed seam, and deliberately not a `block_on`: see
    // `store::setting_blocking`. Nothing async reads this row today, but the
    // safety of a helper must not be a fact about its callers.
    serde_json::from_str(&crate::store::setting_blocking("parse")?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug this reader exists to prevent, pinned in the context that hit
    /// it: `embed_settings` is an `async` Tauri command, so it runs on a
    /// runtime worker thread, and a `block_on` there panics the task without
    /// rejecting the promise — Settings → Library rendered dashes forever.
    ///
    /// Reading a row is not what is under test; surviving the call is. The
    /// assertion is deliberately weak (a config comes back at all) because
    /// this must pass on a machine with no database.
    #[test]
    fn the_config_is_readable_from_inside_an_async_runtime() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let base = runtime.block_on(async { parse_config().base_url });
        assert!(!base.is_empty(), "a backend always resolves to some API root");
    }

    /// A scratch library folder. The staging rename must be same-filesystem,
    /// so the tests use a real directory rather than faking paths.
    fn scratch(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("oculus-parse-{name}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
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
        assert_eq!(pages_path(pdf), Path::new("/library/subj/Lecture 3.pages.json"));
        assert_eq!(images_dir_for(pdf), Path::new("/library/subj/Lecture 3_images"));
        assert_eq!(images_rel_for(pdf), "Lecture 3_images");
    }

    #[test]
    fn pages_are_ordered_and_gap_filled() {
        let pdf = Path::new("/library/Lecture 3.pdf");
        let out = ParseOutput::new(
            pdf,
            4,
            vec![
                ParsePage { page_no: 3, markdown: "three".into() },
                ParsePage { page_no: 1, markdown: "one".into() },
                // Out of range: a backend that split the document and got its
                // offsets wrong must not be able to corrupt the join key.
                ParsePage { page_no: 9, markdown: "nine".into() },
            ],
            Some("mineru-cloud".into()),
            2,
        );
        assert_eq!(out.page_count, 4);
        assert_eq!(out.pages.iter().map(|p| p.page_no).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
        assert_eq!(out.pages[1].markdown, "");
        // Blank pages still occupy a slot in the joined document, as the
        // Python join did — the `.md` and the page records stay in step.
        assert_eq!(out.document_markdown(), "one\n\n\n\nthree\n\n");
    }

    #[test]
    fn record_keeps_the_python_wire_shape() {
        let pdf = Path::new("/library/Lecture 3.pdf");
        let out = ParseOutput::new(pdf, 1, vec![ParsePage { page_no: 1, markdown: "ψ".into() }], None, 0);
        let json = serde_json::to_string(&out).unwrap();
        // Non-ASCII unescaped, as `ensure_ascii=False` wrote it.
        assert!(json.contains("\"ψ\""), "{json}");
        // No backend key at all when there is no backend, and never an
        // image_count — neither appears in the files already in the library.
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
            vec![ParsePage { page_no: 1, markdown: "![](Lecture 3_images/new.jpg)".into() }],
            Some("mineru-cloud".into()),
            1,
        );
        out.write(&pdf, staging).unwrap();

        assert!(images_dir_for(&pdf).join("new.jpg").is_file());
        assert!(!images_dir_for(&pdf).join("old.jpg").exists());
        assert!(md_path(&pdf).is_file());
        assert_eq!(parsed_version(&pdf), Some(PARSER_VERSION));
        assert!(is_parsed(&pdf));
        // No temp record left behind.
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".tmp") || n.starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");

        fs::remove_dir_all(&dir).ok();
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

        fs::remove_dir_all(&dir).ok();
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

        // Quality is done regardless of the version that wrote it — a version
        // check here would re-parse the whole library on the next bump.
        fs::write(pages_path(&pdf), r#"{"mode":"quality","parser_version":1}"#).unwrap();
        assert_eq!(parse_mode(&pdf), Some("quality"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_stale_fallback_policy_is_not_an_engine() {
        // The old `backend` vocabulary — including "auto", which meant "cloud
        // with a local fallback" — must never resolve to an engine.
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
        assert!(matches!(error, ParseError::TooLarge { bytes: 2048, limit_bytes: 1024 }));
        assert!(!error.retryable());
        // Never a raw byte count in front of a student.
        assert!(error.to_string().contains("MB"), "{error}");
        assert_eq!(check_size(&pdf, 4096).unwrap(), 2048);
        fs::remove_dir_all(&dir).ok();
    }
}

/// The one backend today. See `parse/mineru/mod.rs`.
pub mod mineru;

/// `parse-status`, the one event this path emits. See `parse/events.rs`.
pub mod events;
