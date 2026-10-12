//! The snapshots `studio_core::write_library` keeps in `<library>/.history/`
//! (`<stem>.<unix-ms>.<ext>`), for any library file: a component is
//! `(<Name>, tsx)`, a theme `(<name>, json)`. Listed for Undo and found again
//! by id for restore.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::levels::Level;

/// Mirrors `Snapshot` in `app/src/lib/studio/components.ts`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// The snapshot's Unix milliseconds, as a string: what restore takes.
    pub id: String,
    /// The same moment, Unix milliseconds: when the text was replaced.
    pub saved_at: u64,
}

/// The snapshots of `<stem>.<ext>` in the library at `root`, newest first.
pub fn list(root: &Path, stem: &str, ext: &str) -> Vec<Snapshot> {
    let suffix = format!(".{ext}");
    let mut snapshots: Vec<Snapshot> = std::fs::read_dir(root.join(".history"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name().into_string().ok()?;
            let ms = file
                .strip_prefix(stem)?
                .strip_prefix('.')?
                .strip_suffix(&suffix)?;
            let saved_at: u64 = ms.parse().ok()?;
            Some(Snapshot {
                id: saved_at.to_string(),
                saved_at,
            })
        })
        .collect();
    snapshots.sort_by_key(|s| std::cmp::Reverse(s.saved_at));
    snapshots
}

/// The text of snapshot `id` of `<stem>.<ext>`, one `list` gives.
pub fn read(root: &Path, level: Level, stem: &str, ext: &str, id: &str) -> Result<String, String> {
    let snapshot = list(root, stem, ext)
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| {
            format!(
                "{stem} has no snapshot `{id}` at the {} level",
                level.name()
            )
        })?;
    let path: PathBuf = root
        .join(".history")
        .join(format!("{stem}.{}.{ext}", snapshot.saved_at));
    std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}
