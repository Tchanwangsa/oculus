//! The theme list: a row per name, with the colours the style chip paints.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;
use studio_core::{Library, Theme};

use super::super::levels::{roots, Level};
use super::resolve::{locate, names_in, ThemeLevel};

/// The colour tokens a swatch shows, in order: the canvas, the text, the
/// accent and the card surface.
const SWATCH: [&str; 4] = ["bg", "text", "accent", "surface"];

/// Mirrors `ThemeInfo` in `app/src/lib/studio/themes.ts`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeRow {
    pub name: String,
    pub level: ThemeLevel,
    /// Up to four CSS colours from the theme's `color` tokens; empty when the
    /// theme does not parse.
    pub swatch: Vec<String>,
    pub tokens: Option<Value>,
    pub problem: Option<String>,
    /// The farther level a library file replaces under the same name: a
    /// subject's "own version" of a global or bundled theme.
    pub shadows: Option<ThemeLevel>,
}

/// Every theme name in effect for `subject`, `default` first.
pub fn list_in(library: &Library, subject: Option<&str>) -> Result<Vec<ThemeRow>, String> {
    let roots = roots(library, subject)?;
    let mut names: BTreeSet<String> = Theme::bundled_names()
        .into_iter()
        .map(str::to_owned)
        .collect();
    for (_, root) in &roots {
        names.extend(names_in(root));
    }
    let mut rows = Vec::with_capacity(names.len());
    for name in names {
        let found = locate(&roots, &name)?;
        let shadows = shadowed(&roots, &name, found.level)?;
        let row = match found.theme(&name) {
            Ok(theme) => ThemeRow {
                swatch: swatch(&theme),
                level: found.level,
                name,
                tokens: Some(theme.tokens),
                problem: None,
                shadows,
            },
            Err(problem) => ThemeRow {
                level: found.level,
                name,
                swatch: Vec::new(),
                tokens: None,
                problem: Some(problem),
                shadows,
            },
        };
        rows.push(row);
    }
    rows.sort_by(|a, b| {
        (a.name != "default")
            .cmp(&(b.name != "default"))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(rows)
}

/// What `name` at `level` replaces: the level the cascade would find without
/// it.
fn shadowed(
    roots: &[(Level, PathBuf)],
    name: &str,
    level: ThemeLevel,
) -> Result<Option<ThemeLevel>, String> {
    let farther: Vec<(Level, PathBuf)> = match level {
        ThemeLevel::Bundled => return Ok(None),
        ThemeLevel::Subject => roots
            .iter()
            .filter(|(l, _)| *l == Level::Global)
            .cloned()
            .collect(),
        ThemeLevel::Global => Vec::new(),
    };
    let below = locate(&farther, name)?;
    Ok(match below.level {
        ThemeLevel::Bundled if !Theme::bundled_names().contains(&name) => None,
        other => Some(other),
    })
}

fn swatch(theme: &Theme) -> Vec<String> {
    SWATCH
        .iter()
        .filter_map(|key| theme.tokens.pointer(&format!("/color/{key}"))?.as_str())
        .map(str::to_owned)
        .collect()
}
