//! The seam between the pipeline and an engine: the segment an engine
//! returns, why it can fail, and the engines' names.

use std::path::Path;

/// One timed stretch of speech, in seconds from the start of the audio
/// handed to the engine.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Why an engine produced nothing. The first two are the ones a later engine
/// in the fallback order may answer instead.
#[derive(Debug)]
pub enum EngineError {
    /// No key or server configured for this engine.
    NotConfigured(String),
    /// The engine's own limit, with when to try again in the message.
    RateLimited(String),
    /// Anything else, including a refused key.
    Failed(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured(m) | Self::RateLimited(m) | Self::Failed(m) => f.write_str(m),
        }
    }
}

pub trait Engine {
    /// One of [`ENGINES`], as `--engine`, the progress event and the CLI name it.
    fn name(&self) -> &'static str;
    /// The largest audio file one call accepts; `None` reads any length.
    fn max_upload_bytes(&self) -> Option<u64>;
    /// Blocks for the whole round trip; nothing above it adds a deadline.
    fn transcribe(&self, audio: &Path) -> Result<Vec<Segment>, EngineError>;
}

/// Every engine, in the default order.
pub const ENGINES: [&str; 3] = ["groq", "whisper", "apple"];

/// How the CLI and the app's messages name an engine.
pub fn engine_label(name: &str) -> &'static str {
    match name {
        "groq" => "Groq",
        "apple" => "on-device speech",
        "whisper" => "local Whisper",
        _ => "an unknown engine",
    }
}

pub(super) const NO_ENGINE: &str =
    "No transcription engine — add a Groq key, download a Whisper model, \
                         or turn on on-device speech, in Settings → Transcription";
