//! One change to a workspace at a time, in the order they were asked for: a
//! later save of a file must land after an earlier one. A plain mutex does
//! not promise that order, so each change takes a numbered place first.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
pub struct Queue {
    next: AtomicU64,
    serving: Mutex<u64>,
    turn: Condvar,
}

/// A place in the queue, taken when the change is asked for.
pub struct Place {
    queue: Arc<Queue>,
    number: u64,
    waited: bool,
}

/// The current turn; the next place is served when it drops.
pub struct Turn {
    queue: Arc<Queue>,
}

impl Queue {
    pub fn place(self: &Arc<Self>) -> Place {
        Place {
            queue: self.clone(),
            number: self.next.fetch_add(1, Ordering::SeqCst),
            waited: false,
        }
    }

    fn wait_for(&self, number: u64) {
        let mut serving = self.serving.lock().unwrap_or_else(|e| e.into_inner());
        while *serving != number {
            serving = self.turn.wait(serving).unwrap_or_else(|e| e.into_inner());
        }
    }

    fn advance(&self) {
        *self.serving.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        self.turn.notify_all();
    }
}

impl Place {
    /// Block until every earlier place has had its turn.
    pub fn wait(mut self) -> Turn {
        self.queue.wait_for(self.number);
        self.waited = true;
        Turn {
            queue: self.queue.clone(),
        }
    }
}

impl Drop for Place {
    /// A place given up unused still has to pass, or every later one waits
    /// forever.
    fn drop(&mut self) {
        if !self.waited {
            self.queue.wait_for(self.number);
            self.queue.advance();
        }
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        self.queue.advance();
    }
}
