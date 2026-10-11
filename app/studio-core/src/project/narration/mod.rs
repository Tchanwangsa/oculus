//! `narration/manifest.json`: for each narrated scene, the clip
//! `oculus studio narrate` made for its script text and the clip's word
//! timings. studio-core reads only the manifest, never the audio: a narrated
//! scene's length and its `word()` cues come from here, and a scene whose
//! entry is missing or was made for other text fails instead of guessing.

mod cues;

pub use cues::WordCues;

use crate::error::{Result, StudioError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Where the manifest lives in a project folder.
pub const MANIFEST_PATH: &str = "narration/manifest.json";

/// Below this aligner confidence (the mean log-probability of a word's
/// aligned letter frames) a word's timing is suspect, and lint names the
/// word. Measured on Kokoro: clean words stayed above -4.83, mangled words
/// and their smeared neighbours fell to -12 to -18. Re-check per TTS engine.
pub const LOW_CONFIDENCE: f64 = -8.0;

/// The longest narration clip lint accepts, in seconds: the aligner's
/// memory grows with the square of the clip's length.
pub const MAX_NARRATION_SECS: f64 = 60.0;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Always `Manifest::VERSION`.
    pub version: u32,
    /// By scene id.
    pub scenes: BTreeMap<String, ManifestScene>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ManifestScene {
    /// The clip's content address: sha256 of the text, server, model,
    /// voice, speed and aligner model.
    pub key: String,
    /// The script text the clip says (`ScriptSection::text`).
    pub text: String,
    /// The clip's file, relative to `narration/`.
    pub audio: String,
    /// The clip's length in seconds.
    pub duration: f64,
    pub words: Vec<ManifestWord>,
    /// Speech the aligner heard that the script does not have.
    #[serde(default)]
    pub gaps: Vec<ManifestGap>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ManifestWord {
    pub text: String,
    /// Seconds from the clip's start.
    pub start: f64,
    pub end: f64,
    /// The aligner's confidence in this word's timing; higher is surer
    /// (see `LOW_CONFIDENCE`).
    pub confidence: f64,
}

/// Unscripted speech in a clip: the voice said something between two
/// script words. It does not lower any word's confidence, so it is listed.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ManifestGap {
    /// The index in `words` of the word it follows.
    pub after: usize,
    /// Seconds from the clip's start.
    pub start: f64,
    pub end: f64,
    /// What the aligner heard there.
    pub heard: String,
}

impl Manifest {
    pub const VERSION: u32 = 1;

    /// Reads `<dir>/narration/manifest.json`; `None` when there is none.
    pub fn read(dir: &Path) -> Result<Option<Manifest>> {
        let path = dir.join(MANIFEST_PATH);
        let json = match std::fs::read_to_string(&path) {
            Ok(json) => json,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(StudioError::io(&path, e)),
        };
        let fail = |message: String| StudioError::Project {
            path: path.clone(),
            message,
        };
        let manifest: Manifest = serde_json::from_str(&json).map_err(|e| fail(e.to_string()))?;
        manifest.check().map_err(fail)?;
        Ok(Some(manifest))
    }

    fn check(&self) -> Result<(), String> {
        if self.version != Self::VERSION {
            return Err(format!(
                "version {} is not one this build reads ({}); run `oculus studio narrate` to rewrite it",
                self.version,
                Self::VERSION
            ));
        }
        for (id, s) in &self.scenes {
            if !(s.duration.is_finite() && s.duration >= 0.0) {
                return Err(format!(
                    "scenes.{id}.duration is {}; it must be a non-negative number of seconds",
                    s.duration
                ));
            }
            for (i, w) in s.words.iter().enumerate() {
                let finite = [w.start, w.end, w.confidence].iter().all(|v| v.is_finite());
                if !finite || w.start > w.end {
                    return Err(format!(
                        "scenes.{id}.words[{i}] (`{}`) runs {} to {} s with confidence {}; times must be finite with start <= end",
                        w.text, w.start, w.end, w.confidence
                    ));
                }
            }
            for (i, g) in s.gaps.iter().enumerate() {
                if g.after >= s.words.len()
                    || !(g.start.is_finite() && g.end.is_finite())
                    || g.start > g.end
                {
                    return Err(format!(
                        "scenes.{id}.gaps[{i}] follows word {} of {} and runs {} to {} s; it must follow a word, with finite times and start <= end",
                        g.after,
                        s.words.len(),
                        g.start,
                        g.end
                    ));
                }
            }
        }
        Ok(())
    }
}
