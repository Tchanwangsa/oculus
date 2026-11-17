//! Paths inside files, rewritten once everything has moved: every moved
//! markdown file's links, a renamed PDF's parse records, the Ed workspace
//! records and notebook page manifests outside the library, the subjects'
//! memories links, and opencode's generated config. Each is a no-op the
//! second time, so a run cut short repeats this part whole.

use std::path::Path;

use serde_json::Value;

use super::db::remap_json;
use super::journal::Plan;
use super::links::Linker;
use super::rules::Rules;
use crate::library::paths::{PAGES_DIR, PAGES_MANIFEST};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ContentCounts {
    pub notes: usize,
    pub links: usize,
    pub records: usize,
    pub workspaces: usize,
    pub manifests: usize,
}

pub fn rewrite(
    data_dir: &Path,
    plan: &Plan,
    rules: &Rules,
    linker: &Linker<'_>,
) -> Result<ContentCounts, String> {
    let mut counts = ContentCounts::default();
    for note in &plan.notes {
        let path = data_dir.join(&note.new);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (relinked, n) = linker.relink(&text, &note.old, &note.new);
        if n > 0 {
            write(&path, relinked.as_bytes())?;
            counts.notes += 1;
            counts.links += n;
        }
    }
    for (record, pdf) in &plan.renamed_records {
        if rename_in_record(&data_dir.join(record), &pdf.old, &pdf.new)? {
            counts.records += 1;
        }
    }
    counts.workspaces = remap_files(
        &data_dir.join(crate::sources::ed::workspace_state::DIR),
        None,
        rules,
    )?;
    counts.manifests = remap_files(&data_dir.join(PAGES_DIR), Some(PAGES_MANIFEST), rules)?;
    for code in &plan.relink {
        crate::agents::link_course(data_dir, code)
            .map_err(|e| format!("relink {code}'s memories: {e}"))?;
    }
    Ok(counts)
}

fn write(path: &Path, body: &[u8]) -> Result<(), String> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.library-move"));
    crate::runtime::atomic_write::write(path, &tmp, body)
}

/// A parse record of a PDF renamed on a clash: the PDF's name it records,
/// and its pages' links into the pictures folder, which took the new stem.
fn rename_in_record(path: &Path, old_pdf: &str, new_pdf: &str) -> Result<bool, String> {
    let Ok(raw) = std::fs::read(path) else {
        return Ok(false);
    };
    let Ok(mut value) = serde_json::from_slice::<Value>(&raw) else {
        return Ok(false);
    };
    let stem = |name: &str| name.rsplit_once('.').map_or(name, |(s, _)| s).to_string();
    let (old_images, new_images) = (
        format!("{}_images/", stem(old_pdf)),
        format!("{}_images/", stem(new_pdf)),
    );
    let mut changed = false;
    if value.get("pdf").and_then(Value::as_str) == Some(old_pdf) {
        value["pdf"] = Value::String(new_pdf.to_string());
        changed = true;
    }
    changed |= replace_strings(&mut value, &old_images, &new_images);
    if changed {
        let body = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
        write(path, &body)?;
    }
    Ok(changed)
}

fn replace_strings(value: &mut Value, from: &str, to: &str) -> bool {
    match value {
        Value::String(s) if s.contains(from) => {
            *s = s.replace(from, to);
            true
        }
        Value::Array(items) => {
            // Every element is visited: `any` would stop at the first change.
            let mut changed = false;
            for v in items {
                changed |= replace_strings(v, from, to);
            }
            changed
        }
        Value::Object(map) => {
            let mut changed = false;
            for v in map.values_mut() {
                changed |= replace_strings(v, from, to);
            }
            changed
        }
        _ => false,
    }
}

/// The JSON files in `dir` (or, with `inner`, the file of that name in each
/// of its folders) with their library paths followed through the rules.
fn remap_files(dir: &Path, inner: Option<&str>, rules: &Rules) -> Result<usize, String> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Ok(0);
    };
    let mut changed = 0;
    for entry in read.flatten() {
        let path = match inner {
            Some(name) => entry.path().join(name),
            None => entry.path(),
        };
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = std::fs::read(&path) else {
            continue;
        };
        let Ok(mut value) = serde_json::from_slice::<Value>(&raw) else {
            continue;
        };
        if remap_json(&mut value, rules) {
            crate::runtime::atomic_write::json(&path, &value)?;
            changed += 1;
        }
    }
    Ok(changed)
}
