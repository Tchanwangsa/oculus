//! The seam every embedder plugs into.
//!
//! Embedding used to live behind the Python sidecar's HTTP port, with a torch
//! runtime and a 1.2 GB model snapshot behind it. It is a trait now: Voyage
//! implements it in-process, and a local embedder — should one ever ship —
//! implements the same one. Nothing here may assume the cloud; anything
//! cloud-shaped (an API root, a key, a rate limit) arrives as configuration or
//! as a parameter.
//!
//! What the seam owns is the *contract*, not the embedding: the on-disk
//! artifact, the space every vector in the `pages` table must belong to, the
//! error vocabulary the failure UI reads, and the encoding the blob column
//! expects. It is a deliberate mirror of `parse/mod.rs` — same shape, same
//! three-way error split — so one failure UI can read both seams without
//! learning a second vocabulary.
//!
//! **The measured constraint in `CLAUDE.md` binds here:** what gets embedded is
//! the *rendered page image*, never extracted text, and never an average of the
//! two. That is why `embed` takes a PDF and a page count rather than a string —
//! the backend rasterises (`embed/raster.rs`) and embeds pixels.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use half::f16;
use serde::{Deserialize, Serialize};

/// The stored vector width, and the reason this port needs no schema
/// migration.
///
/// 512 is not a round number somebody liked. It is (a) the width of every blob
/// already in `pages.embedding` — 1024 bytes = 512 x f16 — and (b) a supported
/// Matryoshka output width for the cloud model, which honours
/// `output_dimension: 512` directly. Because those two agree, the column, the
/// decoder in `retrieval.rs` and the brute-force scan all carry over untouched;
/// only the numbers inside the blobs change. Move this and you have signed up
/// for a migration of every row.
pub const EMBED_DIM: usize = 512;

/// The model that defines the space, stamped into every record.
///
/// This names the *space*, not the vendor: it is the string `Health::check`
/// compares and `is_embedded` matches, so a local backend claiming to produce
/// the same vectors would have to claim this same id. It is the closest thing
/// this seam has to `PARSER_VERSION` — except that unlike a parser version, a
/// disagreement here is not a formatting difference, it is a different
/// geometry (see `Health::check`).
pub const EMBED_MODEL: &str = "voyage-multimodal-3.5";

/// How a vector is written down: little-endian float16, base64 in the record,
/// raw bytes in the blob column. Recorded rather than assumed because the
/// record has always carried it and the reader round-trips what it finds.
pub const EMBED_DTYPE: &str = "float16";

/// The query side of the asymmetry, recorded so a file on disk says which
/// convention produced its vectors.
///
/// The Python embedder put a literal sentence here and prepended it to every
/// query ("Given a student's question, retrieve the lecture slide…"). The cloud
/// model has no instruction field at all — its asymmetry is
/// `input_type: "query"` against `"document"` — so this constant stops being a
/// prompt and becomes a *name* for the convention. It is compared, never sent;
/// a backend that needs a sentence builds its own from this.
///
/// Documents and queries must keep using opposite sides. Using one for both is
/// not an error anything can detect — it just quietly ranks worse.
pub const QUERY_INSTRUCTION: &str = "input_type:query";

// ── Artifact location ────────────────────────────────────────────────────────
//
// One file, beside the PDF, named off its stem — the same rule the parse
// artifacts follow, and the reason a library folder can be copied whole.

pub fn emb_path(pdf: &Path) -> PathBuf {
    pdf.with_extension("emb.json")
}

/// The record beside this PDF, read back as the type that wrote it.
///
/// Records written by the Python embedder deserialise too — the wire shape is
/// deliberately unchanged. They will not *match* (their `model` is the retired
/// Qwen id, so `is_embedded` rejects them and they re-embed), but they must
/// still parse rather than blow up a scan of the library.
pub fn read_record(pdf: &Path) -> Option<EmbedOutput> {
    serde_json::from_str(&fs::read_to_string(emb_path(pdf)).ok()?).ok()
}

