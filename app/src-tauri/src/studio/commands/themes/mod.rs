//! The themes a video can use, for the style chip, the Components playground
//! and the Themes page (docs/studio.md, "The playground's commands"): the
//! list, a theme's text, validated writes with a history to undo them from,
//! removing a library theme, and each library's default theme
//! (`library.json`). `resolve` is the cascade the playground's stage renders
//! through.

mod defaults;
mod history;
mod resolve;
mod rows;
mod source;
#[cfg(test)]
mod tests;
mod write;

use super::history::Snapshot;
use super::levels::Level;
use crate::runtime::blocking;
use crate::studio::libraries::libraries;
use defaults::ThemeDefaults;
use rows::ThemeRow;
use source::ThemeText;

pub(super) use resolve::resolve;
pub(super) use write::check_size;

/// One row per theme name: the bundled names and the library themes, the
/// subject's over the global one over the bundled one. A theme that does not
/// parse is a row with `problem`, not a failed call.
#[tauri::command]
pub async fn studio_themes(subject: Option<String>) -> Result<Vec<ThemeRow>, String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        rows::list_in(&library, subject.as_deref())
    })
    .await
}

/// The text of a theme from the level that wins (a bundled theme's text as
/// compiled in).
#[tauri::command]
pub async fn studio_theme_source(
    name: String,
    subject: Option<String>,
) -> Result<ThemeText, String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        source::source_in(&library, subject.as_deref(), &name)
    })
    .await
}

/// Writes the theme at `level` once it parses; the text it replaces goes to
/// `.history` first. A bundled theme's name is refused.
#[tauri::command]
pub async fn studio_theme_write(
    name: String,
    level: Level,
    subject: Option<String>,
    text: String,
) -> Result<ThemeText, String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        write::write_in(&library, level, subject.as_deref(), &name, &text)
    })
    .await
}

/// Removes a library theme at `level`, snapshotting it first so restore can
/// bring it back.
#[tauri::command]
pub async fn studio_theme_delete(
    name: String,
    level: Level,
    subject: Option<String>,
) -> Result<(), String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        write::delete_in(&library, level, subject.as_deref(), &name)
    })
    .await
}

/// The snapshots of the theme at `level`, newest first.
#[tauri::command]
pub async fn studio_theme_history(
    name: String,
    level: Level,
    subject: Option<String>,
) -> Result<Vec<Snapshot>, String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        history::list_in(&library, level, subject.as_deref(), &name)
    })
    .await
}

/// Writes snapshot `id` back, snapshotting the current text first.
#[tauri::command]
pub async fn studio_theme_restore(
    name: String,
    level: Level,
    subject: Option<String>,
    id: String,
) -> Result<ThemeText, String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        history::restore_in(&library, level, subject.as_deref(), &name, &id)
    })
    .await
}

/// The global and the subject library's default themes, and the one a new
/// video in that subject takes.
#[tauri::command]
pub async fn studio_theme_defaults(subject: Option<String>) -> Result<ThemeDefaults, String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        defaults::defaults_in(&library, subject.as_deref())
    })
    .await
}

/// Sets the default theme of the library at `level`, or clears it with no
/// `name`. Returns the defaults as `studio_theme_defaults` would.
#[tauri::command]
pub async fn studio_theme_set_default(
    level: Level,
    subject: Option<String>,
    name: Option<String>,
) -> Result<ThemeDefaults, String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        defaults::set_in(&library, level, subject.as_deref(), name.as_deref())
    })
    .await
}
