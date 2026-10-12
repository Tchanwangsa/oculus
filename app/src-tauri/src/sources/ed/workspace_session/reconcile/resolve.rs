//! Settling a conflict from the pane: keep the local file or take Ed's
//! copy, then delete the copy. Works offline too: the record is set so that
//! the next reconcile does the rest, and calls it a conflict again if Ed
//! changed meanwhile.

use serde::Deserialize;

use super::server_copy;
use crate::sources::ed::workspace_session::context::{sha256, write_file, Context};
use crate::sources::ed::workspace_session::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Keep {
    Mine,
    Ed,
}

/// The caller holds the session's queue turn.
pub fn resolve(core: &Context, path: &str, keep: Keep) -> Result<(), String> {
    let root = core.root();
    let target = paths::resolve(&root, path)?;
    let copy = paths::resolve(&root, &server_copy::name(path))?;
    let theirs =
        std::fs::read(&copy).map_err(|_| format!("there is no copy of Ed's {path} to settle"))?;
    let online = core.online();
    let workspace_id = online.as_ref().map(|(w, _)| w.as_str()).unwrap_or("");
    let remove_copy =
        || std::fs::remove_file(&copy).map_err(|e| format!("could not delete Ed's copy: {e}"));
    match keep {
        Keep::Ed => {
            write_file(&target, &theirs).map_err(|e| format!("could not save {path}: {e}"))?;
            // Unrecorded, it is recorded once Ed is found to agree.
            core.record(workspace_id, |state| {
                state.files.remove(path);
            })?;
            remove_copy()?;
            core.emit_files();
        }
        Keep::Mine => {
            let mine = std::fs::read(&target).map_err(|e| format!("could not read {path}: {e}"))?;
            // Ed's version as the base leaves the local edit the only change.
            core.record(workspace_id, |state| {
                state.files.insert(path.to_string(), sha256(&theirs));
            })?;
            remove_copy()?;
            core.emit_files();
            if let Some((workspace_id, host)) = online {
                let hash = core
                    .upload(&workspace_id, &host, path, &mine)
                    .map_err(|e| format!("{path} is kept here, but Ed did not take it: {e}"))?;
                core.record(&workspace_id, |state| {
                    state.files.insert(path.to_string(), sha256(&mine));
                    state.base_commit = Some(hash);
                })?;
            }
        }
    }
    Ok(())
}
