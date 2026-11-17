//! What a folder command may touch: an own entry, a target folder, and the
//! files below a folder. Every path is data-dir-relative and checked by the
//! layout's guard (`layout::own_path_ok`) before the disk is asked.

use std::path::{Path, PathBuf};

use super::error::{DriveError, DriveResult};
use crate::library::layout::{self, Zone};
use crate::library::paths::{fold_code, safe_dir};

/// An existing file or folder of the student's.
#[derive(Clone, Debug)]
pub struct Entry {
    pub rel: String,
    pub path: PathBuf,
    pub dir: bool,
}

/// A folder new entries may land in: a subject's root or one of the
/// student's folders.
#[derive(Clone, Debug)]
pub struct Target {
    pub rel: String,
    pub path: PathBuf,
}

impl Target {
    /// The subject folder this target is in (`courses/<folder>`'s folder).
    pub fn subject_folder(&self) -> &str {
        subject_folder(&self.rel).unwrap_or_default()
    }

    pub fn child_rel(&self, name: &str) -> String {
        format!("{}/{name}", self.rel)
    }

    /// True for the subject's root, where reserved names are refused.
    pub fn is_subject_root(&self) -> bool {
        is_subject_root(&self.rel)
    }
}

/// The subject folder a path is in, or the path's own name when it is one.
pub fn subject_folder(rel: &str) -> Option<&str> {
    let rest = rel.strip_prefix(layout::COURSES_DIR)?.strip_prefix('/')?;
    let folder = rest.split('/').next()?;
    (!folder.is_empty()).then_some(folder)
}

pub fn is_subject_root(rel: &str) -> bool {
    layout::split_course(&format!("{rel}/x")).is_some_and(|(_, inner)| inner == "x")
}

pub fn parent_rel(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// The refusal for a path the student may not change: synced and hidden
/// paths are read-only, one that could leave the library is no name at all.
fn refusal(rel: &str) -> DriveError {
    match layout::zone_of(rel) {
        Zone::Outside => DriveError::invalid_name(format!("{rel} is not a library path")),
        _ => DriveError::read_only(rel),
    }
}

/// True when some folder on the way to `rel` (or `rel` itself) is a symlink:
/// Drive never follows one.
fn through_symlink(data_dir: &Path, rel: &str) -> bool {
    let mut walked = data_dir.to_path_buf();
    rel.split('/').any(|part| {
        walked.push(part);
        std::fs::symlink_metadata(&walked).is_ok_and(|m| m.file_type().is_symlink())
    })
}

/// `rel` as an existing entry the student may rename, move or delete.
pub fn own_entry(data_dir: &Path, rel: &str) -> DriveResult<Entry> {
    if !layout::own_path_ok(rel) {
        return Err(refusal(rel));
    }
    if through_symlink(data_dir, rel) {
        return Err(DriveError::read_only(rel));
    }
    let path = data_dir.join(rel);
    let meta = std::fs::symlink_metadata(&path).map_err(|_| DriveError::not_found(rel))?;
    Ok(Entry {
        rel: rel.to_string(),
        path,
        dir: meta.is_dir(),
    })
}

/// `rel` as an existing entry a copy may read: anything Drive lists, synced
/// or own, but not app state or the subject folders themselves.
pub fn readable_entry(data_dir: &Path, rel: &str) -> DriveResult<Entry> {
    let listed = layout::split_course(rel).is_some()
        && matches!(layout::zone_of(rel), Zone::Synced | Zone::Own)
        && !rel.split('/').any(|part| part.starts_with('.'));
    if !listed || is_subject_root(rel) {
        return Err(refusal(rel));
    }
    if through_symlink(data_dir, rel) {
        return Err(DriveError::read_only(rel));
    }
    let path = data_dir.join(rel);
    let meta = std::fs::symlink_metadata(&path).map_err(|_| DriveError::not_found(rel))?;
    Ok(Entry {
        rel: rel.to_string(),
        path,
        dir: meta.is_dir(),
    })
}

/// `rel` as a folder new entries may land in: a subject's root, or an
/// existing folder of the student's.
pub fn target_folder(data_dir: &Path, rel: &str) -> DriveResult<Target> {
    if !is_subject_root(rel) && !layout::own_path_ok(rel) {
        return Err(refusal(rel));
    }
    if through_symlink(data_dir, rel) {
        return Err(DriveError::read_only(rel));
    }
    let path = data_dir.join(rel);
    match std::fs::metadata(&path) {
        Ok(meta) if meta.is_dir() => Ok(Target {
            rel: rel.to_string(),
            path,
        }),
        Ok(_) => Err(DriveError::invalid_name(format!(
            "{} is not a folder",
            super::error::shown(rel)
        ))),
        Err(_) => Err(DriveError::not_found(rel)),
    }
}

/// Where a creator for subject `code` writes: `target_dir` when given (it
/// must be in that subject), else the subject's root, made if missing.
pub fn own_target(data_dir: &Path, code: &str, target_dir: Option<&str>) -> DriveResult<Target> {
    let Some(rel) = target_dir else {
        let rel = layout::course_rel(code);
        let path = layout::course_root(data_dir, code);
        std::fs::create_dir_all(&path)?;
        return Ok(Target { rel, path });
    };
    let target = target_folder(data_dir, rel)?;
    if fold_code(target.subject_folder()) != fold_code(&safe_dir(code)) {
        return Err(DriveError::io(format!("{rel} is not in {code}")));
    }
    Ok(target)
}

/// Every file below `entry` (or the file itself), data-dir-relative, with no
/// symlink followed; dot-folders included, since they move and go with it.
pub fn files_under(entry: &Entry) -> Vec<String> {
    if !entry.dir {
        return vec![entry.rel.clone()];
    }
    let mut out = Vec::new();
    let mut pending = vec![(entry.path.clone(), entry.rel.clone())];
    while let Some((dir, rel)) = pending.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for child in read.flatten() {
            let Ok(name) = child.file_name().into_string() else {
                continue;
            };
            let Ok(kind) = child.file_type() else {
                continue;
            };
            let child_rel = format!("{rel}/{name}");
            if kind.is_dir() {
                pending.push((child.path(), child_rel));
            } else if kind.is_file() {
                out.push(child_rel);
            }
        }
    }
    out.sort();
    out
}
