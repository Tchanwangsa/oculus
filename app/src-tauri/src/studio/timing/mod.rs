//! Narration timing: when each script word is spoken in a TTS clip, by CTC
//! forced alignment against wav2vec2's letter emissions (ONNX, CPU), a long
//! clip's emissions computed in windows (`windows`). Ported
//! from the alignment spike, which matched Python frame for frame. Rules
//! and thresholds: docs/studio.md, "Narration timing".

mod align;
mod emissions;
mod gaps;
mod model;
mod targets;
#[cfg(test)]
mod tests;
mod viterbi;
mod windows;

pub use crate::runtime::onnx::CANCELLED;
pub use align::{
    align, align_with, Alignment, Gap, WordTime, FRAME_SECONDS, GAP_LETTER_FRAMES, START_BIAS,
};
pub use model::{
    cancel, delete, dir, download, model_file, resolve_model, status, Status, MODEL_ENV, MODEL_ID,
    PACK,
};
