//! Reconcile inside an open session: Ed reached through the session's
//! workspace, the outcome saved to the record and told to the pane. The
//! caller holds the session's queue turn.

use std::sync::mpsc::Sender;

use super::compare::{self, Head, Remote};
use crate::sources::ed::sahara;
use crate::sources::ed::workspace_session::context::Context;
use crate::sources::ed::workspace_session::events::Body;
use crate::sources::ed::workspace_session::frames;
use crate::sources::ed::workspace_session::socket::Out;
use crate::sources::ed::workspace_state;

/// How an `fsop remove` reaches the socket.
pub enum Removes<'a> {
    /// Kept for the socket thread to send once online (a connect runs on
    /// that thread, before it serves).
    Collect(&'a mut Vec<String>),
    /// Queued for the socket thread (a pull, on the worker).
    Send(&'a Sender<Out>),
}

struct SessionRemote<'a> {
    core: &'a Context,
    workspace_id: &'a str,
    host: &'a str,
    removes: Removes<'a>,
}

impl Remote for SessionRemote<'_> {
    fn tree(&mut self) -> Result<Vec<u8>, String> {
        let ticket = self.core.ed.download_ticket(self.workspace_id, ".")?;
        sahara::download(&self.core.origin(self.host), &ticket)
    }

    fn upload(&mut self, path: &str, bytes: &[u8]) -> Result<String, String> {
        self.core.upload(self.workspace_id, self.host, path, bytes)
    }

    fn remove(&mut self, path: &str) -> Result<(), String> {
        let frame = frames::remove(path, false);
        match &mut self.removes {
            Removes::Collect(frames) => frames.push(frame),
            Removes::Send(out) => out
                .send(Out::Frame(frame))
                .map_err(|_| "the workspace session has ended".to_string())?,
        }
        Ok(())
    }
}

/// Reconcile the session's folder with Ed, save the record, and emit
/// `files` and one `conflict` per file in conflict. A record that cannot be
/// read counts as empty: every difference is then a conflict, never a loss.
pub fn reconcile(
    core: &Context,
    workspace_id: &str,
    host: &str,
    head: Head,
    removes: Removes,
) -> Result<(), String> {
    let record = workspace_state::load(&core.data_dir, core.challenge_id).unwrap_or_default();
    let mut remote = SessionRemote {
        core,
        workspace_id,
        host,
        removes,
    };
    let outcome = compare::run(&core.root(), &record, head, &mut remote)?;
    for line in &outcome.skipped {
        elog!("[oculus] ed workspace {}: {line}", core.challenge_id);
    }
    core.record(workspace_id, |state| {
        state.workspace_id = workspace_id.to_string();
        state.files = outcome.files.clone();
        state.base_commit = outcome.base_commit.clone();
    })?;
    core.emit_files();
    for conflict in outcome.conflicts {
        core.emit(Body::Conflict {
            path: conflict.path,
            server_path: conflict.server_path,
        });
    }
    Ok(())
}
