//! The student's own files: where each kind is created, and the guard every
//! command that writes, renames or deletes one runs.

use std::path::{Path, PathBuf};

use super::roots::{course_rel, course_root};
use super::shape::{Shape, CURRENT};
use super::zone::{same_name, segments, zone_of, Zone};

/// A file the student made or brought in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnKind {
    Upload,
    Document,
    Whiteboard,
    Notebook,
}

impl OwnKind {
    /// The `files.category` its rows carry.
    pub fn category(self) -> &'static str {
        match self {
            Self::Upload => "upload",
            Self::Document => "document",
            Self::Whiteboard => "whiteboard",
            Self::Notebook => "notebook",
        }
    }

    /// The extension every file of this kind has; an upload can be anything.
    pub fn extension(self) -> Option<&'static str> {
        match self {
            Self::Upload => None,
            Self::Document => Some(".md"),
            Self::Whiteboard => Some(".whiteboard"),
            Self::Notebook => Some(".notebook"),
        }
    }

    /// The kind a file name says it is, wherever it sits.
    pub(super) fn by_extension(name: &str) -> Self {
        [Self::Document, Self::Whiteboard, Self::Notebook]
            .into_iter()
            .find(|k| k.extension().is_some_and(|ext| name.ends_with(ext)))
            .unwrap_or(Self::Upload)
    }
}

/// Where the student's files live in a subject.
pub fn own_root(data_dir: &Path, code: &str) -> PathBuf {
    course_root(data_dir, code)
}

/// The folder a new file of `kind` lands in, on disk.
pub fn own_dir(data_dir: &Path, code: &str, kind: OwnKind) -> PathBuf {
    match CURRENT.own_dir(kind) {
        "" => own_root(data_dir, code),
        dir => own_root(data_dir, code).join(dir),
    }
}

/// Every file in a subject's own zone at any depth, as data-dir-relative
/// paths `own_path_ok` accepts, with their metadata. The synced and hidden
/// folders, dot-names and symlinks (a subject's `AGENTS.md`) are not walked.
pub fn own_files(data_dir: &Path, code: &str) -> Vec<(String, std::fs::Metadata)> {
    let mut out = Vec::new();
    let mut pending = vec![(own_root(data_dir, code), course_rel(code))];
    while let Some((dir, rel)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(meta) = entry.metadata() else { continue };
            let child = format!("{rel}/{name}");
            if name.starts_with('.')
                || zone_of(&child) != Zone::Own
                || entry.file_type().is_ok_and(|t| t.is_symlink())
            {
                continue;
            }
            if meta.is_dir() {
                pending.push((entry.path(), child));
            } else if meta.is_file() && own_path_ok(&child) {
                out.push((child, meta));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Each subject's top-level own entries on disk, files and folders: what
/// sits beside `Course/` and `.oculus/`, less symlinks (a subject's
/// `AGENTS.md`). A sandbox that cannot carve those two out of a subject's
/// folder grants these one by one (`harness::providers::codex`).
pub fn own_top_level(data_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for code in super::subjects_on_disk(data_dir) {
        let Ok(entries) = std::fs::read_dir(own_root(data_dir, &code)) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let rel = format!("{}/{name}", course_rel(&code));
            if zone_of(&rel) != Zone::Own || entry.file_type().is_ok_and(|t| t.is_symlink()) {
                continue;
            }
            out.push(entry.path());
        }
    }
    out.sort();
    out
}

/// The same folder, data-dir-relative.
pub fn own_dir_rel(code: &str, kind: OwnKind) -> String {
    match CURRENT.own_dir(kind) {
        "" => course_rel(code),
        dir => format!("{}/{dir}", course_rel(code)),
    }
}

/// The folder beside a note that holds its pictures; also the prefix of the
/// links the note carries.
pub fn note_assets_dir() -> &'static str {
    CURRENT.note_assets
}

/// True for a data-dir-relative path the student may write, rename or
/// delete: in the own zone, below its folder, with no traversal, no
/// dot-name (staging files and hidden folders) and no `:` (a Windows drive
/// or stream).
pub fn own_path_ok(rel: &str) -> bool {
    CURRENT.own_path_ok(rel)
}

/// The kind of an own file, from its folder or its extension (the layout
/// decides which); `None` when `own_path_ok` refuses it.
pub fn own_kind_of(rel: &str) -> Option<OwnKind> {
    CURRENT.own_kind_of(rel)
}

/// True when `rel` is an own file of `kind`: `own_kind_of`, plus a name in
/// front of the kind's extension, and for a note, not one of the pictures
/// folder's files.
pub fn own_file_ok(rel: &str, kind: OwnKind) -> bool {
    CURRENT.own_file_ok(rel, kind)
}

impl Shape {
    /// The path inside its subject, when `rel` is in the own zone.
    fn own_inner<'a>(&self, rel: &'a str) -> Option<Vec<&'a str>> {
        if self.zone_of(rel) != Zone::Own {
            return None;
        }
        Some(segments(rel)?.split_off(2))
    }

    pub fn own_path_ok(&self, rel: &str) -> bool {
        let Some(inner) = self.own_inner(rel) else {
            return false;
        };
        let base = usize::from(self.own_kind_by_folder);
        inner.len() > base
            && inner
                .iter()
                .all(|part| !part.starts_with('.') && !part.contains(':'))
    }

    pub fn own_kind_of(&self, rel: &str) -> Option<OwnKind> {
        if !self.own_path_ok(rel) {
            return None;
        }
        let inner = self.own_inner(rel)?;
        if self.own_kind_by_folder {
            self.own_dirs
                .iter()
                .find(|(_, dir)| !dir.is_empty() && same_name(inner[0], dir))
                .map(|(kind, _)| *kind)
        } else {
            Some(OwnKind::by_extension(inner.last()?))
        }
    }

    pub fn own_file_ok(&self, rel: &str, kind: OwnKind) -> bool {
        if self.own_kind_of(rel) != Some(kind) {
            return false;
        }
        let mut parts = rel.rsplit('/');
        let name = parts.next().unwrap_or_default();
        let folder = parts.next().unwrap_or_default();
        kind.extension()
            .is_none_or(|ext| name.len() > ext.len() && name.ends_with(ext))
            && !(kind == OwnKind::Document && same_name(folder, self.note_assets))
    }
}
