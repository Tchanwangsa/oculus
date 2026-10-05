//! `embed-status`: queued → running → done/error. Uses the same file-pipeline
//! wire payload as parsing, while keeping embed errors and success distinct.

use tauri::AppHandle;

use super::{EmbedError, Progress};
use crate::pipeline_events::{Channel, Status};

static CHANNEL: Channel = Channel::new("embed-status");

/// Bind once in app setup; the CLI leaves this channel unbound.
pub fn bind(app: AppHandle) {
    CHANNEL.bind(app);
}

pub fn queued(relative_path: &str, subject_id: i64) {
    CHANNEL.emit(Status::new(relative_path, subject_id, "queued"));
}

pub fn running(relative_path: &str, subject_id: i64, progress: Progress) {
    CHANNEL.emit(Status::new(relative_path, subject_id, "running")
        .progress(progress.pages_done, progress.total_pages));
}

/// `pages` is the exact number that landed in the table.
pub fn embedded(relative_path: &str, subject_id: i64, pages: u32) {
    CHANNEL.emit(Status::new(relative_path, subject_id, "done").completed(pages));
}

pub fn failed(relative_path: &str, subject_id: i64, error: &EmbedError) {
    failed_with(relative_path, subject_id, error.to_string(), Some(error.kind()),
        Some(error.retryable()), Some(error.latching()));
}

/// A failure before reaching a backend keeps its unknown discriminants absent.
pub fn failed_with(
    relative_path: &str,
    subject_id: i64,
    message: String,
    kind: Option<&'static str>,
    retryable: Option<bool>,
    latching: Option<bool>,
) {
    // The UI keeps only the error status, so the sentence also goes to stderr.
    eprintln!("[oculus] embed failed: {relative_path}: {message}");
    CHANNEL.emit(Status::new(relative_path, subject_id, "error")
        .failure(message, kind, retryable, latching));
}
