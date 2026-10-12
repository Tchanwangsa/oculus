//! The snapshots `studio_core::write_library` keeps in `<library>/.history/`
//! (`<Name>.<unix-ms>.tsx`), listed for Undo and written back by restore.

use std::path::Path;

use serde::Serialize;
use studio_core::Library;

use super::super::levels::{root_of, Level};
use super::source::ComponentText;
use super::{name, write};

/// Mirrors `Snapshot` in `app/src/lib/studio/components.ts`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// The snapshot's Unix milliseconds, as a string: what restore takes.
    pub id: String,
    /// The same moment, Unix milliseconds: when the text was replaced.
    pub saved_at: u64,
}

/// The snapshots of `name` at `level`, newest first.
pub fn list_in(
    library: &Library,
    level: Level,
    subject: Option<&str>,
    name: &str,
) -> Result<Vec<Snapshot>, String> {
    name::check(name)?;
    let root = root_of(library, level, subject)?;
    let mut snapshots: Vec<Snapshot> = std::fs::read_dir(root.join(".history"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name().into_string().ok()?;
            let ms = file
                .strip_prefix(name)?
                .strip_prefix('.')?
                .strip_suffix(".tsx")?;
            let saved_at: u64 = ms.parse().ok()?;
            Some(Snapshot {
                id: saved_at.to_string(),
                saved_at,
            })
        })
        .collect();
    snapshots.sort_by_key(|s| std::cmp::Reverse(s.saved_at));
    Ok(snapshots)
}

/// Writes snapshot `id` of `name` back through `write::write_in`: it must
/// still load, and the text it replaces becomes the newest snapshot.
pub fn restore_in(
    library: &Library,
    level: Level,
    subject: Option<&str>,
    name: &str,
    id: &str,
) -> Result<ComponentText, String> {
    let known = list_in(library, level, subject, name)?;
    let snapshot = known.iter().find(|s| s.id == id).ok_or_else(|| {
        format!(
            "{name} has no snapshot `{id}` at the {} level",
            level.name()
        )
    })?;
    let root = root_of(library, level, subject)?;
    let path = root
        .join(".history")
        .join(format!("{name}.{}.tsx", snapshot.saved_at));
    let text = read(&path)?;
    write::write_in(library, level, subject, name, &text)
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}
