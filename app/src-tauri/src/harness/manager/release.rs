//! Ending a thread's session. Every way one ends goes through
//! [`Harness::release`], and the reason decides what becomes of a turn it was
//! running (`app/consumer.rs`, `Consumer::end_session`).

use crate::harness::Provider;

use super::handle::Live;
use super::Harness;

/// Why a session ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Release {
    /// Force stopped: its CLI ignored Stop. A turn it was running ends
    /// interrupted, and its thread's queue is held.
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
    /// Force stop: kill what is answering this thread, for a turn whose CLI
    /// ignored Stop. Claude and Antigravity run a process per thread, so
    /// only that one goes. Codex and opencode answer every thread from one
    /// server: each of its threads is released first, as stopped, so their
    /// turns end interrupted and whatever the dying server says arrives
    /// stale, and then the server is killed. The next send starts a new one.
    pub fn force_stop(&self, thread_id: i64) {
        let provider = {
            let live = self.live.lock().unwrap();
            live.get(&thread_id).map(|l| l.handle.provider())
        };
        match provider {
            Some(provider @ (Provider::Codex | Provider::Opencode)) => {
                for id in self.live_threads(provider) {
                    self.release(id, Release::Stopped);
                }
                match provider {
                    Provider::Codex => {
                        if let Some(server) = self.codex.lock().unwrap().take() {
                            server.kill();
                        }
                    }
                    _ => {
                        if let Some(server) = self.opencode.lock().unwrap().take() {
                            server.kill();
                        }
                    }
                }
            }
            Some(_) => self.release(thread_id, Release::Stopped),
            None => {}
        }
    }

    /// End the thread's session, if it has one. The thread row stays; the
    /// next send resumes it by session id, as a new generation.
    pub fn release(&self, thread_id: i64, reason: Release) {
        let live = self.live.lock().unwrap().remove(&thread_id);
        if let Some(live) = live {
            live.end(reason);
        }
    }
}
