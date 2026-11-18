//! Text that names a drawing, rewritten once every rename has landed: each
//! notebook pages manifest's `canvas` key (now `whiteboard`), and the
//! student's notes that link to a renamed drawing. Each is a no-op the second
//! time, so a run cut short repeats this part whole.

use std::path::Path;

use serde_json::Value;

use super::journal::Plan;
use super::LEGACY_EXTENSION;
use crate::library::layout;
use crate::library::migrate::links::Linker;
use crate::library::migrate::plan::base;
use crate::library::paths::{PAGES_DIR, PAGES_MANIFEST};

/// The manifest key the pictures' notebook path was under.
const LEGACY_MANIFEST_KEY: &str = "canvas";
const MANIFEST_KEY: &str = "whiteboard";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ContentCounts {
    pub manifests: usize,
    pub notes: usize,
    pub links: usize,
}

pub fn rewrite(data_dir: &Path, plan: &Plan, linker: &Linker<'_>) -> Result<ContentCounts, String> {
    let mut counts = ContentCounts {
        manifests: manifests(data_dir)?,
        ..ContentCounts::default()
    };
    if plan.rules.is_empty() {
        return Ok(counts);
    }
    for code in layout::subjects_on_disk(data_dir) {
        for (rel, _) in layout::own_files(data_dir, &code) {
            if !rel.to_ascii_lowercase().ends_with(".md") {
                continue;
            }
            let path = data_dir.join(&rel);
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if !text.contains(LEGACY_EXTENSION) {
                continue;
            }
            let (relinked, n) = linker.relink(&text, &rel, &rel);
            if n > 0 {
                let tmp = path.with_file_name(format!(".{}.whiteboard-rename", base(&rel)));
                crate::runtime::atomic_write::write(&path, &tmp, relinked.as_bytes())?;
                counts.notes += 1;
                counts.links += n;
            }
        }
    }
    Ok(counts)
}

/// Every `<file id>/pages.json` with its notebook's path under the key the
/// app reads.
fn manifests(data_dir: &Path) -> Result<usize, String> {
    let Ok(read) = std::fs::read_dir(data_dir.join(PAGES_DIR)) else {
        return Ok(0);
    };
    let mut changed = 0;
    for entry in read.flatten() {
        let path = entry.path().join(PAGES_MANIFEST);
        let Ok(raw) = std::fs::read(&path) else {
            continue;
        };
        let Ok(Value::Object(mut map)) = serde_json::from_slice::<Value>(&raw) else {
            continue;
        };
        let Some(notebook) = map.remove(LEGACY_MANIFEST_KEY) else {
            continue;
        };
        map.entry(MANIFEST_KEY).or_insert(notebook);
        crate::runtime::atomic_write::json(&path, &Value::Object(map))?;
        changed += 1;
    }
    Ok(changed)
}
