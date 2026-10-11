//! One run: resolve the video, extract and cut its audio, hand it to each
//! engine in turn, and write the VTT beside the video.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::audio;
use super::engine::{Engine, EngineError, Segment};
use super::settings::engines;
use super::vtt;

/// Where a run is, for a progress line. `chunk` counts from 1.
pub enum Step {
    Extracting,
    /// Starts again from chunk 1 when a run falls through to the next engine.
    Transcribing {
        engine: &'static str,
        chunk: usize,
        chunks: usize,
    },
}

pub struct Outcome {
    /// The VTT written, `<video>.vtt`.
    pub path: PathBuf,
    /// The engine that heard it, one of [`ENGINES`].
    pub engine: &'static str,
    pub chunks: usize,
    pub cues: usize,
}

/// A video inside the data directory, given absolute or relative to it.
/// Symlinks and `..` are resolved before the check, so neither escapes it.
pub fn resolve(data_dir: &Path, video: &str) -> Result<PathBuf, String> {
    let given = Path::new(video);
    let joined = if given.is_absolute() {
        given.to_path_buf()
    } else {
        data_dir.join(given)
    };
    let real = joined
        .canonicalize()
        .map_err(|_| format!("{video} is not on disk — download it, then transcribe"))?;
    let root = data_dir
        .canonicalize()
        .map_err(|e| format!("{}: {e}", data_dir.display()))?;
    if !real.starts_with(&root) {
        return Err(format!(
            "{video} is outside the library — only library files are transcribed"
        ));
    }
    if !real.is_file() {
        return Err(format!("{video} is not a file"));
    }
    Ok(real)
}

/// `Matrices Part 1.mp4` → `Matrices Part 1.mp4.vtt`, beside it.
pub fn vtt_path(video: &Path) -> PathBuf {
    let mut name = video.file_name().unwrap_or_default().to_os_string();
    name.push(".vtt");
    video.with_file_name(name)
}

/// Videos with a run in flight in this process. Two runs on one file would
/// spend the engine's allowance twice for one transcript.
static IN_FLIGHT: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

struct Claim(PathBuf);

impl Claim {
    fn take(video: &Path) -> Result<Self, String> {
        let mut held = IN_FLIGHT.lock().unwrap();
        if !held
            .get_or_insert_with(HashSet::new)
            .insert(video.to_path_buf())
        {
            return Err("that video is already being transcribed".into());
        }
        Ok(Self(video.to_path_buf()))
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        if let Some(held) = IN_FLIGHT.lock().unwrap().as_mut() {
            held.remove(&self.0);
        }
    }
}

/// A private temp directory for the run's audio, removed however it ends.
/// Outside the library, so no folder scan ever sees a half-written file.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Self, String> {
        let dir = std::env::temp_dir().join(format!(
            "oculus-transcribe-{}-{}",
            std::process::id(),
            crate::runtime::clock::now_nanos()
        ));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(Self(dir))
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Transcribe `video` (see [`resolve`]) into [`vtt_path`] of it, with the
/// first engine that answers, or with `only` that one. `resource_dir` is
/// where a bundled app's helpers are; the CLI passes `None`.
pub fn run(
    data_dir: &Path,
    video: &str,
    ffmpeg: &Path,
    resource_dir: Option<PathBuf>,
    only: Option<&str>,
    step: impl Fn(Step),
) -> Result<Outcome, String> {
    let video = resolve(data_dir, video)?;
    // Before any decoding, so a missing key answers at once.
    let engines = engines(resource_dir, ffmpeg, only)?;
    let _claim = Claim::take(&video)?;
    let scratch = Scratch::new()?;

    step(Step::Extracting);
    let audio = scratch.file(&format!("audio{}", audio::EXTENSION));
    let extracted = audio::extract(ffmpeg, &video, &audio)?;
    let cut = |engine: &str, index: usize, span: &audio::Span| {
        let part = scratch.file(&format!("part-{engine}-{index}{}", audio::EXTENSION));
        audio::cut(ffmpeg, &audio, &part, span).map(|()| part)
    };
    let heard = transcribe_audio(&engines, &extracted, &audio, cut, &step)?;
    let segments = heard.segments;

    let (body, cues) = vtt::render(&segments);
    // An empty transcript would read as "this video has no speech".
    if cues == 0 {
        return Err("the transcription came back with no speech in it — nothing was saved".into());
    }
    let path = vtt_path(&video);
    // Beside the target: the rename must stay on one filesystem.
    let mut tmp = path.clone().into_os_string();
    tmp.push(format!(
        ".tmp-{}-{}",
        std::process::id(),
        crate::runtime::clock::now_nanos()
    ));
    crate::runtime::atomic_write::write(&path, Path::new(&tmp), body.as_bytes())?;
    Ok(Outcome {
        path,
        engine: heard.engine,
        chunks: heard.chunks,
        cues,
    })
}

struct Heard {
    segments: Vec<Segment>,
    engine: &'static str,
    chunks: usize,
}

/// Each engine in turn over the extracted audio, cut to that engine's cap.
/// One that declines (`NotConfigured`, `RateLimited`) at any chunk hands over
/// to the next, and what it heard is dropped; `Failed` ends the run.
fn transcribe_audio(
    engines: &[Box<dyn Engine>],
    extracted: &audio::Extracted,
    audio: &Path,
    cut: impl Fn(&str, usize, &audio::Span) -> Result<PathBuf, String>,
    step: &impl Fn(Step),
) -> Result<Heard, String> {
    let mut declined = Vec::new();
    'engines: for engine in engines {
        let spans = audio::plan(
            extracted.bytes,
            extracted.duration,
            engine.max_upload_bytes(),
        )?;
        let chunks = spans.len();
        let mut segments = Vec::new();
        for (index, span) in spans.iter().enumerate() {
            let part = if chunks == 1 {
                audio.to_path_buf()
            } else {
                cut(engine.name(), index, span)?
            };
            step(Step::Transcribing {
                engine: engine.name(),
                chunk: index + 1,
                chunks,
            });
            let in_part = |e: EngineError| {
                if chunks > 1 {
                    format!("{e} (part {} of {chunks})", index + 1)
                } else {
                    e.to_string()
                }
            };
            match engine.transcribe(&part) {
                Ok(found) => segments.extend(shift(found, span.start)),
                Err(e @ EngineError::Failed(_)) => return Err(in_part(e)),
                Err(e) => {
                    declined.push(in_part(e));
                    continue 'engines;
                }
            }
        }
        return Ok(Heard {
            segments,
            engine: engine.name(),
            chunks,
        });
    }
    Err(match declined.len() {
        1 => declined.remove(0),
        _ => format!(
            "no engine could transcribe this video — {}",
            declined.join("; ")
        ),
    })
}

/// A chunk's segments are timed from the chunk's own start.
fn shift(segments: Vec<Segment>, by: f64) -> impl Iterator<Item = Segment> {
    segments.into_iter().map(move |s| Segment {
        start: s.start + by,
        end: s.end + by,
        text: s.text,
    })
}

#[cfg(test)]
mod tests;
