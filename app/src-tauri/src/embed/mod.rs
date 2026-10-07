//! The seam every embedder plugs into: Voyage implements it in-process, and a
//! local embedder would implement the same trait. Nothing here may assume the
//! cloud; an API root, a key or a rate limit arrives as configuration.
//!
//! The seam owns the contract — the on-disk record, the vector space every
//! row in `pages` must belong to, the encoding of the blob column, and an
//! error vocabulary that mirrors `parse/mod.rs` so one failure UI reads both.
//!
//! What gets embedded is the rendered page image, never extracted text (see
//! `docs/retrieval.md`), which is why `embed` takes a PDF and a
//! page count rather than a string.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use half::f16;
use serde::{Deserialize, Serialize};

/// The stored vector width: 1024-byte blobs of 512 x f16 in `pages.embedding`,
/// and a Matryoshka width the cloud model honours via `output_dimension`.
/// Changing it means migrating every row.
pub const EMBED_DIM: usize = 512;

/// The model that defines the space, stamped into every record. It names the
/// space, not the vendor: a local backend producing the same vectors would
/// claim this same id. A mismatch is a different geometry (see `Health::check`).
pub const EMBED_MODEL: &str = "voyage-multimodal-3.5";

/// How a vector is written down: little-endian float16, base64 in the record,
/// raw bytes in the blob column.
pub const EMBED_DTYPE: &str = "float16";

/// Names the query side of the asymmetry (`input_type: "query"` against
/// `"document"`). Compared, never sent. Documents and queries must use
/// opposite sides; using one for both is undetectable and just ranks worse.
pub const QUERY_INSTRUCTION: &str = "input_type:query";

// ── Artifact location ────────────────────────────────────────────────────────
//
// One file beside the PDF, named off its stem — the same rule the parse
// artifacts follow, so a library folder can be copied whole.

pub fn emb_path(pdf: &Path) -> PathBuf {
    pdf.with_extension("emb.json")
}

/// The record beside this PDF. Records from another model still deserialise
/// (so a library scan never blows up on one) but `is_embedded` rejects them.
pub fn read_record(pdf: &Path) -> Option<EmbedOutput> {
    serde_json::from_str(&fs::read_to_string(emb_path(pdf)).ok()?).ok()
}

/// True when this PDF's vectors are in this app's space and cover every page.
///
/// Model, dim and instruction must all match — a vector from another model is
/// a different geometry, not older output. Coverage matters too: a cloud run
/// can lose a page to a rate limit or a refusal, and `EmbedOutput::new` drops
/// it rather than inventing a vector, so without the count check that page
/// would never be searchable or retried. The expected count comes from the
/// parse record; with none, identity alone has to do.
pub fn is_embedded(pdf: &Path) -> bool {
    let Some(record) = read_record(pdf) else {
        return false;
    };
    record.is_current(crate::parse::read_record(pdf).map(|parsed| parsed.page_count))
}

// ── Vector encoding ──────────────────────────────────────────────────────────
//
// Voyage has no f16 `output_dtype`; its base64 `output_encoding` is f32. Every
// backend hands over `&[f32]` and the narrowing to f16 happens here, once.

