//! What `narrate` tells its caller, per scene and at the end.

use serde::Serialize;
use studio_core::{ManifestScene, ManifestWord, LOW_CONFIDENCE};

use super::Timings;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The clip on disk matched the key; nothing was requested.
    Cached,
    /// Synthesised and timed now.
    Narrated,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SceneReport {
    pub id: String,
    pub outcome: Outcome,
    /// The clip, relative to `narration/`.
    pub audio: String,
    /// Seconds.
    pub duration: f64,
    pub words: usize,
    /// Words below `studio_core::LOW_CONFIDENCE`: lint fails on each.
    pub low_confidence: Vec<ManifestWord>,
    /// Unscripted speech: lint fails on each.
    pub gaps: Vec<GapReport>,
    /// `None` when cached.
    pub timings: Option<Timings>,
}

impl SceneReport {
    pub(super) fn new(
        id: &str,
        outcome: Outcome,
        clip: &ManifestScene,
        timings: Option<Timings>,
    ) -> Self {
        Self {
            id: id.to_owned(),
            outcome,
            audio: clip.audio.clone(),
            duration: clip.duration,
            words: clip.words.len(),
            low_confidence: clip
                .words
                .iter()
                .filter(|w| w.confidence < LOW_CONFIDENCE)
                .cloned()
                .collect(),
            gaps: clip
                .gaps
                .iter()
                .map(|g| GapReport {
                    after: g.after,
                    after_word: g
                        .after
                        .and_then(|i| clip.words.get(i))
                        .map(|w| w.text.clone()),
                    start: g.start,
                    end: g.end,
                    heard: g.heard.clone(),
                })
                .collect(),
            timings,
        }
    }
}

/// A manifest gap with the word it follows spelled out.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GapReport {
    /// Index of that word in the manifest's `words`; `None` before the first.
    pub after: Option<usize>,
    pub after_word: Option<String>,
    /// Seconds from the clip's start.
    pub start: f64,
    pub end: f64,
    pub heard: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Report {
    /// The scenes asked for, in video order.
    pub scenes: Vec<SceneReport>,
    /// Manifest entries removed: their ids are no longer narrated scenes.
    pub dropped: Vec<String>,
    /// Clip files removed from `narration/`: no entry named them.
    pub deleted: Vec<String>,
}

/// Each step as it happens, for a progress line or an app event.
#[derive(Debug)]
pub enum Progress<'a> {
    /// The manifest could not be read (another version, or damaged) and is
    /// being rebuilt from nothing; the reason.
    Reset(&'a str),
    /// Scene `n` of `of` (1-based) begins.
    Start {
        id: &'a str,
        n: usize,
        of: usize,
    },
    Done(&'a SceneReport),
}
