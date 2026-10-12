//! Whose commit a `new_commit` frame announces. Every upload is a commit,
//! and the upload's HTTP answer names its hash: a frame whose hash one of
//! ours returned is ours. The frame can beat that answer, so a frame that
//! matches nothing while an upload is in flight is held until the uploads
//! finish, and only then called another client's.

use std::collections::VecDeque;

/// How many of our hashes are remembered: far more than can be in flight.
const REMEMBERED: usize = 64;

#[derive(Debug, Default)]
pub struct Commits {
    in_flight: usize,
    ours: VecDeque<String>,
    held: Vec<String>,
}

/// A commit to announce: its hash and whether we made it.
pub type Decided = (String, bool);

impl Commits {
    /// An upload is about to be sent.
    pub fn begin(&mut self) {
        self.in_flight += 1;
    }

    /// An upload finished, with the hash Ed answered when it succeeded.
    /// Returns the held frames this decides.
    pub fn finish(&mut self, hash: Option<&str>) -> Vec<Decided> {
        self.in_flight = self.in_flight.saturating_sub(1);
        if let Some(hash) = hash {
            if self.ours.len() == REMEMBERED {
                self.ours.pop_front();
            }
            self.ours.push_back(hash.to_string());
        }
        let mut decided = Vec::new();
        self.held.retain(|h| {
            let ours = self.ours.contains(h);
            if ours {
                decided.push((h.clone(), true));
            }
            !ours
        });
        if self.in_flight == 0 {
            decided.extend(self.held.drain(..).map(|h| (h, false)));
        }
        decided
    }

    /// A `new_commit` frame arrived: decided now, or held (`None`).
    pub fn arrived(&mut self, hash: &str) -> Option<Decided> {
        if self.ours.iter().any(|h| h == hash) {
            Some((hash.to_string(), true))
        } else if self.in_flight > 0 {
            self.held.push(hash.to_string());
            None
        } else {
            Some((hash.to_string(), false))
        }
    }
}
