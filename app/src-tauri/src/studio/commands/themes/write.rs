//! A theme written to or removed from a library, once its text parses.

use std::path::Path;

use studio_core::{remove_library_file, write_library, Library, Theme};

use super::super::levels::{root_of, Level};
use super::resolve::{check_name, ThemeLevel};
use super::source::ThemeText;

/// The largest theme file; a mistake past it, not a theme.
const MAX_BYTES: usize = 256 * 1024;

pub fn rel(name: &str) -> String {
    format!("themes/{name}.json")
}

pub fn check_size(name: &str, text: &str) -> Result<(), String> {
    if text.len() > MAX_BYTES {
        return Err(format!(
            "{name}.json is {} KB; a theme is at most {} KB",
            text.len() / 1024,
            MAX_BYTES / 1024
        ));
    }
    Ok(())
}

/// A name a library may hold: valid, and not a bundled theme's, which a
/// library file would shadow for every video (make a copy under a new name).
fn check_writable(name: &str) -> Result<(), String> {
    check_name(name)?;
    if Theme::bundled_names().contains(&name) {
        return Err(format!(
            "`{name}` is a bundled theme and cannot be replaced; save a copy under another name"
        ));
    }
    Ok(())
}

/// Writes `text` as theme `name` at `level` once it parses as a theme
/// (`Theme::parse`, the check every load makes). The text it replaces goes to
/// `.history` (`studio_core::write_library`); the same text again writes
/// nothing.
pub fn write_in(
    library: &Library,
    level: Level,
    subject: Option<&str>,
    name: &str,
    text: &str,
) -> Result<ThemeText, String> {
    check_writable(name)?;
    let root = root_of(library, level, subject)?;
    check_size(name, text)?;
    Theme::parse(text, Path::new(&rel(name))).map_err(|e| e.to_string())?;
    let path = root.join(rel(name));
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text) {
        write_library(&root, &rel(name), text).map_err(|e| e.to_string())?;
    }
    Ok(ThemeText {
        level: theme_level(level),
        text: text.to_owned(),
    })
}

/// Removes theme `name` from the library at `level`, snapshotting it first.
pub fn delete_in(
    library: &Library,
    level: Level,
    subject: Option<&str>,
    name: &str,
) -> Result<(), String> {
    check_writable(name)?;
    let root = root_of(library, level, subject)?;
    remove_library_file(&root, &rel(name)).map_err(|e| e.to_string())
}

pub fn theme_level(level: Level) -> ThemeLevel {
    match level {
        Level::Subject => ThemeLevel::Subject,
        Level::Global => ThemeLevel::Global,
    }
}
