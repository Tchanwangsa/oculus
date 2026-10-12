//! What narration leaves behind: entries for scenes no longer narrated, and
//! clips no entry names.

use std::collections::BTreeSet;
use std::path::Path;

use studio_core::Manifest;

/// Remove entries whose id is not in `keep`; their ids, sorted.
pub(super) fn stale_entries(manifest: &mut Manifest, keep: &BTreeSet<&str>) -> Vec<String> {
    let stale: Vec<String> = manifest
        .scenes
        .keys()
        .filter(|id| !keep.contains(id.as_str()))
        .cloned()
        .collect();
    for id in &stale {
        manifest.scenes.remove(id);
    }
    stale
}

/// Delete each `.wav` (and interrupted `.wav.part`) in `clips` that no
/// manifest entry names; their file names, sorted. Other files are left.
pub(super) fn orphan_clips(clips: &Path, manifest: &Manifest) -> Result<Vec<String>, String> {
    let entries = match std::fs::read_dir(clips) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", clips.display())),
    };
    let used: BTreeSet<&str> = manifest.scenes.values().map(|s| s.audio.as_str()).collect();
    let mut deleted = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let clip = name.ends_with(".wav") || name.ends_with(".wav.part");
        if !clip || used.contains(name.as_str()) || !entry.path().is_file() {
            continue;
        }
        std::fs::remove_file(entry.path()).map_err(|e| format!("{name}: {e}"))?;
        deleted.push(name);
    }
    deleted.sort();
    Ok(deleted)
}
