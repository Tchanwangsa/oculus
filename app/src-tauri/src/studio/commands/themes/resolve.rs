//! A theme name through the cascade: the subject's `themes/<name>.json`,
//! then the global one, then the bundled set (as `Project::load_with` does).

use std::path::{Path, PathBuf};

use serde::Serialize;
use studio_core::{valid_theme_name, Library, Theme};

use super::super::levels::{roots, Level};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeLevel {
    Bundled,
    Global,
    Subject,
}

/// Where a name is defined: a library file, or the bundled set (`path` is
/// then None). A name nothing defines is `Bundled` too, and `theme` says so.
pub struct Found {
    pub level: ThemeLevel,
    pub path: Option<PathBuf>,
}

/// A library theme's name is its file stem (`studio_core::valid_theme_name`),
/// which also keeps it inside `themes/`.
pub fn check_name(name: &str) -> Result<(), String> {
    if valid_theme_name(name) {
        Ok(())
    } else {
        Err(format!(
            "`{name}` is not a theme name: lowercase letters, digits and `-`"
        ))
    }
}

pub fn locate(roots: &[(Level, PathBuf)], name: &str) -> Result<Found, String> {
    check_name(name)?;
    for (level, root) in roots {
        let path = root.join("themes").join(format!("{name}.json"));
        if path
            .try_exists()
            .map_err(|e| format!("cannot look for {}: {e}", path.display()))?
        {
            let level = match level {
                Level::Subject => ThemeLevel::Subject,
                Level::Global => ThemeLevel::Global,
            };
            return Ok(Found {
                level,
                path: Some(path),
            });
        }
    }
    Ok(Found {
        level: ThemeLevel::Bundled,
        path: None,
    })
}

impl Found {
    pub fn theme(&self, name: &str) -> Result<Theme, String> {
        match &self.path {
            Some(path) => Theme::from_file(path).map_err(|e| e.to_string()),
            None => Theme::bundled(name).map_err(|e| e.to_string()),
        }
    }
}

/// The theme `name` as a video in `subject`'s course would load it.
pub fn resolve(library: &Library, subject: Option<&str>, name: &str) -> Result<Theme, String> {
    let roots = roots(library, subject)?;
    locate(&roots, name)?.theme(name)
}

/// The theme names a library folder defines.
pub fn names_in(root: &Path) -> Vec<String> {
    std::fs::read_dir(root.join("themes"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name().into_string().ok()?;
            let stem = file.strip_suffix(".json")?;
            valid_theme_name(stem).then(|| stem.to_owned())
        })
        .collect()
}
