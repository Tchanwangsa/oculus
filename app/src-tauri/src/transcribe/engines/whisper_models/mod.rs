//! The local Whisper engine's model files: a fixed catalogue of whisper.cpp's
//! ggml models, which are on disk, how each suits this machine, and their
//! download from Hugging Face (free, no account). They live in
//! `<app data>/models/whisper/`, outside `courses/` and `lectures/`, where
//! folder scans and agents look. Listing reads that directory and the RAM
//! size only; the network is touched only when a download is asked for.

mod download;
mod models;
#[cfg(test)]
mod tests;

pub use download::{cancel, delete, download, CANCELLED};
pub use models::{
    dir, find, list, pick, Catalogue, Fit, Listed, Model, DEFAULT_MODEL, GPU, MODELS, VAD_FILE,
};