/// True when this PDF's vectors are in *this* app's space and it can be
/// skipped.
///
/// Mirrors the Python check (`sidecar/embed_contract.py`): model, dim and
/// instruction must all match. Unlike `parse_mode`, which accepts quality
/// output whatever version wrote it, there is no tolerance available here —
/// a vector from another model is not older output, it is output from a
/// different geometry, and keeping it costs more than re-embedding.
/// **Coverage is part of the question, not just identity.** The Python
/// embedder ran locally and all-or-nothing: it either embedded every page or
/// raised, so a record on disk implied a complete record and checking model,
/// dim and instruction was enough. A cloud backend breaks that assumption —
/// a rate limit or one refused page mid-document is an ordinary outcome, and
/// `EmbedOutput::new` drops pages it never received rather than inventing zero
/// vectors for them. Without the count check below, a document that lost page
/// 47 would read as embedded forever: that page would never be searchable and
/// nothing would ever retry it, with no error anywhere to say so.
///
/// The expected length comes from the parse record beside it, which is the
/// same `page_no` space retrieval joins on. When there is no parse record
/// there is nothing to compare against, and identity alone has to do.
pub fn is_embedded(pdf: &Path) -> bool {
    let Some(record) = read_record(pdf) else {
        return false;
    };
    if record.model != EMBED_MODEL
        || record.dim != EMBED_DIM
        || record.instruction != QUERY_INSTRUCTION
    {
        return false;
    }
    match crate::parse::read_record(pdf) {
        Some(parsed) => record.pages.len() >= parsed.page_count as usize,
        None => true,
    }
}

// ── Vector encoding ──────────────────────────────────────────────────────────
//
// This is the one place the Python wire format could not simply be copied.
//
// The sidecar produced its base64 f16 payload itself — it owned a torch tensor
// and wrote `.tobytes()`. The cloud model cannot hand back that exact thing.
// Two parameters are involved and they are easy to confuse; both were checked
// against the real API on 2026-09-17:
//
// * `output_dtype` is *precision*, and names `['binary', 'float', 'int8', …]`.
//   There is no `float16`, so the stored width is never what arrives.
// * `output_encoding` is *transport*, and `"base64"` there does work — it
//   returns a base64 NumPy array, measured at 2048 bytes for 512 dims, i.e.
//   **f32** little-endian.
//
// So a backend can avoid parsing 512 JSON floats per page, but it still hands
// over f32 and the f16 narrowing has to happen somewhere. It happens here,
// once, rather than in each backend: every backend, cloud or local, hands over
// `&[f32]` and gets the same bytes on disk.

/// Truncate to `EMBED_DIM`, re-normalise, pack little-endian f16.
///
/// All three steps matter:
///
/// * **Truncate** — Matryoshka models nest a shorter vector inside a longer
///   one, so a prefix is a valid embedding. A backend that returns more than
///   `EMBED_DIM` is fine; one that returns fewer is a different space and is
///   refused, not zero-padded.
/// * **Re-normalise** — truncating a unit vector does not leave a unit vector,
///   and a backend may not normalise at all. The cloud model happens to return
///   vectors already at ‖v‖ ≈ 1.0019, which is *close* enough to hide a bug and
///   not close enough to rely on.
/// * **f16** — the storage type. Lossy by design; measured recall is unmoved.
///
/// **Everything downstream assumes unit length.** `retrieval.rs` ranks by a raw
/// dot product and calls the result cosine similarity; it never divides by a
/// norm. If an unnormalised vector reaches the table, nothing breaks, nothing
/// logs, and long pages simply start winning. That is why normalisation is done
/// here, at the one gate every vector passes through, rather than trusted to a
/// backend.
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
    // A zero (or non-finite) vector cannot be normalised, and a silently
    // unnormalised one would rank at score 0 against every query — present in
    // the table, invisible in results. Scoped to this document, not the run.
    if !norm.is_finite() || norm <= 0.0 {
        return Err(EmbedError::Document { code: "zero_vector".into() });
    }
    let mut bytes = Vec::with_capacity(EMBED_DIM * 2);
    for value in head {
        bytes.extend_from_slice(&f16::from_f32((*value as f64 / norm) as f32).to_le_bytes());
    }
    Ok(bytes)
}

/// base64 f16 -> f32, the read side of `encode_vector`.
///
/// `retrieval.rs` has its own copy of this decode for the blob column; this one
/// exists for the record, and for the round-trip test that keeps the two
/// honest.
pub fn decode_vector(encoded: &str) -> Result<Vec<f32>, EmbedError> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| EmbedError::Document { code: format!("bad base64: {e}") })?;
    Ok(unpack_vector(&raw))
}

/// float16 little-endian -> f32. Vectors are stored normalised, so callers may
/// treat a dot product as cosine similarity.
pub fn unpack_vector(raw: &[u8]) -> Vec<f32> {
    raw.chunks_exact(2).map(|c| f16::from_le_bytes([c[0], c[1]]).to_f32()).collect()
}

