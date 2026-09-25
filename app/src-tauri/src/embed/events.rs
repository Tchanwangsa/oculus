//! `embed-status` — the one event the embedding path emits, with page-level
//! progress. The same shape as `parse/events.rs` so a pipeline row reads both
//! stages alike. The `AppHandle` is bound once at startup so no Tauri type runs
//! through code the CLI shares; headless, every emit is a no-op.

use std::sync::OnceLock;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::{EmbedError, Progress};

/// Set once, in `lib.rs`'s setup. Never set in the CLI.
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Give the embed path somewhere to emit. Idempotent; a second call is ignored.
pub fn bind(app: AppHandle) {
    let _ = APP.set(app);
}

/// The payload — a fixed contract read by `app/src/hooks/useBackendEvents.ts`
/// and `app/src/stores/pipelineStore.ts`. Optional fields are omitted, not
/// null. `status` is `queued | running | done | error` (not the parse path's
/// `quality`).
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub relative_path: String,
    pub subject_id: i64,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages_done: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_pages: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latching: Option<bool>,
}

impl Status {
    fn new(relative_path: &str, subject_id: i64, status: &'static str) -> Self {
        Self {
            relative_path: relative_path.to_string(),
            subject_id,
            status,
            pages_done: None,
            total_pages: None,
            error: None,
            kind: None,
            retryable: None,
            latching: None,
        }
    }

    pub fn emit(&self) {
        if let Some(app) = APP.get() {
            app.emit("embed-status", self).ok();
        }
    }
}

/// Accepted, not started — emitted by the command as it takes the file.
pub fn queued(relative_path: &str, subject_id: i64) {
    Status::new(relative_path, subject_id, "queued").emit();
}

/// Pages finished. A zero `total_pages` is omitted, not sent as a denominator.
pub fn running(relative_path: &str, subject_id: i64, progress: Progress) {
    let mut status = Status::new(relative_path, subject_id, "running");
    status.pages_done = Some(progress.pages_done);
    status.total_pages = (progress.total_pages > 0).then_some(progress.total_pages);
    status.emit();
}

/// Terminal success. `pages` is what landed in the table.
pub fn embedded(relative_path: &str, subject_id: i64, pages: u32) {
    let mut status = Status::new(relative_path, subject_id, "done");
    status.pages_done = Some(pages);
    status.total_pages = Some(pages);
    status.emit();
}

/// Terminal failure: `EmbedError`'s `Display` (never server text) plus the
/// three discriminants the failure UI branches on.
pub fn failed(relative_path: &str, subject_id: i64, error: &EmbedError) {
    failed_with(
        relative_path,
        subject_id,
        error.to_string(),
        Some(error.kind()),
        Some(error.retryable()),
        Some(error.latching()),
    );
}

/// The same, for a failure that never reached a backend or is already
/// unpacked. `None` travels as an absent key, which the UI treats as unknown.
pub fn failed_with(
    relative_path: &str,
    subject_id: i64,
    message: String,
    kind: Option<&'static str>,
    retryable: Option<bool>,
    latching: Option<bool>,
) {
    let mut status = Status::new(relative_path, subject_id, "error");
    status.error = Some(message);
    status.kind = kind;
    status.retryable = retryable;
    status.latching = latching;
    status.emit();
}
