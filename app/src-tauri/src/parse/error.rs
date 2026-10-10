//! Why a parse did not happen, and how each failure reads.

use std::fmt;
use std::fs;
use std::path::Path;

/// The `Document` code for an Office file LibreOffice could not convert: there
/// is no PDF, so nothing was sent to any parser.
pub const CONVERSION_FAILED: &str = "office-conversion";

/// The `Document` code for a spreadsheet `crate::pages::sheets` could not read: it
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
    /// reworded). `app/src/lib/pipeline/parseState.ts` matches `/credential|token/i`
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
