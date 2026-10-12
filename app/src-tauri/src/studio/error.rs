//! Why a narration clip could not be decoded or timed, worded for whoever
//! has to fix it.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum NarrationError {
    /// The aligner model is not downloaded.
    ModelMissing,
    /// The script has no word with a letter in it.
    EmptyScript,
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
                "the narration aligner model is not downloaded (378 MB, once); download it in Settings -> Voice or run `oculus studio aligner download`"
            ),
            Self::EmptyScript => write!(
                f,
                "the narration has no words to time: every word is punctuation or symbols"
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
