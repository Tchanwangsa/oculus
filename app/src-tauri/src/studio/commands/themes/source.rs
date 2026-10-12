//! A theme's file text, from the level that wins: what the Themes page edits.

use serde::Serialize;
use studio_core::{Library, Theme};

use super::super::levels::roots;
use super::resolve::{locate, ThemeLevel};

/// Mirrors `ThemeText` in `app/src/lib/studio/themes.ts`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ThemeText {
    pub level: ThemeLevel,
    pub text: String,
}

/// The text of theme `name` as `subject`'s videos resolve it: the library
/// file, or the bundled text as compiled in.
pub fn source_in(
    library: &Library,
    subject: Option<&str>,
    name: &str,
) -> Result<ThemeText, String> {
    let found = locate(&roots(library, subject)?, name)?;
    let text = match &found.path {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?,
        None => Theme::bundled_text(name)
            .ok_or_else(|| format!("no theme `{name}` in the libraries or the bundled set"))?
            .to_owned(),
    };
    Ok(ThemeText {
        level: found.level,
        text,
    })
}