// ── The embedded document ────────────────────────────────────────────────────

/// One page's vector, keyed by its 1-based page number.
///
/// `page_no` is the join key retrieval rests on: a hit here resolves to
/// markdown from `.pages.json` only because both sides agree on this number.
/// The two artifacts are written by different backends at different times and
/// have nothing else in common.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedPage {
    pub page_no: u32,
    /// base64 of little-endian f16, `EMBED_DIM` wide. Stored as the wire string
    /// rather than as floats so a record round-trips byte-for-byte.
    pub vector: String,
}

impl EmbedPage {
    /// Encode one page's vector, normalising it on the way in.
    pub fn new(page_no: u32, vector: &[f32]) -> Result<Self, EmbedError> {
        Ok(Self { page_no, vector: encode_vector(vector)? })
    }

    pub fn vector_f32(&self) -> Result<Vec<f32>, EmbedError> {
        decode_vector(&self.vector)
    }
}

/// Exactly the `.emb.json` on disk.
///
/// Field names are the wire format: snake_case, no rename attributes. The 166
/// records already in the library were written by Python and must keep
/// deserialising even though they will not match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedOutput {
    /// The PDF's file name, not its path — the record travels with the folder.
    pub pdf: String,
    pub model: String,
    pub dim: usize,
    pub dtype: String,
    pub instruction: String,
    /// **How many pages were embedded — not how long the document is.** The
    /// Python wrote `len(pages)` here and the field keeps that meaning, which
    /// is the one place this record reads differently from `.pages.json`, whose
    /// `page_count` *is* the document length. Do not "fix" it to agree: a
    /// reader comparing this against the PDF's real page count is asking a
    /// different question (did every page embed?) and can ask it explicitly.
    pub page_count: usize,
    pub pages: Vec<EmbedPage>,
}

impl EmbedOutput {
    /// Build a record that satisfies the contract regardless of what the
    /// backend produced: pages in ascending order, one entry per page number,
    /// nothing outside `1..=page_count`.
    ///
    /// **Missing pages are dropped, not gap-filled** — the one structural
    /// divergence from `ParseOutput::new`, which pads absent pages with `""`.
    /// An empty string is a real answer ("this page has no text"); there is no
    /// empty *vector*. A zero-filled slot cannot be normalised, scores 0
    /// against every query, and would sit in the table looking indexed. A page
    /// with no vector is simply not in the index, and the join on `page_no` is
    /// what makes that harmless.
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

    /// Put the record on disk, atomically.
    ///
    /// Same discipline as `ParseOutput::write`, for the same reason and against
    /// a worse failure. The Python wrote this with a plain `write_text`
    /// (`sidecar/embedder.py:189`); a crash mid-write left JSON that either
    /// fails to parse (recoverable — it re-embeds) or, with terrible luck,
    /// parses with a truncated `pages` array and reads as a *complete* record
    /// for a partly-indexed file. Nothing downstream would ever notice: the
    /// missing pages are simply never retrieved.
    ///
    /// So: temp file, fsync, rename. The record's existence is the only
    /// evidence the embedding finished, and it becomes visible in one step.
    pub fn write(&self, pdf: &Path) -> Result<(), EmbedError> {
        let final_path = emb_path(pdf);
        let body = serde_json::to_vec(self)
            .map_err(|e| EmbedError::Io(format!("encode {}: {e}", final_path.display())))?;
        let tmp = final_path.with_extension(format!("json.tmp{}", std::process::id()));
        let mut file = fs::File::create(&tmp)
            .map_err(|e| EmbedError::Io(format!("create {}: {e}", tmp.display())))?;
        let staged = file
            .write_all(&body)
            .and_then(|()| file.sync_all())
            .map_err(|e| EmbedError::Io(format!("write {}: {e}", tmp.display())));
        drop(file);
        if let Err(error) = staged {
            fs::remove_file(&tmp).ok();
            return Err(error);
        }
        fs::rename(&tmp, &final_path).map_err(|e| {
            fs::remove_file(&tmp).ok();
            EmbedError::Io(format!("rename into {}: {e}", final_path.display()))
        })
    }
}

// ── Progress and health ──────────────────────────────────────────────────────

