//! `parse-status` — the one event the parse path emits.
//!
//! This replaces `ipc.rs`, a loopback HTTP server that existed for exactly one
//! reason: the Python sidecar was another process, so its progress had to come
//! back over a socket, and the port it posted to had to be threaded through
//! every call site that might eventually cause a parse. Parsing is in-process
//! now, so the progress is already here and the only thing missing is somewhere
//! to send it.
//!
//! That somewhere is a handle set once at startup rather than a parameter.
//! The alternative — passing an `AppHandle` down through `Engine`, `parse_pdf`
//! and the batcher — puts a Tauri type in the middle of code the CLI runs, and
//! the CLI has no handle at all. **A headless run leaves this unbound and every
//! emit is a no-op**, which is the honest shape of the thing: the events exist
//! for a window that may not be there.

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

/// The payload, and it is a **fixed contract** — `app/src/stores/parseStore.ts`
/// and `app/src/hooks/useBackendEvents.ts` are already shipping against exactly
/// these field names. Optional fields are omitted rather than sent as null, so
/// a reader that sees a key can trust it.
///
/// `status` is `queued | running | quality | error`. **`"quality"` is the
/// terminal success** and the name outlived the tier it was named after: there
/// is one parse now, but `files.parse_status = 'quality'` is what every
/// already-parsed row in the user's library says and what the "already done"
/// check reads. Renaming it would invalidate that library.
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

/// Accepted, not started. The batcher holds a file for up to five seconds
/// before its batch goes, and the queue behind it is unordered, so `position`
/// is left absent rather than invented.
pub fn queued(relative_path: &str, subject_id: i64) {
    Status::new(relative_path, subject_id, "queued").emit();
}

/// A page landed. `total_pages` is zero until the backend knows how long the
/// document is; it is dropped rather than sent as a zero denominator, which
/// the UI would render as a finished bar.
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

/// Terminal failure. `error` is `ParseError`'s `Display`, which is written for
/// a student and — load-bearing — **never contains server response text**:
/// MinerU's error bodies can carry the signed URLs it issued for the upload,
/// and this string ends up on screen, in logs and in bug reports.
pub fn failed(relative_path: &str, subject_id: i64, error: &ParseError) {
    let mut status = Status::new(relative_path, subject_id, "error");
    status.error = Some(error.to_string());
    status.kind = Some(error.kind());
    status.retryable = Some(error.retryable());
    status.latching = Some(error.latching());
    status.emit();
}
