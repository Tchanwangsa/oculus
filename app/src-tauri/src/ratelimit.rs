//! The pacing both cloud clients (MinerU, Voyage) share: a token bucket, a
//! counting semaphore, and the retry ladder.

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// A poisoned lock still holds readable state; carry on rather than refuse.
pub fn hold<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Sleep `duration` scaled by `time_scale`, which tests set near zero.
pub fn nap(duration: Duration, time_scale: f64) {
    let scaled = duration.mul_f64(time_scale);
    if !scaled.is_zero() {
        std::thread::sleep(scaled);
    }
}

// ── Token bucket ─────────────────────────────────────────────────────────────

/// Starts full, refills at `per_minute / 60` a second, and `acquire_n` blocks
/// until there is enough. Share one per limit, or each caller spends the whole
/// limit.
pub struct TokenBucket {
    state: Mutex<BucketState>,
    wake: Condvar,
}

struct BucketState {
    capacity: f64,
    rate: f64,
    tokens: f64,
    updated: Instant,
}

impl TokenBucket {
    pub fn new(per_minute: f64) -> Self {
        let per_minute = per_minute.max(1.0);
        Self {
            state: Mutex::new(BucketState {
                capacity: per_minute,
                rate: per_minute / 60.0,
                tokens: per_minute,
                updated: Instant::now(),
            }),
            wake: Condvar::new(),
        }
    }

    pub fn acquire(&self) {
        self.acquire_n(1.0);
    }

    /// Take `cost`, waiting for the refill if short. `cost` is clamped to the
    /// capacity: a bucket that can never hold it would deadlock.
    pub fn acquire_n(&self, cost: f64) {
        let mut state = hold(&self.state);
        loop {
            let now = Instant::now();
            let elapsed = now.duration_since(state.updated).as_secs_f64();
            state.tokens = state.capacity.min(state.tokens + elapsed * state.rate);
            state.updated = now;

            let want = cost.max(0.0).min(state.capacity);
            if state.tokens >= want {
                state.tokens -= want;
                return;
            }
            let short_by = want - state.tokens;
            let wait = Duration::from_secs_f64((short_by / state.rate).clamp(0.001, 60.0));
            state = self.wake.wait_timeout(state, wait).unwrap_or_else(PoisonError::into_inner).0;
        }
    }

    /// Move the ceiling. `drain` empties the bucket, so the burst that earned a
    /// 429 does not repeat when the pause lifts.
    pub fn retune(&self, per_minute: f64, drain: bool) {
        let per_minute = per_minute.max(1.0);
        let mut state = hold(&self.state);
        state.capacity = per_minute;
        state.rate = per_minute / 60.0;
        state.tokens = if drain { 0.0 } else { state.tokens.min(per_minute) };
        state.updated = Instant::now();
        drop(state);
        self.wake.notify_all();
    }

    pub fn per_minute(&self) -> f64 {
        hold(&self.state).capacity
    }
}

// ── Permits ──────────────────────────────────────────────────────────────────

/// A counting semaphore; `std` has none.
pub struct Permits {
    free: Mutex<usize>,
    wake: Condvar,
}

impl Permits {
    pub fn new(count: usize) -> Self {
        Self { free: Mutex::new(count), wake: Condvar::new() }
    }

    pub fn acquire(&self) {
        let mut free = hold(&self.free);
        while *free == 0 {
            free = self.wake.wait(free).unwrap_or_else(PoisonError::into_inner);
        }
        *free -= 1;
    }

    pub fn release(&self) {
        *hold(&self.free) += 1;
        self.wake.notify_one();
    }
}

// ── Retry ladder ─────────────────────────────────────────────────────────────

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
        Self { attempt: 0, attempts, delay: FIRST_RETRY, time_scale }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bucket_starts_full_and_then_paces() {
        let bucket = TokenBucket::new(600.0);
        let start = Instant::now();
        for _ in 0..600 {
            bucket.acquire();
        }
        assert!(start.elapsed() < Duration::from_millis(500), "a full bucket should not wait");

        let paced = Instant::now();
        bucket.acquire();
        assert!(paced.elapsed() >= Duration::from_millis(50), "an empty bucket must wait");
    }

    #[test]
    fn a_cost_larger_than_a_whole_minute_does_not_deadlock() {
        let bucket = TokenBucket::new(6_000.0);
        let start = Instant::now();
        bucket.acquire_n(320_000.0);
        assert!(start.elapsed() < Duration::from_millis(200), "{:?}", start.elapsed());
    }

    #[test]
    fn retuning_narrows_the_ceiling_and_can_drain_the_burst() {
        let bucket = TokenBucket::new(60_000.0);
        bucket.retune(6_000.0, true);
        assert_eq!(bucket.per_minute(), 6_000.0);
        let start = Instant::now();
        bucket.acquire_n(100.0);
        // Drained: even a token has to be waited for.
        assert!(start.elapsed() >= Duration::from_millis(500), "{:?}", start.elapsed());
    }

    #[test]
    fn the_ladder_allows_exactly_its_attempts() {
        let mut retry = Retry::new(4, 0.0);
        let mut backoffs = 0;
        while retry.attempts_left() && retry.back_off() {
            backoffs += 1;
        }
        assert_eq!(backoffs, 3);
    }
}
