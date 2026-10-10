//! Transcription: a video in the library into a WebVTT file beside it
//! (`<video>.vtt`), for recordings that arrive without captions.
//!
//! The pipeline — audio extraction, chunking, offsets, the VTT — belongs to
//! this module; an [`Engine`] only turns one audio file into timed segments.
//! Engines — Groq's hosted Whisper, whisper.cpp with a downloaded model, and
//! Apple's on-device recogniser — are tried in the order set in Settings →
//! Transcription ([`Settings`]). A later engine answers only when an earlier
//! one is unconfigured or rate-limited; any other failure surfaces
//! (docs/viewers.md#videos-without-captions-are-transcribed).

mod apple;
pub(crate) mod audio;
mod groq;
mod vtt;
mod whisper;
pub mod whisper_models;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// One timed stretch of speech, in seconds from the start of the audio
/// handed to the engine.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Why an engine produced nothing. The first two are the ones a later engine
/// in the fallback order may answer instead.
#[derive(Debug)]
pub enum EngineError {
    /// No key or server configured for this engine.
    NotConfigured(String),
    /// The engine's own limit, with when to try again in the message.
    RateLimited(String),
    /// Anything else, including a refused key.
    Failed(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured(m) | Self::RateLimited(m) | Self::Failed(m) => f.write_str(m),
        }
    }
}

pub trait Engine {
    /// One of [`ENGINES`], as `--engine`, the progress event and the CLI name it.
    fn name(&self) -> &'static str;
    /// The largest audio file one call accepts; `None` reads any length.
    fn max_upload_bytes(&self) -> Option<u64>;
    /// Blocks for the whole round trip; nothing above it adds a deadline.
    fn transcribe(&self, audio: &Path) -> Result<Vec<Segment>, EngineError>;
}

/// Every engine, in the default order.
pub const ENGINES: [&str; 3] = ["groq", "whisper", "apple"];

/// How the CLI and the app's messages name an engine.
pub fn engine_label(name: &str) -> &'static str {
    match name {
        "groq" => "Groq",
        "apple" => "on-device speech",
        "whisper" => "local Whisper",
        _ => "an unknown engine",
    }
}

const NO_ENGINE: &str = "No transcription engine — add a Groq key, download a Whisper model, \
                         or turn on on-device speech, in Settings → Transcription";

/// The `transcribe` settings row, which Settings → Transcription writes
/// (`parseTranscribeSettings` in `app/src/lib/transcribe.ts` mirrors it):
///
/// `{"order": [...], "language": string, "groq": {"enabled"},
///   "whisper": {"enabled", "model"}, "apple": {"enabled"}}`
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Settings {
    /// Each of [`ENGINES`] once, in the order a run tries them.
    pub order: Vec<&'static str>,
    pub language: Language,
    pub groq: bool,
    pub whisper: bool,
    /// A catalogue id; `None` lets [`whisper_models::pick`] choose.
    pub whisper_model: Option<String>,
    pub apple: bool,
}

/// The one language every engine transcribes in: `auto`, a locale such as
/// `en_AU`, or a bare language code; `None` is the default, English in the
/// Mac's region.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Language(Option<String>);

impl Language {
    /// Whisper's `-l`: the locale's language part, or `auto`. The default is
    /// English because auto-detection judges from the first 30 s heard, which
    /// is easily noise or someone else's chatter.
    pub fn whisper(&self) -> String {
        match &self.0 {
            None => "en".into(),
            Some(l) => l.split(['_', '-']).next().unwrap_or(l).to_lowercase(),
        }
    }

    /// Groq's `language` field, an ISO-639-1 code; `None` lets it detect.
    pub fn groq(&self) -> Option<String> {
        Some(self.whisper()).filter(|code| code != "auto")
    }

