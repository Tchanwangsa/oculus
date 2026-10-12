//! The session task's timing rules, kept pure so they are tested: how long
//! to wait before the next attempt, and whether the Mac slept since a moment.

use std::time::{Duration, Instant, SystemTime};

pub(super) const BACKOFF_MIN: Duration = Duration::from_secs(2);
pub(super) const BACKOFF_MAX: Duration = Duration::from_secs(60);
/// A connection that lived this long was not flapping: its drop reconnects at
/// once and the backoff starts over.
pub(super) const STABLE_AFTER: Duration = Duration::from_secs(60);
/// Wall-clock time running ahead of monotonic time by more than this means
/// the Mac slept: `Instant` stops during macOS sleep.
pub(super) const SLEPT_AFTER: Duration = Duration::from_secs(30);

/// The wait before each attempt: none after a stable connection drops, then
/// 2 s doubling to 60 s for attempts that fail.
#[derive(Debug)]
pub(super) struct Backoff {
    next: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Self { next: BACKOFF_MIN }
    }
}

impl Backoff {
    /// The wait after a connection that lived `uptime` dropped. One that kept
    /// dropping at once takes the next step instead, so it cannot spin.
    pub(super) fn after_drop(&mut self, uptime: Duration) -> Duration {
        if uptime >= STABLE_AFTER {
            self.next = BACKOFF_MIN;
            return Duration::ZERO;
        }
        self.step()
    }

    /// The wait after an attempt that failed.
    pub(super) fn after_failure(&mut self) -> Duration {
        self.step()
    }

    fn step(&mut self) -> Duration {
        let wait = self.next;
        self.next = (wait * 2).min(BACKOFF_MAX);
        wait
    }
}

/// A moment on both clocks, to tell sleep from time awake.
#[derive(Clone, Copy, Debug)]
pub(super) struct Stamp {
    wall: SystemTime,
    mono: Instant,
}

impl Stamp {
    pub(super) fn now() -> Self {
        Self {
            wall: SystemTime::now(),
            mono: Instant::now(),
        }
    }

    /// Time since this stamp, sleep included.
    pub(super) fn lived(&self) -> Duration {
        self.wall.elapsed().unwrap_or_else(|_| self.mono.elapsed())
    }

    /// Whether the Mac slept since this stamp.
    pub(super) fn slept(&self) -> bool {
        let wall = self.wall.elapsed().unwrap_or_default();
        slept(wall, self.mono.elapsed())
    }
}

/// Whether `wall` elapsed time against `mono` elapsed time over the same span
/// says the Mac slept in it.
pub(super) fn slept(wall: Duration, mono: Duration) -> bool {
    wall.saturating_sub(mono) > SLEPT_AFTER
}
