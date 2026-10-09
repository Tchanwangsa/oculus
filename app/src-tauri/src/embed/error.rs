//! Why an embedding did not happen, and how each failure reads.

use std::fmt;

/// Why an embedding did not happen. Same `kind` / `retryable` / `latching`
/// vocabulary as `ParseError`, so one failure UI reads both seams.
///
/// No variant carries server response text: a body can echo the request (the
/// base64 page image) or the account's billing state. Codes only.
#[derive(Debug, Clone)]
pub enum EmbedError {
    /// No API key is stored.
    MissingCredentials,
    /// A key may be stored, but the keychain refused to hand it over (a denied
    /// prompt, or a sandboxed process). Holds the keychain's own error.
    UnreadableCredentials(String),
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
    ModelMismatch {
        app_model: String,
        app_dim: usize,
        backend_model: String,
        backend_dim: usize,
    },
    /// The backend answered but is not accepting work yet.
    NotReady { backend: String },
    /// Writing the record failed. The embedding may have succeeded — and been
    /// paid for.
    Io(String),
}

impl EmbedError {
    /// The frozen discriminant the failure UI branches on (`Display` prose may
    /// change). `app/src/lib/pipeline/parseState.ts` matches `/credential|token/i`, so
    /// both credential kinds keep that word.
    pub fn kind(&self) -> &'static str {
        match self {
            EmbedError::MissingCredentials => "missing_credentials",
            EmbedError::UnreadableCredentials(_) => "unreadable_credentials",
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
            | EmbedError::NotReady { .. }
            | EmbedError::UnreadableCredentials(_) => true,
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
                | EmbedError::UnreadableCredentials(_)
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
                write!(
                    f,
                    "No Voyage API key is saved — add one in Settings to index PDFs."
                )
            }
            EmbedError::UnreadableCredentials(detail) => write!(
                f,
                "The keychain refused to give out the Voyage API key ({detail}). The key is \
                 not missing — macOS denied this process access to it."
            ),
            EmbedError::RejectedCredentials { code, expired } => {
                let code = code
                    .as_deref()
                    .map(|c| format!(" ({c})"))
                    .unwrap_or_default();
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
                    write!(
                        f,
                        "Voyage is rate-limiting this account — indexing resumes in {secs}s."
                    )
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
            EmbedError::ModelMismatch {
                app_model,
                app_dim,
                backend_model,
                backend_dim,
            } => write!(
                f,
                "The embedder produces {backend_model} vectors at {backend_dim} dimensions but \
                 this app's index holds {app_model} at {app_dim}. Mixing them would make search \
                 results meaningless, so nothing was indexed."
            ),
            EmbedError::NotReady { backend } => {
                write!(
                    f,
                    "The {backend} embedder is not ready yet. Try again in a moment."
                )
            }
            EmbedError::Io(detail) => write!(f, "Could not save the page index: {detail}"),
        }
    }
}

impl std::error::Error for EmbedError {}