    /// The helper's `--locale`. `None` is its default — English in the Mac's
    /// region — which also stands in for auto-detect, since Apple has none.
    pub fn apple(&self) -> Option<String> {
        self.0
            .clone()
            .filter(|l| l != "auto" && !l.eq_ignore_ascii_case("en"))
    }
}

/// A missing row or key, or a value of the wrong type, reads as its default:
/// the default order, every engine on, the default language, the model
/// `pick` chooses. Without a top-level `language`, the older per-engine
/// `apple.locale`, then `whisper.language`, stands in for it.
pub(crate) fn settings_from(row: Option<&str>) -> Settings {
    let row = row.and_then(|row| serde_json::from_str::<serde_json::Value>(row).ok());
    let at = |path: &[&str]| {
        path.iter()
            .try_fold(row.as_ref()?, |value, key| value.get(key))
    };
    let text = |path: &[&str]| {
        at(path)
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let enabled = |engine: &str| {
        at(&[engine, "enabled"])
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    };
    let language = text(&["language"])
        .or_else(|| text(&["apple", "locale"]))
        .or_else(|| text(&["whisper", "language"]))
        .map(|l| {
            if l.eq_ignore_ascii_case("auto") {
                "auto".into()
            } else {
                l
            }
        });
    Settings {
        order: order_from(at(&["order"])),
        language: Language(language),
        groq: enabled("groq"),
        whisper: enabled("whisper"),
        whisper_model: text(&["whisper", "model"]),
        apple: enabled("apple"),
    }
}

/// Known names in the stored order, each once, then any left out in the
/// default order.
fn order_from(stored: Option<&serde_json::Value>) -> Vec<&'static str> {
    let mut order: Vec<&'static str> = Vec::new();
    let named = stored
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str());
    for name in named.chain(ENGINES) {
        if let Some(&engine) = ENGINES.iter().find(|&&e| e == name) {
            if !order.contains(&engine) {
                order.push(engine);
            }
        }
    }
    order
}

const NO_GROQ_KEY: &str = "No Groq API key — add one in Settings → Transcription";

/// Groq as Settings has it: through keyd when it is installed (in
/// `data_dir`), else with the keychain's key from `key`. Nothing is asked
/// while Groq is off, so a switched-off engine never touches keyd or the
/// keychain.
fn groq_engine(
    settings: &Settings,
    data_dir: &Path,
    key: impl FnOnce() -> Result<Option<String>, String>,
) -> Result<groq::Groq, String> {
    if !settings.groq {
        return Err("Groq is turned off in Settings → Transcription".into());
    }
    let broker = crate::credentials::Credentialed::at(data_dir);
    let auth = match broker.has(crate::groq::SECRET) {
        Ok(true) => groq::Auth::Keyd(broker),
        Ok(false) => return Err(NO_GROQ_KEY.into()),
        Err(crate::credentials::KeydError::Absent) => {
            let key = key()
                .map_err(|e| format!("The keychain refused to give out the Groq API key ({e})"))?
                .ok_or(NO_GROQ_KEY)?;
            groq::Auth::Direct(key)
        }
        Err(crate::credentials::KeydError::Keychain(e)) => {
            return Err(format!(
                "The keychain refused to give out the Groq API key ({e})"
            ))
        }
        Err(e) => {
            return Err(format!(
                "oculus-keyd, which holds the Groq API key, could not check it: {e}"
            ))
        }
    };
    Ok(groq::Groq::new(auth, settings.language.groq()))
}

