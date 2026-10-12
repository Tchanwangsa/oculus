//! The session task's timing rules, kept pure so they are tested: how long
//! to wait before the next attempt, and whether the machine slept between
//! two poll ticks.

use std::time::{Duration, Instant, SystemTime};

pub(super) const BACKOFF_MIN: Duration = Duration::from_secs(2);
pub(super) const BACKOFF_MAX: Duration = Duration::from_secs(60);
/// A connection that lived this long was not flapping: its drop reconnects at
/// once and the backoff starts over.
pub(super) const STABLE_AFTER: Duration = Duration::from_secs(60);
/// Poll ticks come a second apart; a gap longer than this between two, on
/// either clock, means the machine slept. `Instant` stops during sleep on
/// macOS but keeps counting through suspend on Windows, so the wall clock
/// catches the one and `Instant` the other.
pub(in crate::music) const SLEPT_AFTER: Duration = Duration::from_secs(30);

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

/// A moment on both clocks, to tell a sleep on any OS.
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

    /// Whether the machine slept since this stamp, taken a poll tick ago. A
    /// wall clock set back reads as no wall time.
    pub(super) fn slept(&self) -> bool {
        let wall = self.wall.elapsed().unwrap_or_default();
        slept(wall, self.mono.elapsed())
    }
}

/// Whether the gap between two poll ticks, `wall` on the wall clock and
/// `mono` on `Instant`, says the machine slept in it.
pub(super) fn slept(wall: Duration, mono: Duration) -> bool {
    wall > SLEPT_AFTER || mono > SLEPT_AFTER
}
