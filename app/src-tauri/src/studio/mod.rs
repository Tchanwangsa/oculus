//! Studio's narration, the half studio-core may not do: audio decode and
//! word timing against a local ONNX model. studio-core reads only the
//! manifest this side writes (docs/studio.md, "Narration timing").

pub mod audio;
pub mod error;
pub mod timing;

pub use error::NarrationError;
