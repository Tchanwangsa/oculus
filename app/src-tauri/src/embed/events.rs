//! `embed-status` — the one event the embedding path emits.
//!
//! A deliberate copy of `parse/events.rs`, down to the field names, because the
//! two seams are one story to the UI: a row in the pipeline table watches
//! Download → Parse → Embed and must not have to learn a second vocabulary at
//! the third stage. The `AppHandle` is bound once at startup for the same
//! reason it is there — threading one through `Embedder::embed` would put a
//! Tauri type in the middle of code the CLI runs, and **a headless run leaves
//! this unbound and every emit is a no-op**.
//!
//! What this event is *for* is the thing `docs/retrieval.md` used to list as
//! the honest gap: page-level progress. A document is one call that blocks for
//! the whole round trip, and on a Voyage account with no payment method that is
//! ~2.8 pages a minute — a 200-page deck is over an hour of one filename on
//! screen and nothing else. The counter was always there (`embed::Progress`,
//! summed from finished requests in `voyage/batch.rs`); it had nowhere to go.

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

/// The payload, and a **fixed contract** — `app/src/hooks/useBackendEvents.ts`
/// and `app/src/stores/pipelineStore.ts` read exactly these field names.
/// Optional fields are omitted rather than sent as null, so a reader that sees
/// a key can trust it.
///
/// `status` is `queued | running | done | error`. The success word is `done`
/// and not `quality`: that name is the parse path's frozen historical baggage
/// (`files.parse_status` says it in every already-parsed row) and there is no
/// reason to inherit it here, where nothing has been written down yet.
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

/// Accepted, not started. There is no queue in Rust — the runner is the app's
/// serial index loop — so this is emitted by the command as it takes the file,
/// which is what turns the row's third dot on before the first request goes.
pub fn queued(relative_path: &str, subject_id: i64) {
    Status::new(relative_path, subject_id, "queued").emit();
}

/// Pages finished. `total_pages` is zero until the parse record has been read;
/// it is dropped rather than sent as a zero denominator, which the UI would
/// draw as a finished bar.
pub fn running(relative_path: &str, subject_id: i64, progress: Progress) {
    let mut status = Status::new(relative_path, subject_id, "running");
    status.pages_done = Some(progress.pages_done);
    status.total_pages = (progress.total_pages > 0).then_some(progress.total_pages);
    status.emit();
}

/// Terminal success. `pages` is what actually landed in the table, which for a
/// file that was already embedded is the record being folded in rather than a
/// round trip.
pub fn embedded(relative_path: &str, subject_id: i64, pages: u32) {
    let mut status = Status::new(relative_path, subject_id, "done");
    status.pages_done = Some(pages);
    status.total_pages = Some(pages);
    status.emit();
}

/// Terminal failure. `error` is `EmbedError`'s `Display`, which is written for
/// a student and — load-bearing — **never contains server response text**: an
/// error body from this API can echo the request, which means the base64 of a
/// page image, and this string ends up on screen and in bug reports.
///
/// The three discriminants ride along because the failure UI cannot work them
/// out from prose: whether to offer a retry at all is `retryable`, and whether
/// the run should stop rather than report one fact a hundred times is
/// `latching`.
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

/// The same, for a caller that has already unpacked the discriminants — or has
/// none, because the failure never reached a backend (the file was not on
/// disk, the database refused the write). `None` travels as an absent key, and
/// the UI treats unknown as its own case rather than coercing it to either
/// extreme.
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
