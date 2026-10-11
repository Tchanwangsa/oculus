//! The token bucket.

use super::hold;

use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

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
        self.acquire_n_reporting(cost, &mut |_| {});
    }

    /// `acquire_n`, telling `on_wait` how long the refill should take each
    /// time it is about to block. Called under the bucket's lock: keep it cheap.
    pub fn acquire_n_reporting(&self, cost: f64, on_wait: &mut dyn FnMut(Duration)) {
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
            let refill = (want - state.tokens) / state.rate;
            on_wait(Duration::from_secs_f64(refill));
            let wait = Duration::from_secs_f64(refill.clamp(0.001, 60.0));
            state = self
                .wake
                .wait_timeout(state, wait)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// How long `acquire_n(cost)` would block if called now, ignoring other
    /// waiters. Takes nothing.
    pub fn shortfall(&self, cost: f64) -> Duration {
        let state = hold(&self.state);
        let elapsed = state.updated.elapsed().as_secs_f64();
        let tokens = state.capacity.min(state.tokens + elapsed * state.rate);
        let want = cost.max(0.0).min(state.capacity);
        Duration::from_secs_f64(((want - tokens) / state.rate).max(0.0))
    }

    /// Move the ceiling. `drain` empties the bucket, so the burst that earned a
    /// 429 does not repeat when the pause lifts.
    pub fn retune(&self, per_minute: f64, drain: bool) {
        let per_minute = per_minute.max(1.0);
        let mut state = hold(&self.state);
        state.capacity = per_minute;
        state.rate = per_minute / 60.0;
        state.tokens = if drain {
            0.0
        } else {
            state.tokens.min(per_minute)
        };
        state.updated = Instant::now();
        drop(state);
        self.wake.notify_all();
    }

    pub fn per_minute(&self) -> f64 {
        hold(&self.state).capacity
    }
}
