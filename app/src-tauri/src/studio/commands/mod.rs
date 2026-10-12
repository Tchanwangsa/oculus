//! Studio's Tauri commands: behind Settings -> Studio, the `voice` row, the
//! TTS server's model list, the aligner model download, Kokoro's packs,
//! voices (`voice_files`) and sample, and the pronunciation review
//! (`pronunciations`), none of which
//! makes a billed call (listing models is free on every server, and
//! Kokoro's samples are synthesised on this Mac); rendering a video
//! (`render`); the Studio page's videos, details and stills (`videos`); and
//! the Components playground's and style chip's reads and writes of the
//! libraries (`components`, `themes`).

pub mod aligner;
pub mod components;
mod history;
pub mod kokoro;
mod levels;
mod progress;
pub mod pronunciations;
pub mod render;
#[cfg(test)]
mod tests;
pub mod themes;
pub mod videos;
pub mod voice;
pub mod voice_files;
