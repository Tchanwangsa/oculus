//! `narrate`: makes each narrated scene's clip and word times and writes
//! `narration/manifest.json`, which studio-core reads. One scene at a time,
//! in order: a scene whose cache key is unchanged and whose clip is on disk
//! is kept; any other is synthesised, timed and saved, and the manifest is
//! rewritten after each, so a failure keeps the scenes already done.
//! Afterwards entries for ids no longer narrated, and clips nothing
//! references, are removed. Rules: docs/studio.md, "Narration is made by
//! `oculus studio narrate`".

mod key;
mod prune;
mod report;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Instant;

use studio_core::{Manifest, ManifestGap, ManifestScene, ManifestWord, Project, MANIFEST_PATH};

use super::audio::resample_to_16k;
use super::timing::{self, Alignment};
use super::voice::Voice;
use super::{tts, NarrationError};
pub use key::cache_key;
pub use report::{GapReport, Outcome, Progress, Report, SceneReport};

/// Times a clip's words. [`ModelAligner`] runs the wav2vec2 model; tests
/// stub it.
pub trait Aligner {
    /// Ok when [`Aligner::align`] can run; asked once, before the first TTS
    /// request, so a missing model costs no synthesis.
    fn ready(&self) -> Result<(), NarrationError>;
    /// `pcm` is mono 16 kHz.
    fn align(&self, pcm: &[f32], words: &[String]) -> Result<Alignment, NarrationError>;
}

/// The downloaded aligner (or `STUDIO_ALIGN_MODEL`'s).
pub struct ModelAligner;

impl Aligner for ModelAligner {
    fn ready(&self) -> Result<(), NarrationError> {
        timing::resolve_model().map(drop)
    }

    fn align(&self, pcm: &[f32], words: &[String]) -> Result<Alignment, NarrationError> {
        timing::align(pcm, words)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Only this scene id.
    pub scene: Option<String>,
    /// Make the clip again even when the cached one matches.
    pub force: bool,
}

/// Why narration stopped, and in which scene.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrateError {
    pub scene: Option<String>,
    pub message: String,
}

impl NarrateError {
    fn video(message: impl Into<String>) -> Self {
        Self {
            scene: None,
            message: message.into(),
        }
    }

    fn scene(id: &str, message: impl Into<String>) -> Self {
        Self {
            scene: Some(id.to_owned()),
            message: message.into(),
        }
    }
}

