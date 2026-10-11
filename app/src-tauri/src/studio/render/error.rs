//! Why a render failed, worded for whoever has to fix it. Cancelling is not
//! one of these: `render` returns `Finished::Cancelled`.

use std::fmt;

use studio_core::StudioError;

#[derive(Debug)]
pub enum RenderError {
    /// studio-core refused the project, or a frame failed
    /// (`StudioError::Frame` names its time and scene).
    Studio(StudioError),
    /// ffmpeg could not start or exited non-zero; what it was doing and the
    /// last lines of its stderr.
    Ffmpeg(String),
    /// A file or folder could not be read or written.
    Io(String),
    /// Anything else: an odd frame size, no ffmpeg, a worker that panicked.
    Render(String),
}

impl RenderError {
    /// The variant's name, for JSON.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Studio(_) => "studio",
            Self::Ffmpeg(_) => "ffmpeg",
            Self::Io(_) => "io",
            Self::Render(_) => "render",
        }
    }

    pub(super) fn io(path: &std::path::Path, e: impl fmt::Display) -> Self {
        Self::Io(format!("{}: {e}", path.display()))
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Studio(e) => write!(f, "{e}"),
            Self::Ffmpeg(m) | Self::Io(m) | Self::Render(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<StudioError> for RenderError {
    fn from(e: StudioError) -> Self {
        Self::Studio(e)
    }
}

/// How a step stopped early: a failure, or the caller's cancel.
#[derive(Debug)]
pub(super) enum Stop {
    Failed(RenderError),
    Cancelled,
}

impl From<RenderError> for Stop {
    fn from(e: RenderError) -> Self {
        Self::Failed(e)
    }
}

impl From<StudioError> for Stop {
    fn from(e: StudioError) -> Self {
        Self::Failed(e.into())
    }
}
