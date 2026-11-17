//! Which part of the data dir a path is in: the question every guard asks.

use std::path::{Path, PathBuf};

use super::roots::COURSES_DIR;
use super::shape::{Shape, CURRENT};
use crate::runtime::platform::canonicalize;

/// * `Synced` — written by Oculus's sync, read-only to the student and the
///   agents (a subject's `Course/` with its lecture recordings, and the
///   `courses/` and subject folders themselves).
/// * `Own` — the student's own files: everything else in a subject.
/// * `Hidden` — inside the data dir but not a library file: a subject's
///   `.oculus/` (its agent workspace and derived records), and everything at
///   the data dir's top level outside the library (the agents' workspace,
///   Studio, the database).
/// * `Outside` — could leave the data dir: absolute, `..`, a backslash, a
///   drive, an empty or `.` segment, a NUL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Synced,
    Own,
    Hidden,
    Outside,
}

/// The zone of a data-dir-relative path, from its spelling alone; `resolve`
/// adds the symlink check. Folder names compare ignoring ASCII case, as the
/// filesystems Oculus runs on do.
pub fn zone_of(rel: &str) -> Zone {
    CURRENT.zone_of(rel)
}

/// The names the layout keeps for itself at a subject's root (`Course`,
/// `.oculus`): what an agent's containment protects in every subject.
pub fn reserved_names() -> Vec<&'static str> {
    CURRENT
        .synced_root
        .into_iter()
        .chain(CURRENT.hidden.iter().copied())
        .collect()
}

/// True for a name the layout keeps for itself at a subject's root, ignoring
/// case: no folder of the student's may take it.
pub fn reserved_name(name: &str) -> bool {
    reserved_names().into_iter().any(|r| same_name(name, r))
}

/// `/`-separated segments, or `None` for any spelling that could leave the
/// directory it is joined onto.
pub(super) fn segments(rel: &str) -> Option<Vec<&str>> {
    if rel.is_empty() || rel.starts_with('/') || rel.contains(['\\', '\0']) {
        return None;
    }
    let parts: Vec<&str> = rel.split('/').collect();
    let bad = |p: &&str| p.is_empty() || *p == "." || *p == "..";
    // `C:x` is relative to a drive's current folder on Windows.
    (!parts.iter().any(bad) && !parts[0].contains(':')).then_some(parts)
}

pub(super) fn same_name(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

impl Shape {
    pub fn zone_of(&self, rel: &str) -> Zone {
        let Some(parts) = segments(rel) else {
            return Zone::Outside;
        };
        if same_name(parts[0], COURSES_DIR) {
            return match parts.get(2..) {
                Some(inner) if !inner.is_empty() => self.subject_zone(inner),
                _ => Zone::Synced,
            };
        }
        if self
            .recordings_root()
            .is_some_and(|r| same_name(parts[0], r))
        {
            return Zone::Synced;
        }
        Zone::Hidden
    }

    /// The zone of a path inside one subject, by its first segment.
    fn subject_zone(&self, inner: &[&str]) -> Zone {
        let first = inner[0];
        if self.hidden.iter().any(|h| same_name(first, h)) {
            return Zone::Hidden;
        }
        match self.synced_root {
            Some(root) if same_name(first, root) => Zone::Synced,
            Some(_) => Zone::Own,
            None if self
                .own_dirs
                .iter()
                .any(|(_, dir)| !dir.is_empty() && same_name(first, dir)) =>
            {
                Zone::Own
            }
            None => Zone::Synced,
        }
    }
}

/// The file a data-dir-relative path names, for an IPC command that reads or
/// opens what the frontend asks for. Refuses anything `Outside`, and a path
/// whose symlinks lead out of the data dir. A missing file is the
/// filesystem's error.
pub fn resolve(data_dir: &Path, rel: &str) -> Result<PathBuf, String> {
    if zone_of(rel) == Zone::Outside {
        return Err(format!("{rel} is outside the library"));
    }
    let root = canonicalize(data_dir).map_err(|e| e.to_string())?;
    let path = canonicalize(root.join(rel)).map_err(|e| e.to_string())?;
    if !path.starts_with(&root) {
        return Err(format!("{rel} is outside the library"));
    }
    Ok(path)
}

/// `resolve` for an absolute path the database stored (a lecture's
/// transcript).
pub fn resolve_absolute(data_dir: &Path, path: &Path) -> Result<PathBuf, String> {
    let outside = || format!("{} is outside the library", path.display());
    if !path.is_absolute() {
        return Err(outside());
    }
    let root = canonicalize(data_dir).map_err(|e| e.to_string())?;
    let full = canonicalize(path).map_err(|e| e.to_string())?;
    let rel = full.strip_prefix(&root).map_err(|_| outside())?;
    if zone_of(&crate::library::paths::slash_path(rel)) == Zone::Outside {
        return Err(outside());
    }
    Ok(full)
}
