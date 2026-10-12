//! What a connect does before the session goes online. A challenge with no
//! sync record takes Ed's tree as it is now and records it, after which the
//! lesson sync leaves the folder alone (`docs/sync.md`). One with a record
//! holds local work; matching that with Ed is reconcile's job (`reconcile`).

use std::collections::BTreeMap;

use super::context::{forget, sha256, write_file, Context, Unrecorded};
use super::frames::{self, Init};
use super::paths;
use crate::sources::ed::workspace_state::WorkspaceState;
use crate::sources::ed::{sahara, unpack_workspace};

/// Ready the folder for a live session. Returns frames to send once the
/// socket is serving.
pub fn on_connect(
    core: &Context,
    workspace_id: &str,
    host: &str,
    init: &Init,
) -> Result<Vec<String>, String> {
    let _turn = core.queue.place().wait();
    if core.has_record() {
        return reconcile(core, init);
    }
    first_connect(core, workspace_id, host, init)
}

/// A workspace with a record goes online as it is: its files are not
/// compared with Ed's, so changes made offline stay local until they are.
fn reconcile(_core: &Context, _init: &Init) -> Result<Vec<String>, String> {
    Ok(Vec::new())
}

/// Download the tree and record it as the base: `base_commit` is the
/// server's head and each file's hash is of Ed's bytes. A change saved here
/// before this (`Unrecorded`) is kept over Ed's copy and then sent to Ed.
fn first_connect(
    core: &Context,
    workspace_id: &str,
    host: &str,
    init: &Init,
) -> Result<Vec<String>, String> {
    let ticket = core.ed.download_ticket(workspace_id, ".")?;
    let zip = sahara::download(&core.origin(host), &ticket)?;
    let (files, skipped) = unpack_workspace(&zip)?;
    for line in skipped {
        elog!("[oculus] ed workspace {}: {line}", core.challenge_id);
    }
    let unrecorded = std::mem::take(&mut core.shared().unrecorded);
    let root = core.root();
    std::fs::create_dir_all(&root)
        .map_err(|e| format!("could not make the workspace folder: {e}"))?;

    let mut record = BTreeMap::new();
    for file in &files {
        let target = match paths::resolve(&root, &file.path) {
            Ok(target) => target,
            Err(e) => {
                elog!("[oculus] ed workspace {}: {e}, skipped", core.challenge_id);
                continue;
            }
        };
        record.insert(file.path.clone(), sha256(&file.bytes));
        let same = std::fs::read(&target).is_ok_and(|b| b == file.bytes);
        if unrecorded.contains_key(&file.path) || same {
            continue;
        }
        write_file(&target, &file.bytes)?;
    }
    core.record(workspace_id, |state| {
        *state = WorkspaceState {
            workspace_id: workspace_id.to_string(),
            slide_dir: core.slide_dir.clone(),
            base_commit: init.head().map(str::to_string),
            files: record,
            submissions: std::mem::take(&mut state.submissions),
        }
    })?;
    core.emit_files();
    send_unrecorded(core, workspace_id, host, &unrecorded)
}

/// Upload what was saved before the record existed and remove what was
/// deleted, recording each as it goes. A failed upload leaves its file
/// differing from the record, as any offline save does.
fn send_unrecorded(
    core: &Context,
    workspace_id: &str,
    host: &str,
    unrecorded: &BTreeMap<String, Unrecorded>,
) -> Result<Vec<String>, String> {
    let root = core.root();
    let mut frames_out = Vec::new();
    for (path, change) in unrecorded {
        match change {
            Unrecorded::Saved => {
                let Ok(bytes) = paths::resolve(&root, path)
                    .and_then(|t| std::fs::read(t).map_err(|e| e.to_string()))
                else {
                    continue;
                };
                let hash = core.upload(workspace_id, host, path, &bytes)?;
                core.record(workspace_id, |state| {
                    state.files.insert(path.clone(), sha256(&bytes));
                    state.base_commit = Some(hash);
                })?;
            }
            Unrecorded::Deleted => {
                frames_out.push(frames::remove(path));
                core.record(workspace_id, |state| forget(state, path))?;
            }
        }
    }
    Ok(frames_out)
}
