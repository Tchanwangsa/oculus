//! Shared file-pipeline event contract. Parse and embed keep their own event
//! channels, terminal status vocabulary and backend error policies.

use std::sync::OnceLock;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub(crate) struct Channel {
    name: &'static str,
    app: OnceLock<AppHandle>,
}

impl Channel {
    pub(crate) const fn new(name: &'static str) -> Self {
        Self { name, app: OnceLock::new() }
    }

    /// Bind once in app setup. Unbound CLI channels are no-ops.
    pub(crate) fn bind(&self, app: AppHandle) {
        let _ = self.app.set(app);
    }

    pub(crate) fn emit(&self, status: Status) {
        if let Some(app) = self.app.get() {
            app.emit(self.name, status).ok();
        }
    }
}

/// Optional discriminants stay absent when unknown, rather than becoming
/// false. The frontend folds both channels into the same pipeline rows.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Status {
    relative_path: String,
    subject_id: i64,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pages_done: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_pages: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latching: Option<bool>,
    /// While a rate limit holds the work: epoch ms it expects to resume, and
    /// why. Absent on any other event, which is what clears it.
    #[serde(skip_serializing_if = "Option::is_none")]
    waiting_until_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    waiting_reason: Option<String>,
}

impl Status {
    pub(crate) fn new(relative_path: &str, subject_id: i64, status: &'static str) -> Self {
        Self {
            relative_path: relative_path.to_string(), subject_id, status,
            pages_done: None, total_pages: None, error: None,
            kind: None, retryable: None, latching: None,
            waiting_until_ms: None, waiting_reason: None,
        }
    }

    /// An unknown total is omitted so it cannot draw a finished progress bar.
    pub(crate) fn progress(mut self, done: u32, total: u32) -> Self {
        self.pages_done = Some(done);
        self.total_pages = (total > 0).then_some(total);
        self
    }

    pub(crate) fn failure(
        mut self,
        error: String,
        kind: Option<&'static str>,
        retryable: Option<bool>,
        latching: Option<bool>,
    ) -> Self {
        self.error = Some(error);
        self.kind = kind;
        self.retryable = retryable;
        self.latching = latching;
        self
    }

    pub(crate) fn waiting(mut self, until_ms: u64, reason: String) -> Self {
        self.waiting_until_ms = Some(until_ms);
        self.waiting_reason = Some(reason);
        self
    }

    /// A completed embed reports its exact page count, including an empty set.
    pub(crate) fn completed(mut self, pages: u32) -> Self {
        self.pages_done = Some(pages);
        self.total_pages = Some(pages);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    #[test]
    fn queued_and_parse_success_keep_the_wire_contract() {
        for state in ["queued", "quality"] {
            assert_eq!(to_value(Status::new("courses/a.pdf", 4, state)).unwrap(), json!({
                "relative_path": "courses/a.pdf", "subject_id": 4, "status": state,
            }));
        }
    }

    #[test]
    fn unknown_progress_total_is_absent_but_terminal_zero_is_exact() {
        let running = to_value(Status::new("a.pdf", 4, "running").progress(0, 0)).unwrap();
        assert_eq!(running["pages_done"], 0);
        assert!(running.get("total_pages").is_none());
        let done = to_value(Status::new("a.pdf", 4, "done").completed(0)).unwrap();
        assert_eq!(done["total_pages"], 0);
    }

    #[test]
    fn a_wait_rides_on_running_and_is_otherwise_absent() {
        let plain = to_value(Status::new("a.pdf", 4, "running").progress(3, 9)).unwrap();
        assert!(plain.get("waiting_until_ms").is_none());
        assert!(plain.get("waiting_reason").is_none());
        let held = to_value(Status::new("a.pdf", 4, "running").progress(3, 9)
            .waiting(1_700_000_000_000, "rate-limited by Voyage".into())).unwrap();
        assert_eq!(held["waiting_until_ms"], 1_700_000_000_000u64);
        assert_eq!(held["waiting_reason"], "rate-limited by Voyage");
        assert_eq!(held["pages_done"], 3);
    }

    #[test]
    fn failures_preserve_unknown_and_false_discriminants() {
        let unknown = to_value(Status::new("a.pdf", 4, "error")
            .failure("failed".into(), None, None, None)).unwrap();
        assert!(unknown.get("kind").is_none());
        assert!(unknown.get("retryable").is_none());
        assert!(unknown.get("latching").is_none());
        let known = to_value(Status::new("a.pdf", 4, "error")
            .failure("failed".into(), Some("document"), Some(false), Some(false))).unwrap();
        assert_eq!(known["kind"], "document");
        assert_eq!(known["retryable"], false);
        assert_eq!(known["latching"], false);
    }
}
