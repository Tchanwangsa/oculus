//! Reset to scaffold, Ed's "Reset attempt": Ed puts the challenge's original
//! files back, then the session ends its terminals and makes the library
//! copy Ed's tree, conflict copies gone. Only `ed_workspace_reset`, after the
//! pane's confirm dialog, calls it.

use std::collections::BTreeMap;
use std::sync::mpsc::Sender;

use super::context::{sha256, Context};
use super::events::{Body, TermState};
use super::paths;
use super::queue::Place;
use super::reconcile::{self, Head, Removes};
use super::session::Session;
use super::socket::Out;
use super::terminal;

impl Session {
    /// Reset the challenge once the saves asked before it have landed. It
    /// keeps its queue turn to the end, so no save lands between Ed's reset
    /// and the library taking Ed's tree.
    pub fn reset(&self, place: Place) -> Result<(), String> {
        let _turn = place.wait();
        let core = &self.core;
        let (workspace_id, host) = core
            .online()
            .ok_or("The workspace is offline: resetting needs Ed's server")?;
        let known = core.shared().user_id;
        let user_id = match known {
            Some(id) => id,
            None => core.ed.user_id()?,
        };
        core.ed.reset_challenge(user_id, core.challenge_id)?;
        self.end_terminals();
        let taken = take_eds_tree(core, &workspace_id, &host, &self.out);
        core.emit(Body::Reset);
        taken.map_err(|e| {
            format!(
                "Ed reset the challenge, but its files are not in the library yet ({e}); \
                 they come down when the workspace next connects"
            )
        })
    }

    /// Ed ends every terminal on a reset; kill and forget ours, and tell the
    /// pane the ones it held are gone.
    fn end_terminals(&self) {
        let (kill, held) = self.core.shared().terminals.end_all();
        for id in kill {
            let _ = self.out.send(Out::Frame(terminal::kill_frame(&id)));
        }
        for id in held {
            let state = TermState::Exited;
            self.core.emit(Body::TermState { id, state });
        }
    }
}

/// Make the folder Ed's tree. Every local file is recorded as the base
/// first, so the reconcile takes Ed's side of each difference (writes Ed's
/// files, deletes the ones Ed lacks) and uploads nothing. Cut short, the
/// next connect's reconcile finishes it the same way.
fn take_eds_tree(
    core: &Context,
    workspace_id: &str,
    host: &str,
    out: &Sender<Out>,
) -> Result<(), String> {
    let root = core.root();
    let mut files = BTreeMap::new();
    for path in paths::list(&root) {
        let target = paths::resolve(&root, &path)?;
        if reconcile::is_server_copy(&path) {
            std::fs::remove_file(&target).map_err(|e| format!("could not delete {path}: {e}"))?;
            continue;
        }
        let bytes = std::fs::read(&target).map_err(|e| format!("could not read {path}: {e}"))?;
        files.insert(path, sha256(&bytes));
    }
    core.record(workspace_id, |state| {
        state.files = files;
        state.base_commit = None;
    })?;
    let head = Head {
        hash: None,
        changed: true,
    };
    reconcile::reconcile(core, workspace_id, host, head, Removes::Send(out))
}
