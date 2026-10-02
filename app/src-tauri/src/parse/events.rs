//! `parse-status`: queued → running → quality/error. The shared wire payload
//! lives in `pipeline_events`; parse errors keep their own discriminants.

use tauri::AppHandle;

use super::{ParseError, Progress};
use crate::pipeline_events::{Channel, Status};

static CHANNEL: Channel = Channel::new("parse-status");

/// Bind once in app setup; the CLI leaves this channel unbound.
pub fn bind(app: AppHandle) {
    CHANNEL.bind(app);
}

/// Accepted, not started. The queue is unordered, so no position is sent.
pub fn queued(relative_path: &str, subject_id: i64) {
    CHANNEL.emit(Status::new(relative_path, subject_id, "queued"));
}

pub fn running(relative_path: &str, subject_id: i64, progress: Progress) {
    CHANNEL.emit(Status::new(relative_path, subject_id, "running")
        .progress(progress.pages_done, progress.total_pages));
}

/// `quality` matches the terminal `files.parse_status` stored by the app.
pub fn parsed(relative_path: &str, subject_id: i64) {
    CHANNEL.emit(Status::new(relative_path, subject_id, "quality"));
}

pub fn failed(relative_path: &str, subject_id: i64, error: &ParseError) {
    CHANNEL.emit(Status::new(relative_path, subject_id, "error").failure(
        error.to_string(), Some(error.kind()), Some(error.retryable()), Some(error.latching()),
    ));
}