/// Reported while an embedding run works through a document. `total_pages` can
/// be zero before the backend knows how long the document is; the UI shows a
/// count, not a fraction, until it isn't.
///
/// Identical in shape to `parse::Progress` on purpose — the sidebar renders
/// both with one component, and a pipeline row does not care which stage it is
/// watching.
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
    pub model: String,
    pub dim: usize,
    pub ready: bool,
}

impl Health {
    /// The space handshake. A backend that embeds into a different space is
    /// **refused, not warned about.**
    ///
    /// This is the stricter cousin of `parse::Health::check`, and for a reason
    /// worth spelling out: a parser version mismatch produces files that look
    /// wrong to a reader, which is annoying but visible. Two embedding spaces in
    /// one `pages` table produce nothing visible at all. The scan runs, every
    /// dot product returns a number, the results sort, and the ranking is
    /// noise — a search that returns confident, well-formatted, unrelated
    /// slides. That is strictly worse than an error, because nothing looks
    /// broken. So a disagreement about `model` or `dim` stops the run before a
    /// single vector is written.
    ///
    /// Like the parser's, this cannot fail today — the only backend in this
    /// process compares its own constant against itself. It exists for the
    /// second implementation, whatever that turns out to be, and for the day
    /// the vendor silently repoints a model alias.
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

/// Ask a backend whether it can be used, and refuse it if it disagrees about
/// the space. Every call site that is about to embed goes through here rather
/// than calling `health()` and reading the fields itself.
pub fn preflight(embedder: &dyn Embedder) -> Result<Health, EmbedError> {
    let health = embedder.health();
    health.check()?;
    Ok(health)
}

// ── The trait ────────────────────────────────────────────────────────────────

pub trait Embedder: Send + Sync {
    /// Embed every page of one PDF.
    ///
    /// `page_count` is passed in rather than discovered, because the caller
    /// already knows it — it has the parse record — and because it is the
    /// bound `EmbedOutput::new` uses to reject a backend whose page offsets
    /// drifted. A backend is free to render fewer pages than this (a page that
    /// will not rasterise is dropped, see `EmbedOutput::new`) but never more.
    ///
    /// `on_progress` may be called from whatever thread the backend is using,
    /// and a cloud backend batching several pages per request will call it in
    /// jumps rather than one page at a time.
    fn embed(
        &self,
        pdf: &Path,
        page_count: u32,
        on_progress: &dyn Fn(Progress),
    ) -> Result<EmbedOutput, EmbedError>;

    /// Embed a search query — the *other* side of the asymmetry
    /// (`QUERY_INSTRUCTION`). Returns floats rather than the stored encoding
    /// because a query vector is never written down; it is dotted against the
    /// table once and discarded.
    ///
    /// It is still normalised, by the same `encode_vector` path, so that the
    /// dot product on both sides is a cosine.
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbedError>;

