//! Pulling another client's commit while the session is live. The socket
//! thread only marks a pull wanted; a worker thread runs it in the save
//! queue, so it lands between saves and never stalls the socket's reads.
//! Commits that arrive during a pull make one more pull, not one each.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use super::compare::Head;
use super::in_session::{self, Removes};
use crate::sources::ed::workspace_session::context::Context;
use crate::sources::ed::workspace_session::events::Body;
use crate::sources::ed::workspace_session::socket::Out;

#[derive(Default)]
pub struct Puller {
    state: Mutex<PullState>,
    wake: Condvar,
}

#[derive(Default)]
struct PullState {
    /// Ed's newest commit from another client, not pulled yet.
    wanted: Option<String>,
    /// A pull failed or found the session offline: the next connect
    /// compares in full even when Ed's head is the base.
    owed: bool,
    closed: bool,
}

impl Puller {
    fn state(&self) -> MutexGuard<'_, PullState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn want(&self, hash: &str) {
        self.state().wanted = Some(hash.to_string());
        self.wake.notify_all();
    }

    /// End the worker.
    pub fn close(&self) {
        self.state().closed = true;
        self.wake.notify_all();
    }

    /// Whether a pull is owed, clearing it: a connect's reconcile serves it.
    pub fn take_owed(&self) -> bool {
        let mut state = self.state();
        let owed = state.owed || state.wanted.is_some();
        state.owed = false;
        state.wanted = None;
        owed
    }

    pub fn owe(&self) {
        self.state().owed = true;
    }

    /// Wait for a wanted pull; `None` once closed.
    fn next(&self) -> Option<String> {
        let mut state = self.state();
        loop {
            if state.closed {
                return None;
            }
            if let Some(hash) = state.wanted.take() {
                return Some(hash);
            }
            state = self.wake.wait(state).unwrap_or_else(|e| e.into_inner());
        }
    }
}

/// Start the session's pull worker; it ends when the puller is closed.
pub fn spawn(core: Arc<Context>, out: Sender<Out>) -> Result<(), String> {
    std::thread::Builder::new()
        .name(format!("ed-workspace-pull-{}", core.challenge_id))
        .spawn(move || {
            while let Some(hash) = core.puller.next() {
                pull(&core, &out, &hash);
            }
        })
        .map(|_| ())
        .map_err(|e| format!("could not start the workspace session: {e}"))
}

fn pull(core: &Context, out: &Sender<Out>, hash: &str) {
    let _turn = core.queue.place().wait();
    let Some((workspace_id, host)) = core.online() else {
        core.puller.owe();
        return;
    };
    if !core.has_record() {
        return;
    }
    let head = Head {
        hash: Some(hash),
        changed: true,
    };
    let pulled = in_session::reconcile(core, &workspace_id, &host, head, Removes::Send(out));
    if let Err(e) = pulled {
        core.puller.owe();
        elog!(
            "[oculus] ed workspace {}: pulling Ed's change failed: {e}",
            core.challenge_id
        );
        core.emit(Body::Error {
            message: format!("Ed's latest change is not in the library yet: {e}"),
        });
    }
}
