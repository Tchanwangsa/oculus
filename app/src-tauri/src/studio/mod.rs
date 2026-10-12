//! Studio's half that studio-core may not do: the TTS client, audio decode,
//! word timing against a local ONNX model, `narrate`, which writes the
//! manifest studio-core reads, `render`, which encodes a video with ffmpeg,
//! the data directory's libraries and videos, `brief`, a scene builder's
//! input in one read, `slides`, crops of library PDF pages as video assets,
//! and the Tauri commands behind
//! Settings -> Studio and rendering
//! (docs/studio.md, "Narration is made by `oculus studio narrate`",
//! "Narration timing" and "Rendering a video").

pub mod audio;
pub mod brief;
pub mod commands;
pub mod error;
pub mod libraries;
pub mod narrate;
pub mod render;
pub mod slides;
#[cfg(test)]
mod test_wav;
pub mod timing;
pub mod tts;
pub mod videos;
pub mod voice;

pub use error::NarrationError;
