//! What Oculus last synced of a challenge workspace, one file per challenge
//! at `<data_dir>/ed-workspaces/<challenge_id>.json`, outside the library so
//! no migration and no library scan ever touches it. While a challenge has
//! one, the lesson sync leaves its files alone (`docs/sync.md`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One challenge's sync record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceState {
    /// Ed's workspace id, from `POST /challenges/<ch>/connect`.
    pub workspace_id: String,
    /// The slide's workspace folder, data-dir-relative:
    /// `courses/<CODE>/lessons/<id>-<slug>/<NN>-<slide-slug>`.
    pub slide_dir: String,
    /// The commit the files were last in step with Ed at.
    pub base_commit: Option<String>,
    /// Each file's path inside the workspace → the SHA-256 (lower-case hex)
    /// of the bytes last synced.
    pub files: BTreeMap<String, String>,
    /// The submissions this install made: Ed lists none for a student.
    pub submissions: Vec<i64>,
}

/// Where a challenge's record lives.
pub fn path(data_dir: &Path, challenge_id: i64) -> PathBuf {
    data_dir
        .join("ed-workspaces")
        .join(format!("{challenge_id}.json"))
}

/// The challenge's record, `None` when it has none or it cannot be read
/// (logged). Whether the file exists is the question [`has_state`] answers.
pub fn load(data_dir: &Path, challenge_id: i64) -> Option<WorkspaceState> {
    let target = path(data_dir, challenge_id);
    let raw = std::fs::read(&target).ok()?;
    match serde_json::from_slice(&raw) {
        Ok(state) => Some(state),
        Err(e) => {
            elog!("[oculus] {} is unreadable: {e}", target.display());
            None
        }
    }
}

/// Whether the challenge has a record at all, readable or not: either way
/// the workspace holds local work a sync must not overwrite.
pub fn has_state(data_dir: &Path, challenge_id: i64) -> bool {
    path(data_dir, challenge_id).is_file()
}

pub fn save(data_dir: &Path, challenge_id: i64, state: &WorkspaceState) -> Result<(), String> {
    crate::runtime::atomic_write::json(&path(data_dir, challenge_id), state)
}
