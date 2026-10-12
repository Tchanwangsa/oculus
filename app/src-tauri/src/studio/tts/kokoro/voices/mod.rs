//! Kokoro's voices: a style file each, `<id>.bin` beside the model, small
//! enough (522,240 bytes) to download one at a time. [`VOICES`] is every
//! voice the app offers; each is its own one-file pack, so a voice
//! downloads, cancels and deletes on its own, hash-checked and atomic
//! (`runtime::onnx`). Only US English voices are listed, as the G2P pack
//! is US-only.

mod catalogue;

use std::path::{Path, PathBuf};

pub use catalogue::VOICES;

use super::model::VOICE_BYTES;
use crate::runtime::onnx::{self, Pack, Status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accent {
    American,
}

impl Accent {
    pub fn label(self) -> &'static str {
        match self {
            Accent::American => "American",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gender {
    Female,
    Male,
}

impl Gender {
    pub fn label(self) -> &'static str {
        match self {
            Gender::Female => "female",
            Gender::Male => "male",
        }
    }
}

pub struct Voice {
    /// The name its file and a narration's `voice` field use: `af_heart`.
    pub id: &'static str,
    /// `Heart`.
    pub name: &'static str,
    pub accent: Accent,
    pub gender: Gender,
    /// Kokoro's published overall grade of its training data's quality and
    /// quantity (`A` to `F`), not a rating of how it sounds.
    pub grade: &'static str,
    /// The voice's one file.
    pub pack: Pack,
}

impl Voice {
    pub fn bytes(&self) -> u64 {
        self.pack.bytes()
    }
}

/// The catalogued voice called `id`.
pub fn find(id: &str) -> Option<&'static Voice> {
    VOICES.iter().find(|v| v.id == id)
}

/// `id`'s style file in `dir`, when it is there at a voice file's size. Any
/// id counts, so a `<id>.bin` put in the folder by hand narrates too.
pub fn file(dir: &Path, id: &str) -> Option<PathBuf> {
    let path = dir.join(format!("{id}.bin"));
    std::fs::metadata(&path)
        .is_ok_and(|m| m.is_file() && m.len() == VOICE_BYTES)
        .then_some(path)
}

/// Whether `voice` is in `dir`, or being downloaded. Local only.
pub fn status(dir: &Path, voice: &Voice) -> Status {
    onnx::status(dir, &voice.pack)
}

/// Download `voice` into `dir` unless it is there, reporting `(received,
/// total)` bytes. Ends in `Err(CANCELLED)` when [`cancel`]led or when `stop`
/// returns true.
pub fn download(
    dir: &Path,
    voice: &Voice,
    progress: impl Fn(u64, u64),
    stop: impl Fn() -> bool,
) -> Result<(), String> {
    onnx::download(dir, &voice.pack, progress, stop)
}

/// Ask `voice`'s download to stop; false when none runs.
pub fn cancel(voice: &Voice) -> bool {
    onnx::cancel(voice.pack.id)
}

/// Delete `voice`'s file and any partial download, stopping its download
/// first; returns the bytes freed.
pub fn delete(dir: &Path, voice: &Voice) -> Result<u64, String> {
    onnx::delete(dir, &voice.pack)
}
