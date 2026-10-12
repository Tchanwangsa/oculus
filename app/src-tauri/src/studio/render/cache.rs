//! `<dir>/.cache/`: encoded segments (`segments/<key>.mp4`), mixed
//! narration tracks (`narration-<key>.wav`), the concat lists, and
//! `renders.json`, which records what each preset's last successful render
//! used and wrote. After a render, segments and tracks no preset's last
//! render used are deleted, as are interrupted `.part` files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::error::RenderError;
use super::preset::Preset;

pub(super) fn root(dir: &Path) -> PathBuf {
    dir.join(".cache")
}

pub(super) fn segments(dir: &Path) -> PathBuf {
    root(dir).join("segments")
}

pub(super) fn segment(dir: &Path, key: &str) -> PathBuf {
    segments(dir).join(format!("{key}.mp4"))
}

pub(super) fn track(dir: &Path, key: &str) -> PathBuf {
    root(dir).join(format!("narration-{key}.wav"))
}

pub(super) fn concat_list(dir: &Path, preset: Preset) -> PathBuf {
    root(dir).join(format!("concat-{}.txt", preset.name()))
}

/// A cache file's name, as `Used` records it.
pub(super) fn file_name(path: &Path) -> Option<String> {
    path.file_name().map(|n| n.to_string_lossy().into_owned())
}

fn record_path(dir: &Path) -> PathBuf {
    root(dir).join("renders.json")
}

/// What one render used, by file name, and what it wrote. Two equal
/// records mean a render now would make the same file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct Used {
    pub segments: Vec<String>,
    pub track: Option<String>,
    /// The file written.
    #[serde(default)]
    pub out: Option<String>,
    /// sha256 of the subtitles' text, for the `subtitles` preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cues: Option<String>,
}

/// Each preset's last render, by name. An unreadable record counts as
/// empty.
pub(super) fn records(dir: &Path) -> BTreeMap<String, Used> {
    std::fs::read(record_path(dir))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Record `used` as `preset`'s last render, then delete every segment and
/// track no preset's record names, and every `.part`; the names deleted,
/// sorted.
pub(super) fn record_and_prune(
    dir: &Path,
    preset: Preset,
    used: Used,
) -> Result<Vec<String>, RenderError> {
    let path = record_path(dir);
    let mut record = records(dir);
    record.insert(preset.name().to_owned(), used);
    crate::runtime::atomic_write::json(&path, &record).map_err(|e| RenderError::io(&path, e))?;

    let keep: BTreeSet<&str> = record
        .values()
        .flat_map(|u| u.segments.iter().chain(&u.track))
        .map(String::as_str)
        .collect();
    let mut deleted = Vec::new();
    for (folder, is_ours) in [
        (segments(dir), is_segment as fn(&str) -> bool),
        (root(dir), is_track),
    ] {
        let entries = match std::fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(RenderError::io(&folder, e)),
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stale = name.ends_with(".part") || (is_ours(&name) && !keep.contains(&*name));
            if stale && entry.path().is_file() {
                std::fs::remove_file(entry.path())
                    .map_err(|e| RenderError::io(&entry.path(), e))?;
                deleted.push(name);
            }
        }
    }
    deleted.sort();
    Ok(deleted)
}

fn is_segment(name: &str) -> bool {
    name.ends_with(".mp4")
}

fn is_track(name: &str) -> bool {
    name.starts_with("narration-") && name.ends_with(".wav")
}
