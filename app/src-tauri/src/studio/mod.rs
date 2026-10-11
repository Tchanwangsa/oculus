//! Studio's narration, the half studio-core may not do: the TTS client, audio
//! decode, word timing against a local ONNX model, `narrate`, which writes
//! the manifest studio-core reads, and the Tauri commands behind Settings ->
//! Voice (docs/studio.md, "Narration is made by `oculus studio narrate`" and
//! "Narration timing").

pub mod audio;
pub mod commands;
pub mod error;
pub mod narrate;
#[cfg(test)]
mod test_wav;
pub mod timing;
pub mod tts;
pub mod voice;

pub use error::NarrationError;
