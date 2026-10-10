use std::path::{Path, PathBuf};

use crate::agents;

/// Where one bucket lives. `None` is the cross-subject one.
pub fn bucket_dir(data_dir: &Path, code: Option<&str>) -> PathBuf {
    match code {
        Some(code) => agents::subject_memories(data_dir, code),
        None => agents::agents_dir(data_dir).join("memories"),
    }
}

/// A subject code to the course folder its bucket is named for.
///
/// Resolved on the filesystem, not the subjects table, so it works from a
/// sandbox. A bare code matches the folder it prefixes; a bucket without a
/// course folder still resolves, so an unsynced subject keeps its memories.
pub fn resolve_subject(data_dir: &Path, code: &str) -> Result<String, String> {
    let wanted = code.trim().to_ascii_uppercase();
    if wanted.is_empty() {
        return Err("a subject code cannot be empty".into());
    }
    let mut names: Vec<String> = Vec::new();
    for dir in [data_dir.join("courses"), bucket_dir(data_dir, None)] {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let Some(name) = e.file_name().to_str().map(String::from) else {
                continue;
            };
            if name.starts_with('.') || names.contains(&name) {
                continue;
            }
            names.push(name);
        }
    }
    names.sort();
    let hit: Vec<&String> = names
        .iter()
        .filter(|n| n.to_ascii_uppercase().starts_with(&wanted))
        .collect();
    match hit.len() {
        1 => Ok(hit[0].clone()),
        0 => Err(format!(
            "no subject matches {code}{}",
            if names.is_empty() {
                String::new()
            } else {
                format!(" — the library has {}", names.join(", "))
            }
        )),
        _ => Err(format!(
            "{code} matched {} subjects ({}) — pass the full folder name",
            hit.len(),
            hit.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Every bucket that exists, the cross-subject one first.
pub fn buckets(data_dir: &Path) -> Vec<Option<String>> {
    let mut out = vec![None];
    if let Ok(rd) = std::fs::read_dir(bucket_dir(data_dir, None)) {
        let mut codes: Vec<String> = rd
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .filter(|n| !n.starts_with('.'))
            .collect();
        codes.sort();
        out.extend(codes.into_iter().map(Some));
    }
    out
}
