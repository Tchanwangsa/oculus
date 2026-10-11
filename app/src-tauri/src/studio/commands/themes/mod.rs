//! The themes a video can use, for the style chip and the Components
//! playground (docs/studio.md, "The playground's commands"). `resolve` is
//! the cascade the playground's stage renders through.

mod resolve;
mod rows;
#[cfg(test)]
mod tests;

use crate::runtime::blocking;
use crate::studio::libraries::libraries;
use rows::ThemeRow;

pub(super) use resolve::resolve;

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
