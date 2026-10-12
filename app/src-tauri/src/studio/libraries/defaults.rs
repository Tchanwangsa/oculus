//! The theme a new video takes when it names none: its subject library's
//! `library.json` `defaultTheme`, else the global library's, else the
//! bundled `default`. A default naming a theme its cascade no longer has is
//! passed over, so deleting a theme never stops a video being made.

use std::path::Path;

use studio_core::{valid_subject, Library, LibrarySettings, Theme};

/// `default` when no library names one.
pub const FALLBACK_THEME: &str = "default";

/// The default theme for a video in `subject`'s course, or the global one.
/// A `library.json` that does not parse is an error naming it.
pub fn default_theme(library: &Library, subject: Option<&str>) -> Result<String, String> {
    let subject_root = match subject {
        Some(code) if !valid_subject(code) => {
            return Err(format!(
                "`{code}` is not a subject: a course code's folder name, in letters, digits, `-` and `_`"
            ))
        }
        Some(code) => library.subjects.as_ref().map(|dir| dir.join(code)),
        None => None,
    };
    let global_root = library.global.as_deref();
    let cascade: Vec<&Path> = [subject_root.as_deref(), global_root]
        .into_iter()
        .flatten()
        .collect();
    for (i, root) in cascade.iter().enumerate() {
        let settings = LibrarySettings::read(root).map_err(|e| e.to_string())?;
        if let Some(name) = settings.default_theme {
            if defined(&cascade[i..], &name) {
                return Ok(name);
            }
        }
    }
    Ok(FALLBACK_THEME.to_owned())
}

/// `name` is a theme file in one of `roots` or a bundled theme.
pub fn defined(roots: &[&Path], name: &str) -> bool {
    Theme::bundled_names().contains(&name)
        || roots
            .iter()
            .any(|root| root.join("themes").join(format!("{name}.json")).is_file())
}
