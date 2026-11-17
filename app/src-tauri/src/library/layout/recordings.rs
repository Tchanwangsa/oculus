//! Lecture recordings: one folder per Echo360 media id, holding its streams,
//! transcript and chapter frames.
//!
//! Callers name a recording by its media id alone; the folder it maps to is
//! this module's business, so a layout that names folders by date and title
//! resolves the id here instead of at every caller.

use std::path::{Path, PathBuf};

use super::shape::CURRENT;

/// `lectures/<id>`, data-dir-relative.
pub fn recording_rel(id: &str) -> String {
    let root = CURRENT
        .recordings_root
        .expect("recordings have a data-dir folder");
    format!("{root}/{id}")
}

/// A recording's folder. Refuses an id that is not one folder name, since
/// the commands that take one get it from the frontend.
pub fn recording_dir(data_dir: &Path, id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || id.starts_with('.') || id.contains(['/', '\\', ':', '\0']) {
        return Err(format!("{id:?} is not a lecture id"));
    }
    Ok(data_dir.join(recording_rel(id)))
}

/// Every recording folder on disk, for sweeps over all of them.
pub fn recording_dirs(data_dir: &Path) -> Vec<PathBuf> {
    let Some(root) = CURRENT.recordings_root else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(data_dir.join(root)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}
