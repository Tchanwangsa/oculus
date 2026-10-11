//! A bounded reorder buffer: workers make items out of order, one reader
//! takes them in order. A worker may only claim index `i` while `i` is
//! within `window` of the next index the reader takes, so at most `window`
//! items are held or in flight whatever the workers' speeds.

use std::collections::BTreeMap;
use std::sync::{Condvar, Mutex, MutexGuard};

pub(super) struct Reorder<T> {
    state: Mutex<State<T>>,
    /// Signalled when an item lands or the buffer stops.
    landed: Condvar,
    /// Signalled when the reader takes an item or the buffer stops.
    taken: Condvar,
    total: usize,
    window: usize,
}

struct State<T> {
    next_claim: usize,
    next_take: usize,
    held: BTreeMap<usize, T>,
    stopped: bool,
}

impl<T> Reorder<T> {
    /// For indices `0..total`; `window` is at least 1.
    pub fn new(total: usize, window: usize) -> Self {
        Self {
            state: Mutex::new(State {
                next_claim: 0,
                next_take: 0,
                held: BTreeMap::new(),
                stopped: false,
            }),
            landed: Condvar::new(),
            taken: Condvar::new(),
            total,
            window: window.max(1),
        }
    }

    /// The next index to make, waiting for room; `None` once every index is
    /// claimed or the buffer has stopped.
    pub fn claim(&self) -> Option<usize> {
        let mut s = self.lock();
        loop {
            if s.stopped || s.next_claim >= self.total {
                return None;
            }
            if s.next_claim < s.next_take + self.window {
                s.next_claim += 1;
                return Some(s.next_claim - 1);
            }
            s = self.taken.wait(s).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// Hand back the item for a claimed index.
    pub fn put(&self, index: usize, item: T) {
        let mut s = self.lock();
        if !s.stopped {
            s.held.insert(index, item);
        }
        drop(s);
        self.landed.notify_all();
    }

    /// The next item in index order, waiting for it; `None` once all have
    /// been taken or the buffer has stopped.
    pub fn take(&self) -> Option<T> {
        let mut s = self.lock();
        loop {
            if s.stopped || s.next_take >= self.total {
                return None;
            }
            let next = s.next_take;
            if let Some(item) = s.held.remove(&next) {
                s.next_take += 1;
                drop(s);
                self.taken.notify_all();
                return Some(item);
            }
            s = self.landed.wait(s).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// Wake everyone and end the run: claims and takes answer `None`, held
    /// items are dropped.
    pub fn stop(&self) {
        let mut s = self.lock();
        s.stopped = true;
        s.held.clear();
        drop(s);
        self.landed.notify_all();
        self.taken.notify_all();
    }

    /// Items held now, not yet taken.
    #[cfg(test)]
    pub fn held(&self) -> usize {
        self.lock().held.len()
    }

    fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}
