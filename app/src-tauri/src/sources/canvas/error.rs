use keyd_core::client::KeydError;
use keyd_core::okta::LoginError;

/// Why a Canvas request did not come back with an answer worth reading. An
/// HTTP status is not an error here: a locked module's 403 is a value.
#[derive(Debug, Clone, PartialEq)]
pub enum CanvasError {
    /// Canvas stopped accepting the session and oculus-keyd could not make a
    /// new one. `Some` is why its sign-in was refused or failed; `None` is a
    /// sign-in that ran and was still rejected. A run that sees this stops:
    /// every later request would fail the same way.
    Expired(Option<LoginError>),
    /// oculus-keyd is not installed or not running. Never an expired session:
    /// signing in again would not help.
    KeydAbsent,
    /// oculus-keyd refused or failed the request, for a reason that is not
    /// the session (a path it will not send, its vault, its caller check).
    Keyd(KeydError),
    /// No answer from Canvas, or from the file host a redirect led to.
    Unreachable(String),
    /// A non-success status the caller asked to be an error.
    Http { status: u16, what: String },
    /// A reply that cannot be used: a redirect loop, a bad URL, bad JSON.
    Failed(String),
}

impl CanvasError {
    /// What oculus-keyd's refusal means for a Canvas request.
    pub(super) fn from_keyd(error: KeydError) -> CanvasError {
        match error {
            KeydError::Absent => CanvasError::KeydAbsent,
            KeydError::NoSession(_, why) => CanvasError::Expired(Some(why)),
            KeydError::Missing(_) => CanvasError::Expired(None),
            KeydError::Upstream(detail) => CanvasError::Unreachable(detail),
            other => CanvasError::Keyd(other),
        }
    }

    pub fn is_expired(&self) -> bool {
        matches!(self, CanvasError::Expired(_))
    }
}

impl std::fmt::Display for CanvasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CanvasError::Expired(None) => {
                f.write_str("Canvas rejected the session — sign in again from Settings → Canvas.")
            }
            CanvasError::Expired(Some(why)) => write!(
                f,
                "Canvas rejected the session, and signing in again did not work: {why}"
            ),
            CanvasError::KeydAbsent => f.write_str(
                "oculus-keyd is not running or not installed, and Canvas is reached only \
                 through it (`oculus keyd status`).",
            ),
            CanvasError::Keyd(error) => error.fmt(f),
            CanvasError::Unreachable(detail) => write!(f, "could not reach Canvas: {detail}"),
            CanvasError::Http { status, what } => write!(f, "HTTP {status} for {what}"),
            CanvasError::Failed(detail) => f.write_str(detail),
        }
    }
}

impl std::error::Error for CanvasError {}

/// Lets `?` carry a Canvas error out of the many `Result<_, String>` callers.
impl From<CanvasError> for String {
    fn from(error: CanvasError) -> String {
        error.to_string()
    }
}