/// Truncate to `EMBED_DIM`, re-normalise, pack little-endian f16.
///
/// A Matryoshka prefix is a valid embedding, so a longer vector is truncated;
/// a shorter one is a different space and is refused, not padded. Truncation
/// breaks unit length and Voyage's own norm is only ≈1, so this re-normalises:
/// `retrieval.rs` ranks by a raw dot product, and an unnormalised vector would
/// silently let long pages win.
pub fn encode_vector(vector: &[f32]) -> Result<String, EmbedError> {
    let bytes = pack_vector(vector)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// The raw bytes that go into `pages.embedding` — exactly what
/// `encode_vector` base64s, so the blob and the record never disagree.
pub fn pack_vector(vector: &[f32]) -> Result<Vec<u8>, EmbedError> {
    if vector.len() < EMBED_DIM {
        return Err(EmbedError::ModelMismatch {
            app_model: EMBED_MODEL.to_string(),
            app_dim: EMBED_DIM,
            backend_model: EMBED_MODEL.to_string(),
            backend_dim: vector.len(),
        });
    }
    let head = &vector[..EMBED_DIM];
    let norm = head.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>().sqrt();
    // A zero vector cannot be normalised and would rank 0 against every query.
    if !norm.is_finite() || norm <= 0.0 {
        return Err(EmbedError::Document { code: "zero_vector".into() });
    }
    let mut bytes = Vec::with_capacity(EMBED_DIM * 2);
    for value in head {
        bytes.extend_from_slice(&f16::from_f32((*value as f64 / norm) as f32).to_le_bytes());
    }
    Ok(bytes)
}

/// base64 f16 -> f32, the read side of `encode_vector`; tests use it to check
/// the round trip.
#[cfg(test)]
pub fn decode_vector(encoded: &str) -> Result<Vec<f32>, EmbedError> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| EmbedError::Document { code: format!("bad base64: {e}") })?;
    Ok(unpack_vector(&raw))
}

/// float16 little-endian -> f32. Vectors are stored normalised, so a dot
/// product is cosine similarity.
pub fn unpack_vector(raw: &[u8]) -> Vec<f32> {
    raw.chunks_exact(2).map(|c| f16::from_le_bytes([c[0], c[1]]).to_f32()).collect()
}

// ── The embedded document ────────────────────────────────────────────────────

/// One page's vector, keyed by its 1-based page number — the join key that
/// resolves a hit to markdown in `.pages.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedPage {
    pub page_no: u32,
    /// base64 of little-endian f16, `EMBED_DIM` wide, kept as the wire string
    /// so a record round-trips byte-for-byte.
    pub vector: String,
}

impl EmbedPage {
    /// Encode one page's vector, normalising it on the way in.
    pub fn new(page_no: u32, vector: &[f32]) -> Result<Self, EmbedError> {
        Ok(Self { page_no, vector: encode_vector(vector)? })
    }
}

/// Exactly the `.emb.json` on disk; field names are the wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedOutput {
    /// The PDF's file name, not its path — the record travels with the folder.
    pub pdf: String,
    pub model: String,
    pub dim: usize,
    pub dtype: String,
    pub instruction: String,
    /// How many pages were embedded — **not** the document length, unlike
    /// `.pages.json`'s `page_count`, which `is_embedded` checks coverage against.
    pub page_count: usize,
    pub pages: Vec<EmbedPage>,
}

impl EmbedOutput {
    /// Identity and coverage checked against the parse record already read by
    /// the caller, so ingest does not decode either artifact twice.
    pub(crate) fn is_current(&self, expected_pages: Option<u32>) -> bool {
        self.model == EMBED_MODEL && self.dim == EMBED_DIM
            && self.instruction == QUERY_INSTRUCTION
            && expected_pages.is_none_or(|count| self.pages.len() >= count as usize)
    }

    /// Pages in ascending order, one per page number, none outside
    /// `1..=page_count`. Missing pages are dropped, not filled: unlike an empty
    /// markdown string there is no empty vector, and a zero one scores 0
    /// against every query while looking indexed.
    pub fn new(pdf: &Path, page_count: u32, pages: Vec<EmbedPage>) -> Self {
        let mut kept: Vec<EmbedPage> = Vec::with_capacity(pages.len());
        for page in pages {
            if page.page_no >= 1
                && page.page_no <= page_count
                && !kept.iter().any(|existing| existing.page_no == page.page_no)
            {
                kept.push(page);
            }
        }
        kept.sort_by_key(|page| page.page_no);
        Self {
            pdf: pdf.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            model: EMBED_MODEL.to_string(),
            dim: EMBED_DIM,
            dtype: EMBED_DTYPE.to_string(),
            instruction: QUERY_INSTRUCTION.to_string(),
            page_count: kept.len(),
            pages: kept,
        }
    }