    fn health(&self) -> Health;
}

// ── Failure ──────────────────────────────────────────────────────────────────

/// Why an embedding did not happen.
///
/// The vocabulary is `ParseError`'s, deliberately: `kind()` / `retryable()` /
/// `latching()` answer the same three questions with the same words, so the
/// failure UI reads one seam's errors and the other's without branching on
/// which stage produced them.
///
/// **No variant carries server response text.** An error body can echo the
/// request, which for this API means an echo of the base64 page image — and, in
/// the rate-limit case, the account's billing state. Errors carry the *code*,
/// never the body. Same rule as the parser's.
#[derive(Debug, Clone)]
pub enum EmbedError {
    /// No API key is stored. Nothing will embed until one is.
    MissingCredentials,
    /// The backend refused the key itself. Latching: every other file in the
    /// queue would hit the identical rejection.
    RejectedCredentials { code: Option<String>, expired: bool },
    /// Throttled — too many requests, or too many tokens, per minute.
    ///
    /// **This is the routine answer on this account, not a failure.** Verified
    /// live 2026-09-17: with no payment method on file the limit is 3 requests
    /// and 10K tokens a minute, and at ~989 tokens for a 200-DPI page that means
    /// a full re-index *spends most of its wall clock inside a 429*. With a card
    /// the same account is 2000 RPM / 2M TPM and may never see one.
    ///
    /// So it is retryable and, emphatically, **not latching**: an
    /// `EmbedError::RateLimited` that stopped the run would make the free tier
    /// unusable rather than slow, and would look to the user like a broken key.
    /// The backend waits and carries on; this variant exists so a *wait* can be
    /// reported honestly when it is long enough to notice.
    RateLimited { retry_after_secs: Option<u64> },
    /// The allowance is spent. Unlike a rejected key this repairs itself at the
    /// next reset, so it is worth retrying — later. Latching, because every
    /// other file draws on the same allowance.
    QuotaExhausted,
    /// Could not reach the backend at all. Holds the *local* transport error (a
    /// connect/timeout message), never a response body.
    Offline(String),
    /// The backend ran and could not make sense of this document — it would not
    /// rasterise, or a page came back empty. Scoped to this file: the rest of
    /// the queue is fine and should keep going.
    Document { code: String },
    /// The backend embeds into a different space than this app reads. See
    /// `Health::check` for why this is fatal rather than a warning.
    ModelMismatch { app_model: String, app_dim: usize, backend_model: String, backend_dim: usize },
    /// The backend answered but is not accepting work yet.
    NotReady { backend: String },
    /// Writing the record failed — disk full, permissions, a vanished
    /// directory. The embedding itself may well have succeeded, and on a metered
    /// backend that means it was also paid for.
    Io(String),
}

impl EmbedError {
    /// The machine-readable discriminant that travels with a failure event.
    ///
    /// The prose in `Display` is for a student and may be reworded at any time;
    /// this is what the failure UI branches on, so it is a frozen vocabulary.
    /// `app/src/lib/parseState.ts` matches `/credential|token/i` against a
    /// parse failure's kind to decide whether to point at Settings — both
    /// credential variants here keep that word in their name so the same test
    /// works unchanged on an embed failure.
    pub fn kind(&self) -> &'static str {
        match self {
            EmbedError::MissingCredentials => "missing_credentials",
            EmbedError::RejectedCredentials { .. } => "rejected_credentials",
            EmbedError::RateLimited { .. } => "rate_limited",
            EmbedError::QuotaExhausted => "quota_exhausted",
            EmbedError::Offline(_) => "offline",
            EmbedError::Document { .. } => "document",
            EmbedError::ModelMismatch { .. } => "model_mismatch",
            EmbedError::NotReady { .. } => "not_ready",
            EmbedError::Io(_) => "io",
        }
    }

    /// Could retrying *this file*, unchanged, ever succeed?
    ///
    /// False means retrying is pointless until something else changes: a new
    /// key, a different file, a matching backend. The distinction drives
    /// whether a failure offers a retry at all.
    pub fn retryable(&self) -> bool {
        match self {
            // Throttling is the ordinary case and always clears; transport
            // hiccups and local write failures are the usual retries; quota
            // repairs itself at the reset; a backend that isn't ready may be in
            // a moment.
            EmbedError::RateLimited { .. }
            | EmbedError::Offline(_)
            | EmbedError::Io(_)
            | EmbedError::QuotaExhausted
            | EmbedError::NotReady { .. } => true,
            // A missing or refused key, a document that will not render, a
            // backend in another space: all need an intervention first.
            EmbedError::MissingCredentials
            | EmbedError::RejectedCredentials { .. }
            | EmbedError::Document { .. }
            | EmbedError::ModelMismatch { .. } => false,
        }
    }

    /// Does this condemn every other file too?
    ///
    /// A latching failure should stop the run: the alternative is a hundred
    /// identical errors and a hundred files marked failed for a reason that had
    /// nothing to do with them.
    ///
    /// Note what is *absent*: `RateLimited`. It is the one retryable condition
    /// that looks account-wide and must not latch — on the free tier it is the
    /// steady state of a working run.
    pub fn latching(&self) -> bool {
        matches!(
            self,
            EmbedError::MissingCredentials
                | EmbedError::RejectedCredentials { .. }
                | EmbedError::QuotaExhausted
                | EmbedError::ModelMismatch { .. }
        )
    }
}

/// These strings are shown to a student, not to a developer: they say what
/// happened and what would change it, and they never contain a URL, a key or
/// anything the server said.
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
            // Deliberately not phrased as a failure: on the free rate limit this
            // is what a healthy run looks like most of the time.
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
            EmbedError::Offline(_) => {
                write!(f, "Could not reach Voyage. Check your connection, then try again.")
            }
            EmbedError::Document { code } => write!(
                f,
                "Could not read the pages of this PDF to index it (error {code}). Other files \
                 are unaffected."
            ),
            // Both sides, always: "model mismatch" alone tells the user nothing
            // about which one to change.
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

