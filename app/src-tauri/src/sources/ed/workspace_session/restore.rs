//! Restore a submission: the challenge's files become the code submitted
//! (`mark::code`), here and on Ed. The library copy is replaced first, then
//! each file Ed does not hold already is uploaded and each recorded file the
//! submission lacks is removed on Ed. Only `ed_workspace_restore`, after the
//! pane's confirm dialog, calls it.

use std::collections::BTreeSet;

use super::context::{sha256, write_file, Context};
use super::events::Body;
use super::frames;
use super::mark::code;
use super::paths;
use super::queue::Place;
use super::reconcile;
use super::session::Session;
use super::socket::Out;
use crate::sources::ed::workspace_state;
use crate::sources::ed::WorkspaceFile;

impl Session {
    /// Replace the workspace with submission `submission_id`'s files, once
    /// the saves asked before it have landed; no save lands in between.
    pub fn restore(&self, place: Place, submission_id: i64) -> Result<(), String> {
        let _turn = place.wait();
        let core = &self.core;
        let (workspace_id, host) = core
            .online()
            .ok_or("The workspace is offline: restoring needs Ed's server")?;
        let files = code::files(&core.ed, submission_id)?;
        let kept = replace_here(core, &files)?;
        core.emit_files();
        let sent = send_to_ed(core, &self.out, &workspace_id, &host, &files, &kept);
        core.emit(Body::Reset);
        sent.map_err(|e| {
            format!(
                "Your files here are the submission's, but Ed did not take them all ({e}); \
                 the next connect sends the rest"
            )
        })
    }
}

/// Make the folder hold `files` and nothing else, conflict copies included.
/// Returns the paths written; one the guard refuses is logged and left out.
fn replace_here(core: &Context, files: &[WorkspaceFile]) -> Result<BTreeSet<String>, String> {
    let root = core.root();
    let mut kept = BTreeSet::new();
    for file in files {
        if reconcile::is_server_copy(&file.path) {
            elog!(
                "[oculus] ed workspace {}: {} is named like a conflict copy, not restored",
                core.challenge_id,
                file.path
            );
            continue;
        }
        let target = match paths::resolve(&root, &file.path) {
            Ok(target) => target,
            Err(e) => {
                elog!(
                    "[oculus] ed workspace {}: {e}, not restored",
                    core.challenge_id
                );
                continue;
            }
        };
        if std::fs::read(&target).ok().as_deref() != Some(file.bytes.as_slice()) {
            write_file(&target, &file.bytes)?;
        }
        kept.insert(file.path.clone());
    }
    for path in paths::list(&root) {
        if kept.contains(&path) {
            continue;
        }
        match std::fs::remove_file(paths::resolve(&root, &path)?) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("could not delete {path}: {e}")),
        }
    }
    Ok(kept)
}

/// Upload what the record says Ed lacks, recording each as it lands, and
/// remove on Ed the recorded files the submission lacks. The record keeps
/// those until a reconcile finds them gone, so a lost remove is sent again.
fn send_to_ed(
    core: &Context,
    out: &std::sync::mpsc::Sender<Out>,
    workspace_id: &str,
    host: &str,
    files: &[WorkspaceFile],
    kept: &BTreeSet<String>,
) -> Result<(), String> {
    let record = workspace_state::load(&core.data_dir, core.challenge_id).unwrap_or_default();
    for file in files.iter().filter(|f| kept.contains(&f.path)) {
        let sha = sha256(&file.bytes);
        if record.files.get(&file.path) == Some(&sha) {
            continue;
        }
        let hash = core.upload(workspace_id, host, &file.path, &file.bytes)?;
        core.record(workspace_id, |state| {
            state.files.insert(file.path.clone(), sha);
            state.base_commit = Some(hash);
        })?;
    }
    for path in record.files.keys().filter(|p| !kept.contains(*p)) {
        out.send(Out::Frame(frames::remove(path, false)))
            .map_err(|_| "the workspace session has ended".to_string())?;
    }
    Ok(())
}