    /// Put the record on disk atomically (temp file, fsync, rename): its
    /// existence is the only evidence the embedding finished, and a torn write
    /// could otherwise read as a complete record for a partly-indexed file.
    pub fn write(&self, pdf: &Path) -> Result<(), EmbedError> {
        let final_path = emb_path(pdf);
        let body = serde_json::to_vec(self)
            .map_err(|e| EmbedError::Io(format!("encode {}: {e}", final_path.display())))?;
        let tmp = final_path.with_extension(format!("json.tmp{}", std::process::id()));
        crate::atomic_write::write(&final_path, &tmp, &body).map_err(EmbedError::Io)
    }
}

// ── Progress and health ──────────────────────────────────────────────────────

/// Reported while a run works through a document; `total_pages` may be zero
/// until the backend knows. Same shape as `parse::Progress` so the sidebar
/// renders both with one component.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Progress {
    pub pages_done: u32,
    pub total_pages: u32,
    pub backend: &'static str,
    /// Set while every request in flight is held back by a rate limit, so a
    /// row that is not moving can say why and for how long.
    pub waiting: Option<Wait>,
}

/// A request held back by a rate limit, and when it is expected to go out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Wait {
    /// Epoch milliseconds.
    pub until_ms: u64,
    pub limiter: Limiter,
}

impl Wait {
    pub fn after(duration: std::time::Duration, limiter: Limiter) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Self { until_ms: (now + duration).as_millis() as u64, limiter }
    }
}

/// Which limit holds the request: the server's refusal, or our own pacing to
/// one of the account's per-minute ceilings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Limiter {
    /// Voyage answered 429.
    Throttled,
    Requests { per_minute: u32 },
    Tokens { per_minute: u32 },
}

impl Limiter {
    /// A lower-case phrase for the pipeline row, e.g. "pacing to Voyage's
    /// 3 requests/min limit".
    pub fn describe(&self) -> String {
        match self {
            Limiter::Throttled => "rate-limited by Voyage".to_string(),
            Limiter::Requests { per_minute } => {
                format!("pacing to Voyage's {} requests/min limit", compact(*per_minute))
            }
            Limiter::Tokens { per_minute } => {
                format!("pacing to Voyage's {} tokens/min limit", compact(*per_minute))
            }
        }
    }
}

/// 3 -> "3", 10000 -> "10K", 2000000 -> "2M".
fn compact(n: u32) -> String {
    match n {
        n if n >= 1_000_000 && n % 1_000_000 == 0 => format!("{}M", n / 1_000_000),
        n if n >= 1_000 && n % 1_000 == 0 => format!("{}K", n / 1_000),
        n => n.to_string(),
    }
}

/// What a backend says about itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub backend: String,
    pub model: String,
    pub dim: usize,
    pub ready: bool,
}

impl Health {
    /// The space handshake: a backend in a different space is refused before a
    /// single vector is written. Two spaces in one `pages` table produce no
    /// error at all — every dot product returns a number and the ranking is
    /// noise — so this is stricter than `parse::Health::check`.
    pub fn check(&self) -> Result<(), EmbedError> {
        if self.model != EMBED_MODEL || self.dim != EMBED_DIM {
            return Err(EmbedError::ModelMismatch {
                app_model: EMBED_MODEL.to_string(),
                app_dim: EMBED_DIM,
                backend_model: self.model.clone(),
                backend_dim: self.dim,
            });
        }
        if !self.ready {
            return Err(EmbedError::NotReady { backend: self.backend.clone() });
        }
        Ok(())
    }
}

/// Ask a backend whether it can be used, refusing one in another space. Every
/// call site about to embed goes through here.
pub fn preflight(embedder: &dyn Embedder) -> Result<Health, EmbedError> {
    let health = embedder.health();
    health.check()?;
    Ok(health)
}

