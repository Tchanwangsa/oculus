//! The pacing both cloud clients (MinerU, Voyage) share: a token bucket, a
//! counting semaphore, the retry ladder, and how a transport failure reads.

mod bucket;
mod permits;
mod retry;

#[cfg(test)]
mod tests;
mod transport;

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

pub use bucket::TokenBucket;
pub use permits::Permits;
pub use retry::Retry;
pub use transport::transport_detail;

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
