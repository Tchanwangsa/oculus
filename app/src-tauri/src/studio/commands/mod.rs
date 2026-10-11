//! Studio's Tauri commands: behind Settings -> Voice, the `voice` row, the
//! TTS server's model list and the aligner model download, none of which
//! synthesises (Settings never makes a billed call, and listing models is
//! free on every server); rendering a video (`render`); and the Studio page's
//! videos, details and stills (`videos`).

pub mod aligner;
pub mod render;
#[cfg(test)]
mod tests;
pub mod videos;
pub mod voice;
