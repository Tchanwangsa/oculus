//! The three engines: Groq's hosted Whisper, whisper.cpp with a downloaded
//! model, and Apple's on-device recogniser.

pub(super) mod apple;
pub(super) mod groq;
pub(super) mod whisper;
pub mod whisper_models;
