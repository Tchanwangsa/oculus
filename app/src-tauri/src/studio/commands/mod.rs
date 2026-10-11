//! The Tauri commands behind Settings -> Voice: the `voice` row, the TTS
//! server's model list and the aligner model download. Nothing here
//! synthesises: Settings never makes a billed call, and listing models is
//! free on every server.

pub mod aligner;
#[cfg(test)]
mod tests;
pub mod voice;
