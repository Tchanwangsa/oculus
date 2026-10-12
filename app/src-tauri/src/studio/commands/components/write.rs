//! A component written to a library, once it loads.

use studio_core::{remove_library_file, write_library, Library};

use super::super::levels::{root_of, Level};
use super::source::ComponentText;
use super::{engine, name};

/// The largest component source; a mistake past it, not a component.
pub const MAX_BYTES: usize = 512 * 1024;

/// Loads `text` as component `name`: compiled, its `props` and `examples`
/// read. The error is the engine's, with the line in the TSX.
pub fn check(name: &str, text: &str) -> Result<(), String> {
    if text.len() > MAX_BYTES {
        return Err(format!(
            "{name}.tsx is {} KB; a component is at most {} KB",
            text.len() / 1024,
            MAX_BYTES / 1024
        ));
    }
    let mut loader = engine::loader()?;
    loader
        .load_component(name, text)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Writes `text` as `name` at `level` after `check`, so a file that does not
/// load is refused and nothing changes. The text it replaces goes to
/// `.history` (`studio_core::write_library`); the same text again writes
/// nothing.
pub fn write_in(
    library: &Library,
    level: Level,
    subject: Option<&str>,
    name: &str,
    text: &str,
) -> Result<ComponentText, String> {
    name::check(name)?;
    let root = root_of(library, level, subject)?;
    check(name, text)?;
    let path = root.join(name::rel(name));
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text) {
        write_library(&root, &name::rel(name), text).map_err(|e| e.to_string())?;
    }
    Ok(ComponentText {
        level,
        text: text.to_owned(),
    })
}

/// Removes `name` from `subject`'s library, snapshotting it first. Only a
/// subject's file is removed: the global library is the floor every subject
/// falls back to.
pub fn delete_in(library: &Library, subject: &str, name: &str) -> Result<(), String> {
    name::check(name)?;
    let root = root_of(library, Level::Subject, Some(subject))?;
    remove_library_file(&root, &name::rel(name)).map_err(|e| e.to_string())
}
