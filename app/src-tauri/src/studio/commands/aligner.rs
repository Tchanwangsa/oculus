//! The narration aligner's model in Settings -> Studio -> Word timing: its status, and a
//! download that reports progress as `studio-aligner-progress` events
//! (`progress`).

use serde::Serialize;
use tauri::AppHandle;

use super::progress::download_with_events;
use crate::studio::timing::{self, Status, PACK};

/// Display only, never persisted.
pub const PROGRESS_EVENT: &str = "studio-aligner-progress";

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
        let dir = timing::dir();
        download_with_events(
            &app,
            PROGRESS_EVENT,
            "the aligner model",
            &dir,
            PACK.bytes(),
            |progress| timing::download(&dir, progress, || false),
        )
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