// ── The trait ────────────────────────────────────────────────────────────────

pub trait Embedder: Send + Sync {
    /// Embed every page of one PDF.
    ///
    /// `page_count` comes from the parse record and bounds `EmbedOutput::new`;
    /// a backend may return fewer pages, never more. `on_progress` may be
    /// called from any thread, and in jumps when pages are batched.
    fn embed(
        &self,
        pdf: &Path,
        page_count: u32,
        on_progress: &dyn Fn(Progress),
    ) -> Result<EmbedOutput, EmbedError>;

    /// Embed a search query — the other side of `QUERY_INSTRUCTION`. Returns
    /// floats, since a query vector is never stored, but still normalised so
    /// the dot product is a cosine.
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbedError>;

    fn health(&self) -> Health;
}

// ── Failure ──────────────────────────────────────────────────────────────────

/// Why an embedding did not happen. Same `kind` / `retryable` / `latching`
/// vocabulary as `ParseError`, so one failure UI reads both seams.
///
/// No variant carries server response text: a body can echo the request (the
/// base64 page image) or the account's billing state. Codes only.
#[derive(Debug, Clone)]
pub enum EmbedError {
    /// No API key is stored.
    MissingCredentials,
    /// The backend refused the key. Latching: every file would hit it.
    RejectedCredentials { code: Option<String>, expired: bool },
    /// Throttled per minute. On the free tier this is the steady state of a
    /// working run (limits: `voyage::ledger`), so it is retryable and never
    /// latching — the backend waits and carries on; this only reports the wait.
    RateLimited { retry_after_secs: Option<u64> },
    /// The allowance is spent. Repairs itself at the reset, so retryable;
    /// latching, because every file draws on the same allowance.
    QuotaExhausted,
    /// We stopped, not Voyage: the Settings → Library spend guard (`percent` of
    /// the free pixel grant) was reached. Separate from `QuotaExhausted`
    /// because a setting does not repair itself, so it is not retryable.
    BudgetReached { percent: u8 },
    /// Could not reach the backend. Holds the local transport error only.
    Offline(String),
    /// This document would not rasterise or a page came back empty. Scoped to
    /// the file; the rest of the queue keeps going.
    Document { code: String },
    /// The backend embeds into a different space (see `Health::check`).
    ModelMismatch { app_model: String, app_dim: usize, backend_model: String, backend_dim: usize },
    /// The backend answered but is not accepting work yet.
    NotReady { backend: String },
    /// Writing the record failed. The embedding may have succeeded — and been
    /// paid for.
    Io(String),
}

impl EmbedError {
    /// The frozen discriminant the failure UI branches on (`Display` prose may
    /// change). `app/src/lib/parseState.ts` matches `/credential|token/i`, so
    /// both credential kinds keep that word.
    pub fn kind(&self) -> &'static str {
        match self {
            EmbedError::MissingCredentials => "missing_credentials",
            EmbedError::RejectedCredentials { .. } => "rejected_credentials",
            EmbedError::RateLimited { .. } => "rate_limited",
            EmbedError::QuotaExhausted => "quota_exhausted",
            EmbedError::BudgetReached { .. } => "budget_reached",
            EmbedError::Offline(_) => "offline",
            EmbedError::Document { .. } => "document",
            EmbedError::ModelMismatch { .. } => "model_mismatch",
            EmbedError::NotReady { .. } => "not_ready",
            EmbedError::Io(_) => "io",
        }
    }

    /// Could retrying this file, unchanged, ever succeed? False means something
    /// else must change first (a key, the file, the backend).
    pub fn retryable(&self) -> bool {
        match self {
            EmbedError::RateLimited { .. }
            | EmbedError::Offline(_)
            | EmbedError::Io(_)
            | EmbedError::QuotaExhausted
            | EmbedError::NotReady { .. } => true,
            EmbedError::MissingCredentials
            | EmbedError::RejectedCredentials { .. }
            | EmbedError::Document { .. }
            | EmbedError::BudgetReached { .. }
            | EmbedError::ModelMismatch { .. } => false,
        }
    }

    /// Does this condemn every other file too? Then the run stops rather than
    /// failing each file for the same reason. `RateLimited` is deliberately
    /// absent (see the variant).
    pub fn latching(&self) -> bool {
        matches!(
            self,
            EmbedError::MissingCredentials
                | EmbedError::RejectedCredentials { .. }
                | EmbedError::QuotaExhausted
                | EmbedError::BudgetReached { .. }
                | EmbedError::ModelMismatch { .. }
        )
    }
}

