//! `parse-status`: queued → running → quality/error/skipped. The shared wire
//! payload lives in `pipeline_events`; parse errors keep their own
//! discriminants.

use tauri::AppHandle;

use super::{ParseError, Phase, Progress};
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
    CHANNEL.emit(running_status(relative_path, subject_id, progress));
}

/// `quality` matches the terminal `files.parse_status` stored by the app.
pub fn parsed(relative_path: &str, subject_id: i64) {
    CHANNEL.emit(Status::new(relative_path, subject_id, "quality"));
}

/// The user skipped this file; `skipped` is also its stored `parse_status`.
pub fn skipped(relative_path: &str, subject_id: i64) {
    CHANNEL.emit(Status::new(relative_path, subject_id, "skipped"));
}

pub fn failed(relative_path: &str, subject_id: i64, error: &ParseError) {
    // The UI keeps only the status, so the sentence also goes to stderr.
    if matches!(error, ParseError::Cancelled) {
        eprintln!("[oculus] parse skipped: {relative_path}");
    } else {
        eprintln!("[oculus] parse failed: {relative_path}: {error}");
    }
    CHANNEL.emit(failure_status(relative_path, subject_id, error));
}

/// Bytes ride only on the upload phases: `upload_wait` knows the total.
fn running_status(relative_path: &str, subject_id: i64, progress: Progress) -> Status {
    let (done, total) = match progress.phase {
        Phase::UploadWait => (None, Some(progress.bytes_total)),
        Phase::Uploading => (Some(progress.bytes_done), Some(progress.bytes_total)),
        Phase::Processing => (None, None),
    };
    Status::new(relative_path, subject_id, "running")
        .progress(progress.pages_done, progress.total_pages)
        .phase(progress.phase.as_str(), done, total)
}

/// A skip ends the parse but is not a failure, so it carries no error fields.
fn failure_status(relative_path: &str, subject_id: i64, error: &ParseError) -> Status {
    if matches!(error, ParseError::Cancelled) {
        return Status::new(relative_path, subject_id, "skipped");
    }
    Status::new(relative_path, subject_id, "error").failure(
        error.to_string(),
        Some(error.kind()),
        Some(error.retryable()),
        Some(error.latching()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    #[test]
    fn a_cancelled_parse_is_reported_as_skipped_not_as_an_error() {
        let skipped = to_value(failure_status("a.pdf", 4, &ParseError::Cancelled)).unwrap();
        assert_eq!(
            skipped,
            json!({ "relative_path": "a.pdf", "subject_id": 4, "status": "skipped" })
        );
        let failed = to_value(failure_status("a.pdf", 4, &ParseError::Io("x".into()))).unwrap();
        assert_eq!(failed["status"], "error");
        assert_eq!(failed["kind"], "io");
    }

    #[test]
    fn each_phase_sends_only_its_own_byte_counts() {
        let at = |phase| Progress {
            pages_done: 0,
            total_pages: 9,
            backend: "test",
            phase,
            bytes_done: 10,
            bytes_total: 40,
        };
        let wait = to_value(running_status("a.pdf", 4, at(Phase::UploadWait))).unwrap();
        assert_eq!(wait["phase"], "upload_wait");
        assert!(wait.get("bytes_done").is_none());
        assert_eq!(wait["bytes_total"], 40);
        let up = to_value(running_status("a.pdf", 4, at(Phase::Uploading))).unwrap();
        assert_eq!(
            (up["bytes_done"].clone(), up["bytes_total"].clone()),
            (json!(10), json!(40))
        );
        let busy = to_value(running_status(
            "a.pdf",
            4,
            Progress::processing(3, 9, "test"),
        ))
        .unwrap();
        assert_eq!(busy["phase"], "processing");
        assert_eq!(busy["pages_done"], 3);
        assert!(busy.get("bytes_total").is_none());
    }
}
