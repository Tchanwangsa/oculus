//! A model download that reports progress as Tauri events, shared by the
//! aligner (`studio-aligner-progress`) and Kokoro's packs
//! (`studio-voice-progress`), like the Whisper models' (`transcribe::app`).

use std::cell::Cell;
use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::runtime::onnx::CANCELLED;

/// One event's payload. Display only, never persisted.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress<'a> {
    /// `downloading`, `complete`, `cancelled` or `error`.
    phase: &'static str,
    received: u64,
    total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
}

/// Run `download`, emitting `event` for each `(received, total)` it reports
/// and once more when it ends; `what` names the model and `dir` where it
/// goes, for the log. Resolves with `download`'s own result,
/// `Err(CANCELLED)` on a cancel.
pub(super) fn download_with_events(
    app: &AppHandle,
    event: &str,
    what: &str,
    dir: &Path,
    total: u64,
    download: impl FnOnce(&dyn Fn(u64, u64)) -> Result<(), String>,
) -> Result<(), String> {
    let emit = |phase, received, total, error: Option<&str>| {
        app.emit(
            event,
            Progress {
                phase,
                received,
                total,
                error,
            },
        )
        .ok();
    };
    let seen = Cell::new((0, total));
    let result = download(&|received, total| {
        seen.set((received, total));
        emit("downloading", received, total, None);
    });
    let (received, total) = seen.get();
    match &result {
        Ok(()) => {
            elog!("[oculus] downloaded {what} to {}", dir.display());
            emit("complete", received, total, None);
        }
        Err(e) if e == CANCELLED => emit("cancelled", received, total, None),
        Err(e) => {
            elog!("[oculus] {what} failed to download: {e}");
            emit("error", received, total, Some(e));
        }
    }
    result
}
