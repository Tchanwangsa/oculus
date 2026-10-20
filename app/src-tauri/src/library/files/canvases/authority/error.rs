use serde::Serialize;

/// What went wrong, as a stable code the desktop pane and browser clients act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Code {
    /// An edited element no longer matches the edit's `before`.
    ConflictShape,
    /// The touched elements were reordered by someone else.
    ConflictOrder,
    /// The background is neither the edit's `before` nor its `after`.
    ConflictBackground,
    /// A malformed transaction, document, viewport or request.
    Invalid,
    /// A sequence number below the retained receipt window: resync, keep the draft.
    StaleOp,
    /// No live session: stopped, closed, deleted, or another epoch.
    SessionEnded,
    /// The resulting document exceeds the size bound.
    TooLarge,
    /// The file on disk changed outside the authority; it is paused.
    ExternalEdit,
    /// A filesystem failure; the canonical document is unchanged.
    Io,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthorityError {
    pub code: Code,
    pub message: String,
}

impl AuthorityError {
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(Code::Invalid, message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(Code::Io, message)
    }

    pub fn ended() -> Self {
        Self::new(Code::SessionEnded, "This canvas is no longer shared.")
    }

    pub fn external() -> Self {
        Self::new(
            Code::ExternalEdit,
            "The canvas file changed outside Oculus. Choose which version to keep.",
        )
    }
}

impl std::fmt::Display for AuthorityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AuthorityError {}
