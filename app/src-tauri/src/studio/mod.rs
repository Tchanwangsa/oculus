//! Studio's half that studio-core may not do: the TTS client, audio decode,
//! word timing against a local ONNX model, `narrate`, which writes the
//! manifest studio-core reads, `render`, which encodes a video with ffmpeg,
//! and the Tauri commands behind Settings -> Voice and rendering
//! (docs/studio.md, "Narration is made by `oculus studio narrate`",
//! "Narration timing" and "Rendering a video").

pub mod audio;
pub mod commands;
pub mod error;
pub mod libraries;
pub mod narrate;
pub mod render;
#[cfg(test)]
mod test_wav;
pub mod timing;
pub mod tts;
pub mod voice;

pub use error::NarrationError;
