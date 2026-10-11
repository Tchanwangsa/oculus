//! Studio's Tauri commands: behind Settings -> Voice, the `voice` row, the
//! TTS server's model list and the aligner model download, none of which
//! synthesises (Settings never makes a billed call, and listing models is
//! free on every server); and rendering a video (`render`).

pub mod aligner;
pub mod render;
#[cfg(test)]
mod tests;
pub mod voice;
