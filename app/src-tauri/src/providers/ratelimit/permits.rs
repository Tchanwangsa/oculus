//! The counting semaphore.

use super::hold;

use std::sync::{Condvar, Mutex, PoisonError};

/// A counting semaphore; `std` has none.
pub struct Permits {
    free: Mutex<usize>,
    wake: Condvar,
}

impl Permits {
    pub fn new(count: usize) -> Self {
        Self {
            free: Mutex::new(count),
            wake: Condvar::new(),
        }
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