/// Which backend *is* the embedder.
///
/// Same two names as `parse::Engine`, and the same rule about strangers: a
/// value that is neither is treated as if the field were absent rather than
/// guessed at. The stakes are higher here than for parsing — quietly resolving
/// an unknown engine to *something* could point the app at a second embedding
/// space — so the fallback is always the one engine that exists.
///
/// `Local` names a backend that does not ship. It is in the enum because the
/// seam's whole purpose is that it could, and because a setting that cannot
/// express it would have to be migrated the day it does.
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

    /// Anything that is not one of the two names — including the parse row's
    /// legacy `"auto"` fallback policy, should anyone paste it across — is not
    /// an engine, and is treated as if the field were absent.
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
    /// The macOS keychain, via `crate::voyage`. The key never enters SQLite or
    /// the WebView, so it is named here rather than carried here.
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
    /// API root for the chosen engine, overridable from the settings blob so a
    /// local embedder on a non-default port can be reached without a release.
    pub base_url: String,
    pub credentials: CredentialSource,
}

/// Voyage's published API root.
pub const CLOUD_BASE_URL: &str = "https://api.voyageai.com/v1";

/// A local embedder's default origin. Deliberately *not* the parse server's
/// port: they are two different programs and a user may well run one without
/// the other.
pub const LOCAL_BASE_URL: &str = "http://127.0.0.1:9548";

/// The `embed` row, as far as this seam cares about it. Every field is
/// optional: the blob is shared with settings this module has no opinion on,
/// and those must survive being read here.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct StoredEmbedSettings {
    /// Read as a string, not as `Engine`: a typo or a stale value must cost us
    /// this one field, not the whole row.
    engine: Option<String>,
    engine_url: Option<String>,
}

/// Read the backend selection out of the `settings` table's `embed` row.
///
/// Unreadable, missing or nonsense settings all resolve to the cloud default —
/// a user whose database is in an odd state should still get their library
/// indexed.
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

