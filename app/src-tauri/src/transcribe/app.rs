//! The Tauri commands behind Settings → Transcription and the video viewer.

use std::cell::Cell;
use tauri::{AppHandle, Emitter, Manager};

use super::engines::{apple, whisper, whisper_models};
use super::pipeline::{run, Step};

/// Display only, never persisted.
pub const PROGRESS_EVENT: &str = "transcribe-progress";

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress<'a> {
    /// The path exactly as the caller passed it.
    path: &'a str,
    /// `extracting`, `transcribing`, `complete` or `error`.
    phase: &'static str,
    /// One of [`super::ENGINES`], while transcribing and once complete.
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
    crate::runtime::blocking::run(move || {
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

        let result = crate::sources::echo360::find_ffmpeg(resource_dir.clone())
            .ok_or_else(|| "ffmpeg not found — run `bun run ffmpeg` in app/".to_string())
            .and_then(|ffmpeg| {
                run(
                    &crate::library::paths::data_dir(),
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
    crate::runtime::blocking::run(move || Ok(apple::status(resource_dir))).await
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
    crate::runtime::blocking::run(move || {
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
    let model = whisper_models::find(&id).ok_or_else(|| format!("no Whisper model called {id}"))?;
    crate::runtime::blocking::run(move || {
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
        let result = whisper_models::download(&whisper_models::dir(), model, |received, total| {
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
            Err(e) if e == whisper_models::CANCELLED => emit("cancelled", received, total, None),
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
    crate::runtime::blocking::run(move || whisper_models::delete(&whisper_models::dir(), &id)).await
}