/// Shown to a student: what happened and what would change it — never a URL, a
/// key or anything the server said.
impl fmt::Display for EmbedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmbedError::MissingCredentials => {
                write!(f, "No Voyage API key is saved — add one in Settings to index PDFs.")
            }
            EmbedError::RejectedCredentials { code, expired } => {
                let code = code.as_deref().map(|c| format!(" ({c})")).unwrap_or_default();
                if *expired {
                    write!(
                        f,
                        "The Voyage API key has expired{code} — create a new one and paste it \
                         into Settings."
                    )
                } else {
                    write!(
                        f,
                        "Voyage rejected the API key{code} — check it was copied in full, or \
                         create a new one in Settings."
                    )
                }
            }
            // Not phrased as a failure: on the free tier this is a healthy run.
            EmbedError::RateLimited { retry_after_secs } => match retry_after_secs {
                Some(secs) => {
                    write!(f, "Voyage is rate-limiting this account — indexing resumes in {secs}s.")
                }
                None => write!(
                    f,
                    "Voyage is rate-limiting this account — indexing continues as the limit \
                     allows."
                ),
            },
            EmbedError::QuotaExhausted => write!(
                f,
                "Voyage's allowance is used up. Indexing resumes on its own after it resets."
            ),
            EmbedError::BudgetReached { percent } => write!(
                f,
                "Indexing stopped at the {percent}% spend limit set in Settings → Library. \
                 Raise or turn off the limit there to carry on."
            ),
            EmbedError::Offline(detail) => write!(f, "Could not reach Voyage: {detail}"),
            EmbedError::Document { code } => write!(
                f,
                "Could not read the pages of this PDF to index it (error {code}). Other files \
                 are unaffected."
            ),
            EmbedError::ModelMismatch { app_model, app_dim, backend_model, backend_dim } => write!(
                f,
                "The embedder produces {backend_model} vectors at {backend_dim} dimensions but \
                 this app's index holds {app_model} at {app_dim}. Mixing them would make search \
                 results meaningless, so nothing was indexed."
            ),
            EmbedError::NotReady { backend } => {
                write!(f, "The {backend} embedder is not ready yet. Try again in a moment.")
            }
            EmbedError::Io(detail) => write!(f, "Could not save the page index: {detail}"),
        }
    }
}

impl std::error::Error for EmbedError {}

// ── Configuration ────────────────────────────────────────────────────────────

/// Which backend is the embedder. An unknown value is treated as absent (so
/// `Cloud`), never guessed at. `Local` names a backend that does not ship yet;
/// the setting can express it so it need not be migrated when one does.
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

    /// Anything else — including the parse row's `"auto"` — is not an engine.
    fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "cloud" => Some(Engine::Cloud),
            "local" => Some(Engine::Local),
            _ => None,
        }
    }
}

/// Where a backend's key comes from, if it needs one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSource {
    /// The macOS keychain, via `crate::voyage`; the key never enters SQLite or
    /// the WebView.
    Keychain,
    /// Loopback to a process on this machine: nothing to authenticate.
    None,
}

