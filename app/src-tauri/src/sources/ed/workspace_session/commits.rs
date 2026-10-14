//! Whose commit a `new_commit` frame announces. Every upload is a commit,
//! and the upload's HTTP answer names its hash: a frame whose hash one of
//! ours returned is ours. An upload also makes a snapshot commit whose hash
//! it never returns, and its own frame can beat the answer, so any frame
//! that arrives while one of our uploads is in flight is ours too. Another
//! client's commit hidden that way is caught by the next reconcile.

use std::collections::VecDeque;

/// How many of our hashes are remembered: far more than can be in flight.
const REMEMBERED: usize = 64;

#[derive(Debug, Default)]
pub struct Commits {
    in_flight: usize,
    ours: VecDeque<String>,
}

impl Commits {
    /// An upload is about to be sent.
    pub fn begin(&mut self) {
        self.in_flight += 1;
    }

    /// An upload finished, with the hash Ed answered when it succeeded.
    pub fn finish(&mut self, hash: Option<&str>) {
        self.in_flight = self.in_flight.saturating_sub(1);
        if let Some(hash) = hash {
            if self.ours.len() == REMEMBERED {
                self.ours.pop_front();
            }
            self.ours.push_back(hash.to_string());
        }
    }

    /// A `new_commit` frame arrived: whether it is ours.
    pub fn arrived(&mut self, hash: &str) -> bool {
        self.in_flight > 0 || self.ours.iter().any(|h| h == hash)
    }
}
