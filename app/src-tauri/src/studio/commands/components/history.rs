//! A component's snapshots (`super::super::history`), listed for Undo and
//! written back by restore.

use studio_core::Library;

use super::super::history::{self, Snapshot};
use super::super::levels::{root_of, Level};
use super::source::ComponentText;
use super::{name, write};

/// The snapshots of `name` at `level`, newest first.
pub fn list_in(
    library: &Library,
    level: Level,
    subject: Option<&str>,
    name: &str,
) -> Result<Vec<Snapshot>, String> {
    name::check(name)?;
    let root = root_of(library, level, subject)?;
    Ok(history::list(&root, name, "tsx"))
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
    name::check(name)?;
    let root = root_of(library, level, subject)?;
    let text = history::read(&root, level, name, "tsx", id)?;
    write::write_in(library, level, subject, name, &text)
}
