//! The retry ladder.

use super::nap;

use std::time::Duration;

const FIRST_RETRY: Duration = Duration::from_secs(1);
const MAX_RETRY: Duration = Duration::from_secs(10);

/// `attempts` tries per call, sleeping 1s between the first two and doubling
/// to a 10s ceiling. A wait that is not a failure (a 429) naps without
/// stepping the ladder.
pub struct Retry {
    attempt: u32,
    attempts: u32,
    delay: Duration,
    time_scale: f64,
}

impl Retry {
    pub fn new(attempts: u32, time_scale: f64) -> Self {
        Self {
            attempt: 0,
            attempts,
            delay: FIRST_RETRY,
            time_scale,
        }
    }

    pub fn attempts_left(&self) -> bool {
        self.attempt < self.attempts
    }

    /// Back off and return `true` if another attempt remains; `false`, without
    /// sleeping, when this was the last.
    pub fn back_off(&mut self) -> bool {
        if self.attempt + 1 >= self.attempts {
            return false;
        }
        self.attempt += 1;
        nap(self.delay, self.time_scale);
        self.delay = (self.delay * 2).min(MAX_RETRY);
        true
    }
}
