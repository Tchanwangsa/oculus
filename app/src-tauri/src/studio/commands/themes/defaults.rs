//! Each library's default theme, `library.json`'s `defaultTheme`, and the
//! one a new video in a subject would take (`libraries::default_theme`).

use std::path::Path;

use serde::Serialize;
use studio_core::{Library, LibrarySettings};

use super::super::levels::{root_of, roots, Level};
use super::resolve::check_name;
use crate::studio::libraries::{default_theme, theme_defined};

/// Mirrors `ThemeDefaults` in `app/src/lib/studio/themes.ts`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ThemeDefaults {
    /// The global library's `defaultTheme`, as written.
    pub global: Option<String>,
    /// The subject library's, when a subject is asked about.
    pub subject: Option<String>,
    /// The theme a new video there takes.
    pub effective: String,
}

pub fn defaults_in(library: &Library, subject: Option<&str>) -> Result<ThemeDefaults, String> {
    let read = |root: &Path| {
        LibrarySettings::read(root)
            .map(|s| s.default_theme)
            .map_err(|e| e.to_string())
    };
    let global = read(&root_of(library, Level::Global, subject)?)?;
    let subject_default = match subject {
        Some(_) => read(&root_of(library, Level::Subject, subject)?)?,
        None => None,
    };
    Ok(ThemeDefaults {
        global,
        subject: subject_default,
        effective: default_theme(library, subject)?,
    })
}

/// Sets (or with None clears) the default theme of the library at `level`.
/// The name must be one that library's cascade defines: its own `themes/`,
/// a farther library's, or a bundled theme.
pub fn set_in(
    library: &Library,
    level: Level,
    subject: Option<&str>,
    name: Option<&str>,
) -> Result<ThemeDefaults, String> {
    let root = root_of(library, level, subject)?;
    if let Some(name) = name {
        check_name(name)?;
        let reach: Vec<_> = roots(library, subject)?
            .into_iter()
            .filter(|(l, _)| level == Level::Subject || *l == Level::Global)
            .map(|(_, root)| root)
            .collect();
        let reach: Vec<&Path> = reach.iter().map(|p| p.as_path()).collect();
        if !theme_defined(&reach, name) {
            return Err(format!(
                "no theme `{name}` in the {} library's reach to make its default",
                level.name()
            ));
        }
    }
    let mut settings = LibrarySettings::read(&root).map_err(|e| e.to_string())?;
    settings.default_theme = name.map(str::to_owned);
    settings.write(&root).map_err(|e| e.to_string())?;
    defaults_in(library, subject)
}
