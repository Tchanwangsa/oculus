//! Studio's Tauri commands: behind Settings -> Voice, the `voice` row, the
//! TTS server's model list and the aligner model download, none of which
//! synthesises (Settings never makes a billed call, and listing models is
//! free on every server); rendering a video (`render`); the Studio page's
//! videos, details and stills (`videos`); and the Components playground's and
//! style chip's reads and writes of the libraries (`components`, `themes`).

pub mod aligner;
pub mod components;
mod levels;
pub mod render;
#[cfg(test)]
mod tests;
pub mod themes;
pub mod videos;
pub mod voice;