/// The configured engines in the order set in Settings, or `only` that one.
/// With none, the error says what to set up.
fn engines(
    resource_dir: Option<PathBuf>,
    ffmpeg: &Path,
    only: Option<&str>,
) -> Result<Vec<Box<dyn Engine>>, String> {
    if let Some(name) = only.filter(|name| !ENGINES.contains(name)) {
        return Err(format!(
            "no engine called {name} — it is {}",
            ENGINES.join(", ")
        ));
    }
    let settings = settings_from(crate::store::setting_blocking("transcribe").as_deref());
    let mut found: Vec<Box<dyn Engine>> = Vec::new();
    let mut missing = Vec::new();
    for &name in settings
        .order
        .iter()
        .filter(|&&name| only.is_none_or(|only| only == name))
    {
        let engine: Result<Box<dyn Engine>, String> = match name {
            "groq" => groq_engine(
                &settings,
                &crate::paths::data_dir(),
                crate::groq::fetch_api_key,
            )
            .map(|e| Box::new(e) as Box<dyn Engine>),
            "whisper" => whisper::Whisper::configured(&settings, resource_dir.clone(), ffmpeg)
                .map(|e| Box::new(e) as Box<dyn Engine>),
            _ => apple::Apple::configured(&settings, resource_dir.clone())
                .map(|e| Box::new(e) as Box<dyn Engine>),
        };
        match engine {
            Ok(engine) => found.push(engine),
            Err(reason) => missing.push(reason),
        }
    }
    match (found.is_empty(), only) {
        (false, _) => Ok(found),
        (true, Some(_)) => Err(missing.join("; ")),
        (true, None) => Err(NO_ENGINE.into()),
    }
}

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
            crate::clock::now_nanos()
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
        crate::clock::now_nanos()
    ));
    crate::atomic_write::write(&path, Path::new(&tmp), body.as_bytes())?;
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

pub mod app {
    use super::*;
    use std::cell::Cell;
    use tauri::{AppHandle, Emitter, Manager};

    /// Display only, never persisted.
    pub const PROGRESS_EVENT: &str = "transcribe-progress";

