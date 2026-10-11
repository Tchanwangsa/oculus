//! The rate-limit gate: after a 429, no Web API call leaves until its
//! `Retry-After` has passed. The shared client id is throttled hard, and a
//! call made early only extends the wait.

use std::time::{Duration, Instant};

/// The wait when a 429 names none.
pub(super) const DEFAULT_WAIT: Duration = Duration::from_secs(30);

#[derive(Debug, Default)]
pub(super) struct Gate {
    until: Option<Instant>,
}

impl Gate {
    /// Whole seconds left to wait at `now` (rounded up), or `None` when open.
    pub(super) fn closed_for(&mut self, now: Instant) -> Option<u64> {
        let until = self.until?;
        if now >= until {
            self.until = None;
            return None;
        }
        let left = until - now;
        Some(left.as_secs() + u64::from(left.subsec_nanos() > 0))
    }

    /// A 429 at `now`; a later deadline already set is kept.
    pub(super) fn close(&mut self, now: Instant, wait: Duration) {
        let until = now + wait;
        if self.until.is_none_or(|u| u < until) {
            self.until = Some(until);
        }
    }
}

/// `Retry-After` as seconds. Spotify sends delta-seconds; an HTTP date or
/// anything else falls back to `DEFAULT_WAIT`.
pub(super) fn retry_after(header: Option<&str>) -> Duration {
    header
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_WAIT)
}
