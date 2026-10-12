//! A component's source, from the nearest library that has it.

use std::path::PathBuf;

use serde::Serialize;
use studio_core::{ComponentSource, FolderComponents, Library};

use super::super::levels::{roots, Level};
use super::name;

/// Mirrors `ComponentText` in `app/src/lib/studio/components.ts`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ComponentText {
    pub level: Level,
    pub text: String,
}

/// Where the cascade finds `name`: the level and its file.
pub fn find(
    library: &Library,
    subject: Option<&str>,
    name: &str,
) -> Result<(Level, PathBuf), String> {
    name::check(name)?;
    let roots = roots(library, subject)?;
    for (level, root) in &roots {
        let path = root.join(name::rel(name));
        if path
            .try_exists()
            .map_err(|e| format!("cannot look for {}: {e}", path.display()))?
        {
            return Ok((*level, path));
        }
    }
    let mut available: Vec<String> = roots
        .iter()
        .flat_map(|(_, root)| FolderComponents::new(root.join("components")).names())
        .collect();
    available.sort();
    available.dedup();
    Err(format!(
        "no component `{name}` in the {} library; available: {}",
        if subject.is_some() {
            "subject or global"
        } else {
            "global"
        },
        if available.is_empty() {
            "none".to_owned()
        } else {
            available.join(", ")
        }
    ))
}

pub fn source_in(
    library: &Library,
    subject: Option<&str>,
    name: &str,
) -> Result<ComponentText, String> {
    let (level, path) = find(library, subject, name)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(ComponentText { level, text })
}
