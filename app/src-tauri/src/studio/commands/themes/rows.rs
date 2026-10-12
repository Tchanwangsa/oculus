//! The theme list: a row per name, with the colours the style chip paints.

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;
use studio_core::{Library, Theme};

use super::super::levels::roots;
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
        let row = match found.theme(&name) {
            Ok(theme) => ThemeRow {
                swatch: swatch(&theme),
                level: found.level,
                name,
                tokens: Some(theme.tokens),
                problem: None,
            },
            Err(problem) => ThemeRow {
                level: found.level,
                name,
                swatch: Vec::new(),
                tokens: None,
                problem: Some(problem),
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

fn swatch(theme: &Theme) -> Vec<String> {
    SWATCH
        .iter()
        .filter_map(|key| theme.tokens.pointer(&format!("/color/{key}"))?.as_str())
        .map(str::to_owned)
        .collect()
}
