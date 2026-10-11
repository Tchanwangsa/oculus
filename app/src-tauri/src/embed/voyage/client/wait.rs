//! Reporting a rate-limit wait once, however many places see it.

use crate::embed::Wait;
use std::time::Duration;

/// A pacing wait shorter than this is not reported: the row is still moving.
/// A 429's wait is always reported.
pub(super) const REPORTED_WAIT: Duration = Duration::from_secs(2);

/// What one request has said about its waits, so the same wait seen twice
/// (the 429's nap, then the gate's pause it set) is one event, not two.
pub(super) struct WaitNotice<'a> {
    on_wait: &'a dyn Fn(Option<Wait>),
    shown_until_ms: Option<u64>,
}

impl<'a> WaitNotice<'a> {
    pub(super) fn new(on_wait: &'a dyn Fn(Option<Wait>)) -> Self {
        Self {
            on_wait,
            shown_until_ms: None,
        }
    }

    /// Reported unless it ends within the threshold of the one already shown.
    pub(super) fn show(&mut self, wait: Wait) {
        let threshold = REPORTED_WAIT.as_millis() as u64;
        if let Some(shown) = self.shown_until_ms {
            if shown.abs_diff(wait.until_ms) <= threshold {
                return;
            }
        }
        self.shown_until_ms = Some(wait.until_ms);
        (self.on_wait)(Some(wait));
    }

    pub(super) fn clear(&mut self) {
        if self.shown_until_ms.take().is_some() {
            (self.on_wait)(None);
        }
    }
}
