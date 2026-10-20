use serde::Serialize;

/// What a sharing command could not do, as `{code, message}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Code {
    /// The canvas has no live authority session: the pane attaches first.
    NotLive,
    /// No port could be bound, or the server could not start.
    Server,
    /// The system random source failed, so no secret could be made.
    Random,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompanionError {
    pub code: Code,
    pub message: String,
}

impl CompanionError {
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn not_live() -> Self {
        Self::new(
            Code::NotLive,
            "This canvas is not open for sharing. Open it and try again.",
        )
    }
}

impl std::fmt::Display for CompanionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CompanionError {}
