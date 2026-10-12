//! A library theme's snapshots (`super::super::history`), listed for Undo and
//! written back by restore.

use studio_core::Library;

use super::super::history::{self, Snapshot};
use super::super::levels::{root_of, Level};
use super::resolve::check_name;
use super::source::ThemeText;
use super::write;

/// The snapshots of theme `name` at `level`, newest first.
pub fn list_in(
    library: &Library,
    level: Level,
    subject: Option<&str>,
    name: &str,
) -> Result<Vec<Snapshot>, String> {
    check_name(name)?;
    let root = root_of(library, level, subject)?;
    Ok(history::list(&root, name, "json"))
}

/// Writes snapshot `id` back through `write::write_in`: it must still parse,
/// and the text it replaces becomes the newest snapshot. A removed theme
/// comes back this way too.
pub fn restore_in(
    library: &Library,
    level: Level,
    subject: Option<&str>,
    name: &str,
    id: &str,
) -> Result<ThemeText, String> {
    check_name(name)?;
    let root = root_of(library, level, subject)?;
    let text = history::read(&root, level, name, "json", id)?;
    write::write_in(library, level, subject, name, &text)
}
