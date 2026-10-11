//! The narration aligner's model in Settings -> Voice: its status, and a
//! download that reports progress as `studio-aligner-progress` events, like
//! the Whisper models' (`transcribe::app`).

use std::cell::Cell;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::studio::timing::{self, Status, CANCELLED, PACK};

/// Display only, never persisted.
pub const PROGRESS_EVENT: &str = "studio-aligner-progress";

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

#[derive(Serialize)]
pub struct AlignerStatus {
    pub status: Status,
    /// The download's size.
    pub bytes: u64,
}

/// Whether the model is downloaded or downloading. Local only.
#[tauri::command]
pub async fn studio_aligner_status() -> Result<AlignerStatus, String> {
    crate::runtime::blocking::run(|| {
        Ok(AlignerStatus {
            status: timing::status(&timing::dir()),
            bytes: PACK.bytes(),
        })
    })
    .await
}

/// Download the model, resolving when it is on disk. A cancel rejects with
/// `cancelled`.
#[tauri::command]
pub async fn studio_aligner_download(app: AppHandle) -> Result<(), String> {
    crate::runtime::blocking::run(move || {
        let emit = |phase, received, total, error: Option<&str>| {
            app.emit(
                PROGRESS_EVENT,
                Progress {
                    phase,
                    received,
                    total,
                    error,
                },
            )
            .ok();
        };
        let seen = Cell::new((0, PACK.bytes()));
        let result = timing::download(
            &timing::dir(),
            |received, total| {
                seen.set((received, total));
                emit("downloading", received, total, None);
            },
            || false,
        );
        let (received, total) = seen.get();
        match &result {
            Ok(()) => {
                elog!(
                    "[oculus] downloaded the aligner model to {}",
                    timing::dir().display()
                );
                emit("complete", received, total, None);
            }
            Err(e) if e == CANCELLED => emit("cancelled", received, total, None),
            Err(e) => {
                elog!("[oculus] the aligner model failed to download: {e}");
                emit("error", received, total, Some(e));
            }
        }
        result
    })
    .await
}

/// Ask an in-flight download to stop; false when none runs.
#[tauri::command]
pub fn studio_aligner_cancel() -> bool {
    timing::cancel()
}

/// Delete the model and any partial download, stopping a download if one
/// runs; returns the bytes freed.
#[tauri::command]
pub async fn studio_aligner_delete() -> Result<u64, String> {
    crate::runtime::blocking::run(|| timing::delete(&timing::dir())).await
}