impl CredentialSource {
    pub fn key(self) -> Option<String> {
        match self {
            CredentialSource::Keychain => crate::voyage::stored_api_key(),
            CredentialSource::None => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmbedConfig {
    pub engine: Engine,
    /// API root for the chosen engine, overridable from the settings blob.
    pub base_url: String,
    pub credentials: CredentialSource,
}

/// Voyage's published API root.
pub const CLOUD_BASE_URL: &str = "https://api.voyageai.com/v1";

/// A local embedder's default origin — not the parse server's port; they are
/// separate programs.
pub const LOCAL_BASE_URL: &str = "http://127.0.0.1:9548";

/// The `embed` row, as far as this seam cares. Every field is optional because
/// the blob holds other settings too.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct StoredEmbedSettings {
    /// A string, not `Engine`, so a stale value costs this field, not the row.
    engine: Option<String>,
    engine_url: Option<String>,
}

/// Read the backend selection from the `embed` settings row. Anything
/// unreadable resolves to the cloud default.
pub fn embed_config() -> EmbedConfig {
    let stored = stored_settings().unwrap_or_default();
    let engine = stored.engine.as_deref().and_then(Engine::parse).unwrap_or(Engine::Cloud);
    let base_url = stored.engine_url.filter(|u| !u.trim().is_empty()).unwrap_or_else(|| {
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
    EmbedConfig { engine, base_url, credentials }
}

/// The embedder this app's settings select. Every call site about to embed
/// goes through here rather than naming a client.
pub fn backend() -> Result<Box<dyn Embedder>, EmbedError> {
    let config = embed_config();
    match config.engine {
        Engine::Cloud => Ok(Box::new(voyage::client::VoyageCloud::with_config(&config)?)),
        // No local client exists in this process. `NotReady`, never a fallback
        // to the cloud: that would embed into a space the user did not choose.
        Engine::Local => Err(EmbedError::NotReady { backend: "local".into() }),
    }
}

/// The `embed` row, decoded.
fn stored_settings() -> Option<StoredEmbedSettings> {
    // Reached from async Tauri commands, where a `block_on` would panic; see
    // `store::setting_blocking`.
    serde_json::from_str(&crate::store::setting_blocking("embed")?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    /// `embed_settings` is an async command; a `block_on` on its worker thread
    /// panics. Surviving the call is what is tested, so it passes without a DB.
    #[test]
    fn the_config_is_readable_from_inside_an_async_runtime() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let base = runtime.block_on(async { embed_config().base_url });
        assert!(!base.is_empty(), "a backend always resolves to some API root");
    }

    /// A real scratch directory: the record's temp+rename must be same-filesystem.
    fn scratch(name: &str) -> Scratch {
        Scratch::new(&format!("embed-{name}"))
    }

    fn sample_pdf(dir: &Path) -> PathBuf {
        let pdf = dir.join("Lecture 3.pdf");
        fs::write(&pdf, b"%PDF-1.4").unwrap();
        pdf
    }

    /// Deliberately not unit length, so every round trip also tests normalising.
    fn raw_vector(seed: u32, len: usize) -> Vec<f32> {
        (0..len).map(|i| ((i as u32 * 37 + seed) % 101) as f32 - 50.0).collect()
    }

    /// A tolerance, not an equality, because of f16 rounding.
    fn assert_unit_norm(vector: &[f32]) {
        let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 2e-3, "‖v‖ = {norm}, not 1");
    }

    #[test]
    fn artifact_name_matches_the_records_already_on_disk() {
        let pdf = Path::new("/library/subj/Lecture 3.pdf");
        assert_eq!(emb_path(pdf), Path::new("/library/subj/Lecture 3.emb.json"));
    }

    #[test]
    fn f16_round_trips_through_base64_at_the_stored_width() {
        let raw = raw_vector(7, EMBED_DIM);
        let encoded = encode_vector(&raw).unwrap();
        let bytes = pack_vector(&raw).unwrap();

        // The blob column's width.
        assert_eq!(bytes.len(), EMBED_DIM * 2);

        let decoded = decode_vector(&encoded).unwrap();
        assert_eq!(decoded.len(), EMBED_DIM);
        assert_eq!(decoded, unpack_vector(&bytes));

        // Same direction as what went in, at f16 precision, after normalising.
        let norm = raw.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>().sqrt();
        for (i, value) in decoded.iter().enumerate() {
            let expected = (raw[i] as f64 / norm) as f32;
            assert!((value - expected).abs() < 1e-3, "dim {i}: {value} vs {expected}");
        }
    }

    #[test]
    fn vectors_are_stored_normalised() {
        let raw = raw_vector(3, EMBED_DIM);
        assert_unit_norm(&decode_vector(&encode_vector(&raw).unwrap()).unwrap());

        // Matryoshka: a longer vector is truncated to EMBED_DIM and
        // *re-normalised*, because a prefix of a unit vector is not one.
        let long = raw_vector(11, EMBED_DIM * 2);
        let page = EmbedPage::new(1, &long).unwrap();
        let stored = decode_vector(&page.vector).unwrap();
        assert_eq!(stored.len(), EMBED_DIM);
        assert_unit_norm(&stored);

        let self_score: f32 = stored.iter().map(|v| v * v).sum();
        assert!((self_score - 1.0).abs() < 2e-3, "{self_score}");
    }

    #[test]
    fn a_short_vector_is_a_different_space_not_something_to_pad() {
        let error = pack_vector(&raw_vector(1, EMBED_DIM - 1)).unwrap_err();
        assert!(matches!(error, EmbedError::ModelMismatch { backend_dim, .. } if backend_dim == EMBED_DIM - 1));
        assert!(!error.retryable());
        assert!(error.latching());
    }

    #[test]
    fn a_zero_vector_is_refused_rather_than_stored_unrankable() {
        let error = pack_vector(&vec![0.0; EMBED_DIM]).unwrap_err();
        assert_eq!(error.kind(), "document");
        // Scoped to the file: the rest of the queue keeps going.
        assert!(!error.latching());
    }

    #[test]
    fn record_keeps_its_wire_shape() {
        let pdf = Path::new("/library/Lecture 3.pdf");
        let out = EmbedOutput::new(pdf, 1, vec![EmbedPage::new(1, &raw_vector(5, EMBED_DIM)).unwrap()]);
        let json = serde_json::to_string(&out).unwrap();
        for key in ["pdf", "model", "dim", "dtype", "instruction", "page_count", "pages", "page_no", "vector"] {
            assert!(json.contains(&format!("\"{key}\"")), "missing {key}: {json}");
        }
        assert!(json.contains("\"dim\":512"), "{json}");
        assert!(json.contains("\"dtype\":\"float16\""), "{json}");
        // And it reads back as the type that wrote it.
        let back: EmbedOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(back.model, EMBED_MODEL);
        assert_eq!(back.pages[0].vector, out.pages[0].vector);
    }

    #[test]
    fn a_record_from_another_model_deserialises_and_re_embeds() {
        let dir = scratch("legacy");
        let pdf = sample_pdf(&dir);
        fs::write(
            emb_path(&pdf),
            r#"{"pdf":"Lecture 3.pdf","model":"Qwen/Qwen3-VL-Embedding-2B","dim":512,
                "dtype":"float16","instruction":"Given a student's question, retrieve the lecture
                slide that answers it.","page_count":1,"pages":[{"page_no":1,"vector":"AAA="}]}"#
                .replace('\n', ""),
        )
        .unwrap();

        let record = read_record(&pdf).expect("legacy record should parse");
        assert_eq!(record.dim, EMBED_DIM);
        assert!(!is_embedded(&pdf));
    }

    #[test]
    fn pages_are_ordered_and_missing_ones_are_dropped_not_filled() {
        let pdf = Path::new("/library/Lecture 3.pdf");
        let out = EmbedOutput::new(
            pdf,
            4,
            vec![
                EmbedPage::new(3, &raw_vector(3, EMBED_DIM)).unwrap(),
                EmbedPage::new(1, &raw_vector(1, EMBED_DIM)).unwrap(),
                // Out of range.
                EmbedPage::new(9, &raw_vector(9, EMBED_DIM)).unwrap(),
                // Duplicate page number.
                EmbedPage::new(1, &raw_vector(2, EMBED_DIM)).unwrap(),
            ],
        );
        assert_eq!(out.pages.iter().map(|p| p.page_no).collect::<Vec<_>>(), vec![1, 3]);
        // Counts embedded pages, not document pages.
        assert_eq!(out.page_count, 2);
    }

    #[test]
    fn write_lands_the_record_atomically_and_leaves_no_temp() {
        let dir = scratch("write");
        let pdf = sample_pdf(&dir);

        let out = EmbedOutput::new(
            &pdf,
            2,
            vec![
                EmbedPage::new(1, &raw_vector(1, EMBED_DIM)).unwrap(),
                EmbedPage::new(2, &raw_vector(2, EMBED_DIM)).unwrap(),
            ],
        );
        out.write(&pdf).unwrap();

        assert!(is_embedded(&pdf));
        assert_eq!(read_record(&pdf).unwrap().page_count, 2);
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn a_different_space_is_refused_and_names_both_sides() {
        let health = Health {
            backend: "oculus-local".into(),
            model: "some-other-embedder".into(),
            dim: 768,
            ready: true,
        };
        let error = health.check().unwrap_err();
        assert_eq!(error.kind(), "model_mismatch");
        let shown = error.to_string();
        assert!(shown.contains("some-other-embedder"), "{shown}");
        assert!(shown.contains(EMBED_MODEL), "{shown}");
        assert!(shown.contains("768") && shown.contains("512"), "{shown}");
        assert!(!error.retryable());
        assert!(error.latching());

        let waiting =
            Health { backend: "voyage".into(), model: EMBED_MODEL.into(), dim: EMBED_DIM, ready: false };
        let error = waiting.check().unwrap_err();
        assert_eq!(error.kind(), "not_ready");
        assert!(error.retryable());
        assert!(!error.latching());

        let ok = Health { backend: "voyage".into(), model: EMBED_MODEL.into(), dim: EMBED_DIM, ready: true };
        assert!(ok.check().is_ok());
    }

    #[test]
    fn a_rate_limit_is_routine_not_fatal() {
        let error = EmbedError::RateLimited { retry_after_secs: Some(20) };
        assert_eq!(error.kind(), "rate_limited");
        assert!(error.retryable());
        assert!(!error.latching());
        assert!(EmbedError::QuotaExhausted.retryable());
        assert!(EmbedError::QuotaExhausted.latching());
    }

    #[test]
    fn credential_failures_keep_the_word_the_failure_ui_matches_on() {
        for error in [
            EmbedError::MissingCredentials,
            EmbedError::RejectedCredentials { code: None, expired: false },
        ] {
            assert!(error.kind().contains("credential"), "{}", error.kind());
            assert!(!error.retryable());
            assert!(error.latching());
        }
        let shown = EmbedError::RejectedCredentials { code: Some("401".into()), expired: true }
            .to_string();
        assert!(shown.contains("Settings"), "{shown}");
        assert!(shown.contains("401"), "{shown}");
    }

    #[test]
    fn a_stale_fallback_policy_is_not_an_engine() {
        assert_eq!(Engine::parse("auto"), None);
        assert_eq!(Engine::parse(""), None);
        assert_eq!(Engine::parse("cloud"), Some(Engine::Cloud));
        assert_eq!(Engine::parse(" local "), Some(Engine::Local));
        assert_eq!(Engine::Cloud.as_str(), "cloud");
    }
}

pub mod commands;
pub mod estimate;
pub mod events;
pub mod raster;
pub mod voyage;
