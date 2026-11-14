//! Ending a thread's session. Every way one ends goes through
//! [`Harness::release`], and the reason decides what becomes of a turn it was
//! running (`app/consumer.rs`, `Consumer::end_session`).

use super::handle::Live;
use super::Harness;

/// Why a session ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Release {
    /// Stopped for good; a turn it was running ends interrupted.
    Stopped,
    /// A different model, reasoning level or set of rules needs a new
    /// session; a turn it was running ends interrupted.
    Replaced,
    /// Its thread is being deleted, rows and all.
    Deleted,
    /// The process had already died; a turn it left open fails.
    Exited,
    /// The app is quitting; startup repairs what was open.
    Quit,
}

impl Live {
    /// The consumer is told before the kill, so whatever the dying process
    /// still says arrives stale.
    pub(super) fn end(self, reason: Release) {
        self.sinks.released(self.generation, reason);
        self.handle.close(false);
    }
}

impl Harness {
    /// End the thread's session, if it has one. The thread row stays; the
    /// next send resumes it by session id, as a new generation.
    pub fn release(&self, thread_id: i64, reason: Release) {
        let live = self.live.lock().unwrap().remove(&thread_id);
        if let Some(live) = live {
            live.end(reason);
        }
    }
}
