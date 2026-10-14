//! What a connect does before the session goes online. A challenge with no
//! sync record takes Ed's tree as it is now and records it. One with a
//! record holds local work, which `reconcile` brings in step with Ed
//! (`docs/sync.md`).

use std::collections::{BTreeMap, BTreeSet};

use super::context::{sha256, write_file, Context, Unrecorded};
use super::frames::{self, Init};
use super::paths;
use super::reconcile::{self, Head, Removes};
use crate::library::paths::safe_rel_path;
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
    reconcile::wait_for_sync(core.challenge_id);
    let _turn = core.queue.place().wait();
    if core.has_record() {
        return reconcile_on_connect(core, workspace_id, host, init);
    }
    first_connect(core, workspace_id, host, init)
}

/// Reconcile, downloading Ed's tree unless `init` says Ed is still at the
/// base commit and no pull of another client's commit is owed.
fn reconcile_on_connect(
    core: &Context,
    workspace_id: &str,
    host: &str,
    init: &Init,
) -> Result<Vec<String>, String> {
    let owed = core.puller.take_owed();
    let head = Head {
        hash: init.head(),
        changed: owed,
    };
    let mut removes = Vec::new();
    let done = reconcile::reconcile(
        core,
        workspace_id,
        host,
        head,
        Removes::Collect(&mut removes),
    );
    if let Err(e) = done {
        if owed {
            core.puller.owe();
        }
        return Err(e);
    }
    Ok(removes)
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
    let mut unknown = BTreeSet::new();
    for skip in skipped {
        elog!("[oculus] ed workspace {}: {skip}", core.challenge_id);
        unknown.insert(skip.path);
    }
    let mut unrecorded = std::mem::take(&mut core.shared().unrecorded);
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
    // A file Ed lacks is new here and goes up, except an earlier sync's
    // renamed copy of one of Ed's (`my_file.py` for `my file.py`).
    let renamed = old_sync_names(files.iter().map(|f| f.path.as_str()));
    for path in paths::list(&root) {
        let known = record.contains_key(&path)
            || unrecorded.contains_key(&path)
            || unknown.contains(&path)
            || reconcile::is_server_copy(&path);
        if known {
            continue;
        }
        if renamed.contains(&path) {
            elog!(
                "[oculus] ed workspace {}: {path} is an earlier sync's name for a file on Ed, deleted",
                core.challenge_id
            );
            let _ = std::fs::remove_file(root.join(&path));
        } else {
            unrecorded.insert(path, Unrecorded::Saved);
        }
    }
    core.record(workspace_id, |state| {
        *state = WorkspaceState {
            workspace_id: workspace_id.to_string(),
            slide_dir: core.slide_dir.clone(),
            base_commit: init.head().map(str::to_string),
            files: record,
        }
    })?;
    core.emit_files();
    send_unrecorded(core, workspace_id, host, &unrecorded)
}

/// The names the lesson sync gave Ed's `paths` before it kept Ed's own
/// (`safe_rel_path`), where they differ.
pub(super) fn old_sync_names<'a>(paths: impl Iterator<Item = &'a str>) -> BTreeSet<String> {
    paths
        .filter_map(|p| safe_rel_path(p).filter(|old| old != p))
        .collect()
}

/// Upload what was saved before the record existed and remove what was
/// deleted. An upload is recorded as it goes; a removed file stays in the
/// record until a reconcile finds it gone from Ed too. A failed upload
/// leaves its file differing from the record, as any offline save does.
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
                frames_out.push(frames::remove(path, core.recorded_folder(path)))
            }
        }
    }
    Ok(frames_out)
}