impl fmt::Display for NarrateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.scene {
            Some(id) => write!(f, "scene `{id}`: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for NarrateError {}

/// Narrate the video in `dir` with `voice`; see the module doc.
pub fn narrate(
    dir: &Path,
    voice: &Voice,
    aligner: &dyn Aligner,
    options: &Options,
    progress: &mut dyn FnMut(Progress),
) -> Result<Report, NarrateError> {
    let project = Project::load_unnarrated(dir).map_err(|e| NarrateError::video(e.to_string()))?;
    if project.script.is_none() {
        return Err(NarrateError::video(format!(
            "{} has no script.md; write one `## <scene id>` section per narrated scene",
            dir.display()
        )));
    }
    let narrated = narrated_scenes(&project);
    let todo = match &options.scene {
        None => narrated.clone(),
        Some(id) => {
            let one: Vec<_> = narrated.iter().filter(|(s, _)| s == id).cloned().collect();
            if one.is_empty() {
                let ids: Vec<_> = narrated.iter().map(|(s, _)| format!("`{s}`")).collect();
                return Err(NarrateError::video(format!(
                    "no narrated scene `{id}`; the narrated scenes are {}",
                    if ids.is_empty() {
                        "none".into()
                    } else {
                        ids.join(", ")
                    }
                )));
            }
            one
        }
    };

    let (mut manifest, reset) = match Manifest::read(dir) {
        Ok(Some(m)) => (m, false),
        Ok(None) => (empty_manifest(), false),
        Err(e) => {
            progress(Progress::Reset(&e.to_string()));
            (empty_manifest(), true)
        }
    };
    let clips = dir.join("narration");
    let mut report = Report::default();
    let mut ready = false;
    for (n, (id, text)) in todo.iter().enumerate() {
        progress(Progress::Start {
            id,
            n: n + 1,
            of: todo.len(),
        });
        let key = cache_key(text, voice);
        let cached = manifest
            .scenes
            .get(id)
            .filter(|s| !options.force && s.key == key && clips.join(&s.audio).is_file());
        let scene = match cached {
            Some(clip) => SceneReport::new(id, Outcome::Cached, clip, None),
            None => {
                if !ready {
                    aligner
                        .ready()
                        .map_err(|e| NarrateError::video(e.to_string()))?;
                    ready = true;
                }
                let (clip, timings) = make_clip(&clips, id, text, &key, voice, aligner)?;
                let scene = SceneReport::new(id, Outcome::Narrated, &clip, Some(timings));
                manifest.scenes.insert(id.clone(), clip);
                write_manifest(dir, &manifest)?;
                scene
            }
        };
        progress(Progress::Done(&scene));
        report.scenes.push(scene);
    }

    let keep: BTreeSet<&str> = narrated.iter().map(|(id, _)| id.as_str()).collect();
    report.dropped = prune::stale_entries(&mut manifest, &keep);
    if reset || !report.dropped.is_empty() {
        write_manifest(dir, &manifest)?;
    }
    report.deleted = prune::orphan_clips(&clips, &manifest).map_err(NarrateError::video)?;
    Ok(report)
}

/// `(id, text)` of each scene with a script section, in video order.
fn narrated_scenes(project: &Project) -> Vec<(String, String)> {
    (0..project.video.scenes.len())
        .filter_map(|i| project.section(i))
        .map(|s| (s.id.clone(), s.text.clone()))
        .collect()
}

fn empty_manifest() -> Manifest {
    Manifest {
        version: Manifest::VERSION,
        scenes: Default::default(),
    }
}

/// Milliseconds spent on one scene's synthesis and alignment.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Timings {
    pub synth_ms: u64,
    pub align_ms: u64,
}

/// Synthesise, time and save one scene's clip; its manifest entry.
fn make_clip(
    clips: &Path,
    id: &str,
    text: &str,
    key: &str,
    voice: &Voice,
    aligner: &dyn Aligner,
) -> Result<(ManifestScene, Timings), NarrateError> {
    let fail = |message: String| NarrateError::scene(id, message);
    let started = Instant::now();
    let speech = tts::synthesise(voice, text).map_err(fail)?;
    let synth_ms = started.elapsed().as_millis() as u64;
    let duration = speech.decoded.duration;
    let started = Instant::now();
    let pcm = resample_to_16k(&speech.decoded.samples, speech.decoded.sample_rate)
        .map_err(|e| fail(e.to_string()))?;
    let words: Vec<String> = text.split_whitespace().map(str::to_owned).collect();
    let alignment = aligner
        .align(&pcm, &words)
        .map_err(|e| fail(e.to_string()))?;
    let align_ms = started.elapsed().as_millis() as u64;

    let audio = format!("{key}.wav");
    write_clip(&clips.join(&audio), &speech.wav).map_err(fail)?;
    let scene = ManifestScene {
        key: key.to_owned(),
        text: text.to_owned(),
        audio,
        duration,
        words: alignment
            .words
            .iter()
            .map(|w| ManifestWord {
                text: w.text.clone(),
                start: w.start,
                end: w.end,
                confidence: w.confidence,
            })
            .collect(),
        gaps: alignment
            .gaps
            .iter()
            .map(|g| ManifestGap {
                after: g.after,
                start: g.start,
                end: g.end,
                heard: g.heard.clone(),
            })
            .collect(),
    };
    Ok((scene, Timings { synth_ms, align_ms }))
}

fn write_clip(path: &Path, wav: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let tmp = PathBuf::from(format!("{}.part", path.display()));
    crate::runtime::atomic_write::write(path, &tmp, wav)
}

fn write_manifest(dir: &Path, manifest: &Manifest) -> Result<(), NarrateError> {
    crate::runtime::atomic_write::json(&dir.join(MANIFEST_PATH), manifest)
        .map_err(|e| NarrateError::video(format!("writing {MANIFEST_PATH}: {e}")))
}
