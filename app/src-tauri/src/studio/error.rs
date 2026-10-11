//! Why a narration clip could not be decoded or timed, worded for whoever
//! has to fix it.

use std::fmt;

use studio_core::MAX_NARRATION_SECS;

#[derive(Debug, Clone, PartialEq)]
pub enum NarrationError {
    /// The aligner model is not downloaded.
    ModelMissing,
    /// The script has no word with a letter in it.
    EmptyScript,
    /// The clip is over `MAX_NARRATION_SECS`.
    TooLong { seconds: f64 },
    /// ONNX Runtime failed to load or run the model.
    Onnx(String),
    /// The audio is not a WAV hound reads, or could not be resampled.
    Decode(String),
    /// No path through the clip spells the script, e.g. a clip too short
    /// for its letters.
    Unalignable(String),
}

impl fmt::Display for NarrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelMissing => write!(
                f,
                "the narration aligner model is not downloaded (378 MB, once); run `oculus studio aligner download`"
            ),
            Self::EmptyScript => write!(
                f,
                "the narration has no words to time: every word is punctuation or symbols"
            ),
            Self::TooLong { seconds } => write!(
                f,
                "the clip is {seconds:.1} s; narration clips over {MAX_NARRATION_SECS} s are refused because timing them needs gigabytes of memory; split the paragraph"
            ),
            Self::Onnx(e) => write!(f, "the aligner failed: {e}"),
            Self::Decode(e) => write!(f, "could not decode the narration audio: {e}"),
            Self::Unalignable(e) => write!(f, "could not time the narration: {e}"),
        }
    }
}

impl std::error::Error for NarrationError {}

impl From<NarrationError> for String {
    fn from(e: NarrationError) -> Self {
        e.to_string()
    }
}