    #[derive(serde::Serialize, Clone)]
    #[serde(rename_all = "camelCase")]
    struct Progress<'a> {
        /// The path exactly as the caller passed it.
        path: &'a str,
        /// `extracting`, `transcribing`, `complete` or `error`.
        phase: &'static str,
        /// One of [`ENGINES`], while transcribing and once complete.
        #[serde(skip_serializing_if = "Option::is_none")]
        engine: Option<&'static str>,
        /// From 1 while transcribing; 0 while extracting.
        chunk: usize,
        chunks: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<&'a str>,
    }

    /// Transcribe a library video and return the absolute path of the VTT
    /// written beside it. Records nothing in the database.
    #[tauri::command]
    pub async fn transcribe_video(app: AppHandle, path: String) -> Result<String, String> {
        let resource_dir = app.path().resource_dir().ok();
        crate::blocking::run(move || {
            let emit = |phase, engine, chunk, chunks, error: Option<&str>| {
                app.emit(
                    PROGRESS_EVENT,
                    Progress {
                        path: &path,
                        phase,
                        engine,
                        chunk,
                        chunks,
                        error,
                    },
                )
                .ok();
            };
            let seen = Cell::new((0, 0));

            let result = crate::echo360::find_ffmpeg(resource_dir.clone())
                .ok_or_else(|| "ffmpeg not found — run `bun run ffmpeg` in app/".to_string())
                .and_then(|ffmpeg| {
                    run(
                        &crate::paths::data_dir(),
                        &path,
                        &ffmpeg,
                        resource_dir,
                        None,
                        |s| match s {
                            Step::Extracting => emit("extracting", None, 0, 0, None),
                            Step::Transcribing {
                                engine,
                                chunk,
                                chunks,
                            } => {
                                seen.set((chunk, chunks));
                                emit("transcribing", Some(engine), chunk, chunks, None);
                            }
                        },
                    )
                });

            let (chunk, chunks) = seen.get();
            match &result {
                Ok(done) => {
                    eprintln!(
                        "[oculus] transcribed {path} with {}: {} cue(s)",
                        done.engine, done.cues
                    );
                    emit(
                        "complete",
                        Some(done.engine),
                        done.chunks,
                        done.chunks,
                        None,
                    );
                }
                Err(e) => {
                    eprintln!("[oculus] transcription failed for {path}: {e}");
                    emit("error", None, chunk, chunks, Some(e));
                }
            }
            result.map(|done| done.path.to_string_lossy().into_owned())
        })
        .await
    }

    /// Whether this Mac can transcribe on device, and in which languages —
    /// `apple-speech locales`, or `available: false` with the reason. Free and
    /// quick: it lists what the OS has and never downloads a model.
    #[tauri::command]
    pub async fn apple_speech_status(app: AppHandle) -> Result<apple::Status, String> {
        let resource_dir = app.path().resource_dir().ok();
        crate::blocking::run(move || Ok(apple::status(resource_dir))).await
    }

    /// Display only, never persisted.
    pub const WHISPER_MODEL_PROGRESS_EVENT: &str = "whisper-model-progress";

    #[derive(serde::Serialize, Clone)]
    #[serde(rename_all = "camelCase")]
    struct ModelProgress<'a> {
        id: &'a str,
        /// `downloading`, `complete`, `cancelled` or `error`.
        phase: &'static str,
        /// Bytes of the model file; the VAD model's are not counted.
        received: u64,
        total: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<&'a str>,
    }

    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct WhisperListing {
        #[serde(flatten)]
        catalogue: whisper_models::Catalogue,
        /// Whether `whisper-cli` is found; without it no model runs.
        helper: bool,
    }

    /// The Whisper model catalogue with what is downloaded and how each suits
    /// this machine, and whether the helper is there. Reads the models
    /// directory, the RAM size and the helper's path only.
    #[tauri::command]
    pub async fn whisper_models(app: AppHandle) -> Result<WhisperListing, String> {
        let resource_dir = app.path().resource_dir().ok();
        crate::blocking::run(move || {
            Ok(WhisperListing {
                catalogue: whisper_models::list(&whisper_models::dir()),
                helper: whisper::helper(resource_dir).is_some(),
            })
        })
        .await
    }

    /// Download one model (and the VAD model with the first), resolving when it
    /// is on disk. A cancel rejects with `cancelled`.
    #[tauri::command]
    pub async fn whisper_download_model(app: AppHandle, id: String) -> Result<(), String> {
        let model =
            whisper_models::find(&id).ok_or_else(|| format!("no Whisper model called {id}"))?;
        crate::blocking::run(move || {
            let emit = |phase, received, total, error: Option<&str>| {
                app.emit(
                    WHISPER_MODEL_PROGRESS_EVENT,
                    ModelProgress {
                        id: &id,
                        phase,
                        received,
                        total,
                        error,
                    },
                )
                .ok();
            };
            let seen = Cell::new((0, model.bytes));
            let result =
                whisper_models::download(&whisper_models::dir(), model, |received, total| {
                    seen.set((received, total));
                    emit("downloading", received, total, None);
                });
            let (received, total) = seen.get();
            match &result {
                Ok(path) => {
                    eprintln!(
                        "[oculus] downloaded Whisper model {id} to {}",
                        path.display()
                    );
                    emit("complete", received, total, None);
                }
                Err(e) if e == whisper_models::CANCELLED => {
                    emit("cancelled", received, total, None)
                }
                Err(e) => {
                    eprintln!("[oculus] Whisper model {id} failed to download: {e}");
                    emit("error", received, total, Some(e));
                }
            }
            result.map(|_| ())
        })
        .await
    }

    /// Ask an in-flight model download to stop; false when none runs for `id`.
    #[tauri::command]
    pub fn whisper_cancel_download(id: String) -> bool {
        whisper_models::cancel(&id)
    }

    /// Delete a downloaded model, stopping its download if one runs; returns
    /// the bytes freed.
    #[tauri::command]
    pub async fn whisper_delete_model(id: String) -> Result<u64, String> {
        crate::blocking::run(move || whisper_models::delete(&whisper_models::dir(), &id)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch as Dir;

    #[test]
    fn a_second_claim_on_one_video_is_refused_until_the_first_drops() {
        let (a, b) = (
            Path::new("/claim-test/a.mp4"),
            Path::new("/claim-test/b.mp4"),
        );
        let first = Claim::take(a).unwrap();
        assert!(Claim::take(a).is_err());
        assert!(Claim::take(b).is_ok());
        drop(first);
        assert!(Claim::take(a).is_ok());
    }

    #[test]
    fn the_scratch_directory_is_removed_on_drop() {
        let dir = {
            let scratch = Scratch::new().unwrap();
            std::fs::write(scratch.file("audio.ogg"), b"x").unwrap();
            scratch.0.clone()
        };
        assert!(!dir.exists());
    }

    #[test]
    fn the_vtt_sits_beside_the_video_with_its_full_name() {
        assert_eq!(
            vtt_path(Path::new("/lib/courses/MAST/files/Matrices Part 1.mp4")),
            Path::new("/lib/courses/MAST/files/Matrices Part 1.mp4.vtt")
        );
    }

    #[test]
    fn only_files_inside_the_library_resolve() {
        let library = Dir::new("transcribe-library");
        let outside = Dir::new("transcribe-outside");
        let files = library.join("courses").join("MAST").join("files");
        std::fs::create_dir_all(&files).unwrap();
        std::fs::write(files.join("Week 1.mp4"), b"x").unwrap();
        std::fs::write(outside.join("elsewhere.mp4"), b"x").unwrap();
        let inside = files.join("Week 1.mp4").canonicalize().unwrap();

        assert_eq!(
            resolve(&library, "courses/MAST/files/Week 1.mp4").unwrap(),
            inside
        );
        assert_eq!(resolve(&library, inside.to_str().unwrap()).unwrap(), inside);

        let escape = format!(
            "courses/../../{}/elsewhere.mp4",
            outside.file_name().unwrap().to_string_lossy()
        );
        assert!(resolve(&library, &escape)
            .unwrap_err()
            .contains("outside the library"));
        let absolute = outside.join("elsewhere.mp4");
        assert!(resolve(&library, absolute.to_str().unwrap())
            .unwrap_err()
            .contains("outside the library"));

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.join("elsewhere.mp4"), files.join("link.mp4"))
                .unwrap();
            assert!(resolve(&library, "courses/MAST/files/link.mp4")
                .unwrap_err()
                .contains("outside"));
        }

        assert!(resolve(&library, "courses/MAST/files/missing.mp4")
            .unwrap_err()
            .contains("not on disk"));
        assert!(resolve(&library, "courses/MAST/files")
            .unwrap_err()
            .contains("not a file"));
    }

    /// Answers each call from a script, in order, and records the paths.
    struct Scripted {
        name: &'static str,
        cap: Option<u64>,
        replies: Mutex<Vec<Result<Vec<Segment>, EngineError>>>,
        heard: Mutex<Vec<PathBuf>>,
    }

    impl Scripted {
        fn new(
            name: &'static str,
            cap: Option<u64>,
            replies: Vec<Result<Vec<Segment>, EngineError>>,
        ) -> Box<dyn Engine> {
            Box::new(Self {
                name,
                cap,
                replies: Mutex::new(replies),
                heard: Mutex::new(vec![]),
            })
        }
    }

    impl Engine for Scripted {
        fn name(&self) -> &'static str {
            self.name
        }
        fn max_upload_bytes(&self) -> Option<u64> {
            self.cap
        }
        fn transcribe(&self, audio: &Path) -> Result<Vec<Segment>, EngineError> {
            self.heard.lock().unwrap().push(audio.to_path_buf());
            self.replies.lock().unwrap().remove(0)
        }
    }

    fn said(text: &str) -> Result<Vec<Segment>, EngineError> {
        Ok(vec![Segment {
            start: 1.0,
            end: 2.0,
            text: text.into(),
        }])
    }

    /// 30 MB over 90 s: three parts under a 20 MB-ish cap, one without.
    const AUDIO: audio::Extracted = audio::Extracted {
        bytes: 30_000_000,
        duration: Some(90.0),
    };

    fn hear(engines: &[Box<dyn Engine>]) -> (Result<Heard, String>, Vec<String>) {
        let steps = Mutex::new(Vec::new());
        let result = transcribe_audio(
            engines,
            &AUDIO,
            Path::new("/scratch/audio.ogg"),
            |engine, index, _| Ok(PathBuf::from(format!("/scratch/part-{engine}-{index}.ogg"))),
            &|step| {
                if let Step::Transcribing {
                    engine,
                    chunk,
                    chunks,
                } = step
                {
                    steps
                        .lock()
                        .unwrap()
                        .push(format!("{engine} {chunk}/{chunks}"));
                }
            },
        );
        (result, steps.into_inner().unwrap())
    }

    #[test]
    fn a_rate_limit_mid_run_hands_the_whole_audio_to_the_next_engine() {
        let engines = [
            Scripted::new(
                "groq",
                Some(12_000_000),
                vec![
                    said("from groq"),
                    Err(EngineError::RateLimited("Groq's limit".into())),
                ],
            ),
            Scripted::new("apple", None, vec![said("on device")]),
        ];
        let (result, steps) = hear(&engines);
        let heard = result.unwrap();
        assert_eq!(heard.engine, "apple");
        assert_eq!(heard.chunks, 1);
        // Groq's first part is dropped, and the whole file is reused unsplit.
        assert_eq!(
            heard.segments,
            vec![Segment {
                start: 1.0,
                end: 2.0,
                text: "on device".into()
            }]
        );
        assert_eq!(steps, vec!["groq 1/3", "groq 2/3", "apple 1/1"]);
    }

    #[test]
    fn a_failure_surfaces_and_never_falls_through() {
        let engines = [
            Scripted::new(
                "groq",
                None,
                vec![Err(EngineError::Failed(
                    "Groq rejected the saved key".into(),
                ))],
            ),
            Scripted::new("apple", None, vec![said("unreached")]),
        ];
        let (result, steps) = hear(&engines);
        assert_eq!(result.err().unwrap(), "Groq rejected the saved key");
        assert_eq!(steps, vec!["groq 1/1"]);
    }

    #[test]
    fn when_every_engine_declines_each_reason_is_reported() {
        let engines = [
            Scripted::new(
                "groq",
                None,
                vec![Err(EngineError::RateLimited(
                    "Groq: try again in 5 minutes".into(),
                ))],
            ),
            Scripted::new(
                "apple",
                None,
                vec![Err(EngineError::NotConfigured("needs macOS 26".into()))],
            ),
        ];
        let message = hear(&engines).0.err().unwrap();
        assert!(
            message.contains("try again in 5 minutes") && message.contains("needs macOS 26"),
            "{message}"
        );

        let alone = [Scripted::new(
            "apple",
            None,
            vec![Err(EngineError::NotConfigured("needs macOS 26".into()))],
        )];
        assert_eq!(hear(&alone).0.err().unwrap(), "needs macOS 26");
    }

    #[test]
    fn an_engine_that_answers_first_is_the_only_one_called() {
        let engines = [
            Scripted::new(
                "groq",
                Some(12_000_000),
                vec![said("a"), said("b"), said("c")],
            ),
            Scripted::new("apple", None, vec![]),
        ];
        let heard = hear(&engines).0.unwrap();
        assert_eq!((heard.engine, heard.chunks), ("groq", 3));
        // Each part is on the video's clock: 30 s apart.
        let starts: Vec<f64> = heard.segments.iter().map(|s| s.start).collect();
        assert_eq!(starts, vec![1.0, 31.0, 61.0]);
    }

    fn settings(row: &str) -> Settings {
        settings_from(Some(row))
    }

    #[test]
    fn the_order_defaults_to_groq_whisper_apple_and_keeps_each_engine_once() {
        let default = vec!["groq", "whisper", "apple"];
        assert_eq!(settings_from(None).order, default);
        assert_eq!(settings("not json").order, default);
        assert_eq!(settings(r#"{"order":"apple"}"#).order, default);
        assert_eq!(settings(r#"{"order":[]}"#).order, default);
        assert_eq!(
            settings(r#"{"order":["apple","groq","whisper"]}"#).order,
            vec!["apple", "groq", "whisper"]
        );
        // Unknown names and non-strings are dropped, the missing appended in the default order.
        assert_eq!(
            settings(r#"{"order":["vosk",3,"apple",null]}"#).order,
            vec!["apple", "groq", "whisper"]
        );
        assert_eq!(
            settings(r#"{"order":["whisper","whisper","apple","whisper"]}"#).order,
            vec!["whisper", "apple", "groq"]
        );
    }

    #[test]
    fn every_engine_is_on_unless_switched_off_and_mistyped_values_read_as_on() {
        let all = settings_from(None);
        assert!(all.groq && all.whisper && all.apple);
        assert_eq!(all.whisper_model, None);
        let row = settings(
            r#"{"groq":{"enabled":false},"whisper":{"enabled":"no","model":" small "},"apple":[1],"x":1}"#,
        );
        assert!(!row.groq && row.whisper && row.apple);
        assert_eq!(row.whisper_model.as_deref(), Some("small"));
        assert_eq!(
            settings(r#"{"whisper":{"model":"  "},"apple":{"enabled":false}}"#).whisper_model,
            None
        );
        assert!(!settings(r#"{"apple":{"enabled":false}}"#).apple);
    }

    #[test]
    fn the_language_falls_back_to_the_older_per_engine_keys() {
        let language = |row: &str| settings(row).language;
        assert_eq!(settings_from(None).language, Language(None));
        assert_eq!(
            language(r#"{"language":"th_TH","apple":{"locale":"en_AU"}}"#),
            Language(Some("th_TH".into()))
        );
        assert_eq!(
            language(r#"{"apple":{"locale":"en_AU"},"whisper":{"language":"fr"}}"#),
            Language(Some("en_AU".into()))
        );
        assert_eq!(
            language(r#"{"apple":{"locale":" "},"whisper":{"language":"fr"}}"#),
            Language(Some("fr".into()))
        );
        assert_eq!(
            language(r#"{"whisper":{"language":"AUTO"}}"#),
            Language(Some("auto".into()))
        );
        assert_eq!(
            language(r#"{"language":7,"whisper":{"language":"de"}}"#),
            Language(Some("de".into()))
        );
    }

    #[test]
    fn each_engine_reads_the_language_its_own_way() {
        let of = |l: Option<&str>| Language(l.map(str::to_string));
        // The default: English, in the Mac's region for on-device speech.
        assert_eq!(
            (of(None).whisper(), of(None).groq(), of(None).apple()),
            ("en".into(), Some("en".into()), None)
        );
        let australian = of(Some("en_AU"));
        assert_eq!(
            (australian.whisper(), australian.groq(), australian.apple()),
            ("en".into(), Some("en".into()), Some("en_AU".into()))
        );
        let thai = of(Some("th_TH"));
        assert_eq!(
            (thai.whisper(), thai.groq(), thai.apple()),
            ("th".into(), Some("th".into()), Some("th_TH".into()))
        );
        assert_eq!(of(Some("zh-Hant-TW")).whisper(), "zh");
        // Apple has no auto-detect: it keeps its default.
        let auto = of(Some("auto"));
        assert_eq!(
            (auto.whisper(), auto.groq(), auto.apple()),
            ("auto".into(), None, None)
        );
        // The old Whisper default, a bare `en`, is the Mac's own English.
        assert_eq!(of(Some("en")).apple(), None);
        assert_eq!(of(Some("fr")).apple(), Some("fr".into()));
    }

    #[test]
    fn groq_switched_off_is_unconfigured_without_reading_the_key() {
        // keyd is absent here: nothing listens in this scratch dir.
        let dir = crate::test_support::Scratch::new("groq-engine");
        let off = settings(r#"{"groq":{"enabled":false}}"#);
        let refused = groq_engine(&off, &dir, || panic!("the key was read"))
            .err()
            .unwrap();
        assert_eq!(refused, "Groq is turned off in Settings → Transcription");

        let on = settings(r#"{"language":"auto"}"#);
        assert!(groq_engine(&on, &dir, || Ok(None))
            .err()
            .unwrap()
            .contains("Settings → Transcription"));
        let refused = groq_engine(&on, &dir, || Err("denied".into()))
            .err()
            .unwrap();
        assert!(
            refused.contains("keychain") && refused.contains("denied"),
            "{refused}"
        );
        assert_eq!(
            groq_engine(&on, &dir, || Ok(Some("gsk_test".into())))
                .ok()
                .unwrap()
                .name(),
            "groq"
        );
    }

    #[test]
    fn with_keyd_installed_groq_asks_keyd_and_never_the_keychain() {
        use crate::test_support::{FakeKeyd, Scratch};
        use serde_json::json;
        let on = settings(r#"{"language":"auto"}"#);
        let answers = [
            (json!({"has": true}), None),
            (json!({"has": false}), Some("No Groq API key")),
            (
                json!({"error": "keychain", "detail": "OSStatus -128"}),
                Some("The keychain refused to give out the Groq API key (OSStatus -128)"),
            ),
            (
                json!({"error": "caller", "detail": "outside the bundle"}),
                Some("oculus-keyd, which holds the Groq API key"),
            ),
        ];
        for (reply, refused) in answers {
            let dir = Scratch::new("groq-engine-keyd");
            let keyd = FakeKeyd::start(&dir, move |_, _| (reply.clone(), vec![]));
            let engine = groq_engine(&on, &dir, || panic!("the keychain was read"));
            match refused {
                None => assert_eq!(engine.ok().unwrap().name(), "groq"),
                Some(start) => {
                    let message = engine.err().unwrap();
                    assert!(message.starts_with(start), "{message}");
                }
            }
            assert_eq!(keyd.ops(), ["has"]);
            assert_eq!(keyd.requests()[0].0["secret"], "groq");
        }
    }

    #[test]
    fn an_unknown_forced_engine_is_refused_before_anything_is_read() {
        let refused = engines(None, Path::new("/no/ffmpeg"), Some("vosk"))
            .err()
            .unwrap();
        assert_eq!(
            refused,
            "no engine called vosk — it is groq, whisper, apple"
        );
    }

    #[test]
    fn chunk_replies_become_one_vtt_on_the_videos_clock() {
        // Two chunks of a long recording, the second starting past the hour.
        let first = groq::parse_segments(
            r#"{"segments":[{"start":0.0,"end":3.25,"text":" Welcome back."}]}"#,
        )
        .unwrap();
        let second = groq::parse_segments(
            r#"{"segments":[
                {"start":0.5,"end":4.0,"text":" Now the proof."},
                {"start":4.0,"end":5.0,"text":" "}
            ]}"#,
        )
        .unwrap();
        let segments: Vec<Segment> = shift(first, 0.0).chain(shift(second, 3725.0)).collect();
        let (body, cues) = vtt::render(&segments);
        assert_eq!(cues, 2);
        assert_eq!(
            body,
            "WEBVTT\n\
             \n00:00:00.000 --> 00:00:03.250\nWelcome back.\n\
             \n01:02:05.500 --> 01:02:09.000\nNow the proof.\n"
        );
    }
}
