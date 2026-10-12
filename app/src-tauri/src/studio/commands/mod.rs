//! Studio's Tauri commands: behind Settings -> Voice, the `voice` row, the
//! TTS server's model list, the aligner model download, Kokoro's packs and
//! sample, and the pronunciation review (`pronunciations`), none of which
//! makes a billed call (listing models is free on every server, and
//! Kokoro's samples are synthesised on this Mac); and rendering a video
//! (`render`).

pub mod aligner;
pub mod kokoro;
mod progress;
pub mod pronunciations;
pub mod render;
#[cfg(test)]
mod tests;
pub mod voice;
