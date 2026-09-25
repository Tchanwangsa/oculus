//! `parse-status` — the one event the parse path emits.
//!
//! The handle is set once at startup rather than passed down, so no Tauri type
//! sits in code the CLI runs. **A headless run leaves it unbound and every
//! emit is a no-op.**

use std::sync::OnceLock;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::{ParseError, Progress};

/// Set once, in `lib.rs`'s setup. Never set in the CLI.
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Give the parse path somewhere to emit. Idempotent; a second call is ignored.
pub fn bind(app: AppHandle) {
    let _ = APP.set(app);
}

/// The payload: a **fixed contract** with `app/src/stores/parseStore.ts` and
/// `app/src/hooks/useBackendEvents.ts`. Optional fields are omitted, not null.
///
/// `status` is `queued | running | quality | error`. **`"quality"` is the
/// terminal success**: it matches `files.parse_status` on every parsed row.
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
    pub position: Option<u32>,
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
            position: None,
            error: None,
            kind: None,
            retryable: None,
            latching: None,
        }
    }

    pub fn emit(&self) {
        if let Some(app) = APP.get() {
            app.emit("parse-status", self).ok();
        }
    }
}

/// Accepted, not started. The queue is unordered, so no `position`.
pub fn queued(relative_path: &str, subject_id: i64) {
    Status::new(relative_path, subject_id, "queued").emit();
}

/// A page landed. A zero `total_pages` is dropped: the UI would draw a zero
/// denominator as a finished bar.
pub fn running(relative_path: &str, subject_id: i64, progress: Progress) {
    let mut status = Status::new(relative_path, subject_id, "running");
    status.pages_done = Some(progress.pages_done);
    status.total_pages = (progress.total_pages > 0).then_some(progress.total_pages);
    status.emit();
}

/// Terminal success.
pub fn parsed(relative_path: &str, subject_id: i64) {
    Status::new(relative_path, subject_id, "quality").emit();
}

/// Terminal failure. `error` is `ParseError`'s `Display`, which never carries
/// server text (see `ParseError`).
pub fn failed(relative_path: &str, subject_id: i64, error: &ParseError) {
    let mut status = Status::new(relative_path, subject_id, "error");
    status.error = Some(error.to_string());
    status.kind = Some(error.kind());
    status.retryable = Some(error.retryable());
    status.latching = Some(error.latching());
    status.emit();
}
