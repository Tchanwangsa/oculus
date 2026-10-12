//! `narrate`: makes each narrated scene's clip and word times and writes
//! `narration/manifest.json`, which studio-core reads. One scene at a time,
//! in order: a scene whose cache key is unchanged and whose clip is on disk
//! is kept; any other is synthesised, timed and saved, and the manifest is
//! rewritten after each, so a failure keeps the scenes already done. A
//! section's `[pause]`s are exact silence spliced into its clip between
//! the words either side (`clip`).
//! Afterwards entries for ids no longer narrated, and clips nothing
//! references, are removed. Rules: docs/studio.md, "Narration is made by
//! `oculus studio narrate`".

mod clip;
mod key;
mod prune;
mod report;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use studio_core::{Library, Manifest, ManifestScene, Project, MANIFEST_PATH};

use super::timing::{self, Alignment};
use super::tts::{self, Speech};
use super::voice::ServerVoice;
use super::NarrationError;
pub use key::{cache_key, server_key};
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

/// Makes a scene's clip. [`ServerSpeaker`] asks a TTS server,
/// `tts::kokoro::KokoroSpeaker` runs Kokoro; tests stub it.
pub trait Speaker {
    /// Ok when [`Speaker::speak`] can run; asked once, beside
    /// [`Aligner::ready`], before the first clip.
    fn ready(&self) -> Result<(), String>;
    /// What a clip of `text` is made from besides the text and the aligner,
    /// engine first, for its cache key ([`cache_key`]); asked for every
    /// scene, cached or not.
    fn key(&self, text: &str) -> Result<Vec<String>, String>;
    /// One clip of `text`: a WAV and its decoded samples, never silent.
    fn speak(&self, text: &str) -> Result<Speech, String>;
}

/// The OpenAI-compatible TTS server `voice` names (`studio::tts`).
pub struct ServerSpeaker<'a>(pub &'a ServerVoice);

impl Speaker for ServerSpeaker<'_> {
    /// A server is only known to answer by asking it for a clip.
    fn ready(&self) -> Result<(), String> {
        Ok(())
    }

    fn key(&self, _text: &str) -> Result<Vec<String>, String> {
        Ok(server_key(self.0))
    }

    fn speak(&self, text: &str) -> Result<Speech, String> {
        tts::synthesise(self.0, text)
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

/// Narrate the video in `dir` with `speaker`, which also keys the clips,
/// loading it over `library` (the app's are `studio::libraries::libraries`),
/// so a library theme or component resolves as it does for every other
/// command; see the module doc.
pub fn narrate(
    dir: &Path,
    library: &Library,
    speaker: &dyn Speaker,
    aligner: &dyn Aligner,
    options: &Options,
    progress: &mut dyn FnMut(Progress),
) -> Result<Report, NarrateError> {
    let project = Project::load_unnarrated_with(dir, library)
        .map_err(|e| NarrateError::video(e.to_string()))?;
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
            let one: Vec<_> = narrated.iter().filter(|s| &s.id == id).cloned().collect();
            if one.is_empty() {
                let ids: Vec<_> = narrated.iter().map(|s| format!("`{}`", s.id)).collect();
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
    for (n, said) in todo.iter().enumerate() {
        let id = &said.id;
        progress(Progress::Start {
            id,
            n: n + 1,
            of: todo.len(),
        });
        let key = said.key(speaker).map_err(|e| NarrateError::scene(id, e))?;
        let cached = manifest
            .scenes
            .get(id)
            .filter(|s| !options.force && s.key == key && clips.join(&s.audio).is_file());
        let scene = match cached {
            Some(clip) => SceneReport::new(id, Outcome::Cached, clip, None),
            None => {
                if !ready {
                    speaker.ready().map_err(NarrateError::video)?;
                    aligner
                        .ready()
                        .map_err(|e| NarrateError::video(e.to_string()))?;
                    ready = true;
                }
                let (clip, timings) = make_clip(&clips, said, &key, speaker, aligner)?;
                let scene = SceneReport::new(id, Outcome::Narrated, &clip, Some(timings));
                manifest.scenes.insert(id.clone(), clip);
                write_manifest(dir, &manifest)?;
                scene
            }
        };
        progress(Progress::Done(&scene));
        report.scenes.push(scene);
    }

    let keep: BTreeSet<&str> = narrated.iter().map(|s| s.id.as_str()).collect();
    report.dropped = prune::stale_entries(&mut manifest, &keep);
    if reset || !report.dropped.is_empty() {
        write_manifest(dir, &manifest)?;
    }
    report.deleted = prune::orphan_clips(&clips, &manifest).map_err(NarrateError::video)?;
    Ok(report)
}

/// A narrated scene as its clip is made: the text its manifest entry
/// must match (`ScriptSection::clip_text`), the text said, and each pause
/// as how many words come before it and its seconds.
#[derive(Debug, Clone)]
struct Said {
    id: String,
    text: String,
    spoken: String,
    pauses: Vec<(usize, f64)>,
}

impl Said {
    /// The clip's cache key: the text with its pauses and the speaker's
    /// parts for the words said. Without pauses the two texts are one, so
    /// the key is the one a clip has always had.
    fn key(&self, speaker: &dyn Speaker) -> Result<String, String> {
        Ok(cache_key(&self.text, &speaker.key(&self.spoken)?))
    }
}

/// Each scene with a script section, in video order.
fn narrated_scenes(project: &Project) -> Vec<Said> {
    let pause = project.theme.narration.pause;
    (0..project.video.scenes.len())
        .filter_map(|i| project.section(i))
        .map(|s| Said {
            id: s.id.clone(),
            text: s.clip_text(pause),
            spoken: s.text.clone(),
            pauses: s
                .pauses
                .iter()
                .map(|p| (p.words, p.length(pause)))
                .collect(),
        })
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
    said: &Said,
    key: &str,
    speaker: &dyn Speaker,
    aligner: &dyn Aligner,
) -> Result<(ManifestScene, Timings), NarrateError> {
    let fail = |message: String| NarrateError::scene(&said.id, message);
    let made = clip::make(&said.spoken, &said.pauses, speaker, aligner).map_err(fail)?;
    let audio = format!("{key}.wav");
    write_clip(&clips.join(&audio), &made.wav).map_err(fail)?;
    let scene = ManifestScene {
        key: key.to_owned(),
        text: said.text.clone(),
        audio,
        duration: made.duration,
        words: made.words,
        gaps: made.gaps,
        pauses: made.pauses,
    };
    Ok((scene, made.timings))
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
