//! Transcription: a video in the library into a WebVTT file beside it
//! (`<video>.vtt`), for recordings that arrive without captions.
//!
//! The pipeline — audio extraction, chunking, offsets, the VTT — belongs to
//! this module; an [`Engine`] only turns one audio file into timed segments.
//! Engines — Groq's hosted Whisper, whisper.cpp with a downloaded model, and
//! Apple's on-device recogniser — are tried in the order set in Settings →
//! Transcription ([`Settings`]). A later engine answers only when an earlier
//! one is unconfigured or rate-limited; any other failure surfaces

pub mod app;
pub(crate) mod audio;
mod engine;
mod engines;
mod pipeline;
mod settings;
mod vtt;

pub use engine::{engine_label, Engine, EngineError, Segment, ENGINES};
pub use engines::whisper_models;
pub use pipeline::{resolve, run, vtt_path, Outcome, Step};
pub(crate) use settings::Settings;