/// One row, one connection, no pool — the same reasoning as
/// `parse::stored_settings`: this reader needs a single `SELECT` a few times
/// per run, and the pool helpers live in `store.rs`, which is async-first.
fn stored_settings() -> Option<StoredEmbedSettings> {
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::{Connection, Row, SqliteConnection};

    let database = crate::paths::db_path(&crate::paths::data_dir());
    if !database.is_file() {
        return None;
    }

    // Callers are synchronous (the index queue, the CLI), exactly as on the
    // parse side.
    tauri::async_runtime::block_on(async move {
        let options = SqliteConnectOptions::new()
            .filename(&database)
            .create_if_missing(false)
            .busy_timeout(std::time::Duration::from_secs(15));
        let mut connection = SqliteConnection::connect_with(&options).await.ok()?;
        let row = sqlx::query("SELECT value FROM settings WHERE key = 'embed'")
            .fetch_optional(&mut connection)
            .await
            .ok()
            .flatten();
        connection.close().await.ok();
        serde_json::from_str(&row?.get::<String, _>("value")).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch library folder. The record's temp+rename must be
    /// same-filesystem, so the tests use a real directory rather than faking
    /// paths.
    fn scratch(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("oculus-embed-{name}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_pdf(dir: &Path) -> PathBuf {
        let pdf = dir.join("Lecture 3.pdf");
        fs::write(&pdf, b"%PDF-1.4").unwrap();
        pdf
    }

    /// Something vector-shaped and deliberately *not* unit length, so every
    /// test that round-trips one is also testing that the seam normalised it.
    fn raw_vector(seed: u32, len: usize) -> Vec<f32> {
        (0..len).map(|i| ((i as u32 * 37 + seed) % 101) as f32 - 50.0).collect()
    }

    /// The invariant the whole ranking path rests on: `retrieval.rs` scores by
    /// a plain dot product and calls it cosine similarity, so every stored
    /// vector must be unit length. f16 rounding is why this is a tolerance and
    /// not an equality — at 512 dimensions the accumulated error is ~1e-3.
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

        // 1024 bytes = 512 x f16. This number is the blob column's width and
        // the reason this port needs no migration.
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
        // Not unit length going in — a backend is allowed to be sloppy, and the
        // cloud one returns ‖v‖ ≈ 1.0019 rather than exactly 1.
        let raw = raw_vector(3, EMBED_DIM);
        assert_unit_norm(&decode_vector(&encode_vector(&raw).unwrap()).unwrap());

        // Matryoshka: a longer vector is truncated to EMBED_DIM and
        // *re-normalised*, because a prefix of a unit vector is not one.
        let long = raw_vector(11, EMBED_DIM * 2);
        let page = EmbedPage::new(1, &long).unwrap();
        let stored = page.vector_f32().unwrap();
        assert_eq!(stored.len(), EMBED_DIM);
        assert_unit_norm(&stored);

        // A unit vector dotted with itself is 1 — the property `retrieval.rs`
        // reads as "identical page".
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
    fn record_keeps_the_python_wire_shape() {
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
    fn a_python_era_record_still_deserialises_and_still_re_embeds() {
        let dir = scratch("legacy");
        let pdf = sample_pdf(&dir);
        // The shape written by `sidecar/embedder.py`, Qwen model id and all.
        fs::write(
            emb_path(&pdf),
            r#"{"pdf":"Lecture 3.pdf","model":"Qwen/Qwen3-VL-Embedding-2B","dim":512,
                "dtype":"float16","instruction":"Given a student's question, retrieve the lecture
                slide that answers it.","page_count":1,"pages":[{"page_no":1,"vector":"AAA="}]}"#
                .replace('\n', ""),
        )
        .unwrap();

        // Readable — a library scan must not blow up on one.
        let record = read_record(&pdf).expect("legacy record should parse");
        assert_eq!(record.dim, EMBED_DIM);
        // But not ours: a different space is not stale output, it is wrong
        // output, so the file re-embeds.
        assert!(!is_embedded(&pdf));

        fs::remove_dir_all(&dir).ok();
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
                // Out of range: a backend whose page offsets drifted must not be
                // able to corrupt the join key.
                EmbedPage::new(9, &raw_vector(9, EMBED_DIM)).unwrap(),
                // A duplicate page number would put two vectors in one slot.
                EmbedPage::new(1, &raw_vector(2, EMBED_DIM)).unwrap(),
            ],
        );
        assert_eq!(out.pages.iter().map(|p| p.page_no).collect::<Vec<_>>(), vec![1, 3]);
        // `page_count` counts embedded pages, not document pages — page 2 has
        // no entry and no empty placeholder either.
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

        fs::remove_dir_all(&dir).ok();
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
        // Mixing spaces is never worth attempting, and it condemns the run.
        assert!(!error.retryable());
        assert!(error.latching());

        // The right space but not up yet is a wait, not a refusal.
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
        // The load-bearing pair: on the free tier (3 RPM / 10K TPM) a 429 is
        // what a *working* run looks like. Latching here would make the free
        // tier unusable rather than slow.
        let error = EmbedError::RateLimited { retry_after_secs: Some(20) };
        assert_eq!(error.kind(), "rate_limited");
        assert!(error.retryable());
        assert!(!error.latching());
        // A spent allowance is the opposite: worth retrying later, but every
        // other file draws on the same pool, so the run stops.
        assert!(EmbedError::QuotaExhausted.retryable());
        assert!(EmbedError::QuotaExhausted.latching());
    }

    #[test]
    fn credential_failures_keep_the_word_the_failure_ui_matches_on() {
        // `app/src/lib/parseState.ts` tests /credential|token/i to decide
        // whether to point at Settings.
        for error in [
            EmbedError::MissingCredentials,
            EmbedError::RejectedCredentials { code: None, expired: false },
        ] {
            assert!(error.kind().contains("credential"), "{}", error.kind());
            assert!(!error.retryable());
            assert!(error.latching());
        }
        // Never the key, never a server body, in front of a student.
        let shown = EmbedError::RejectedCredentials { code: Some("401".into()), expired: true }
            .to_string();
        assert!(shown.contains("Settings"), "{shown}");
        assert!(shown.contains("401"), "{shown}");
    }

    #[test]
    fn a_stale_fallback_policy_is_not_an_engine() {
        // The parse row's legacy `backend` vocabulary — "auto" meant "cloud with
        // a local fallback" — must never resolve to an engine here either.
        assert_eq!(Engine::parse("auto"), None);
        assert_eq!(Engine::parse(""), None);
        assert_eq!(Engine::parse("cloud"), Some(Engine::Cloud));
        assert_eq!(Engine::parse(" local "), Some(Engine::Local));
        assert_eq!(Engine::Cloud.as_str(), "cloud");
    }
}

/// Page rasterization — the page *images* everything above embeds. See
/// `embed/raster.rs`.
pub mod raster;
