//! Versioned drawing documents among each subject's own files: whiteboards
//! (`<title>.whiteboard`) and notebooks (`<title>.notebook`), anywhere in the
//! own zone, created in the folder `layout::own_dir` gives their kind.
pub(crate) mod authority;
pub(crate) mod drafts;
pub(crate) mod export;
pub(crate) mod export_pdf;
pub(crate) mod images;
pub(crate) mod imports;
pub(crate) mod legacy;
pub(crate) mod snapshots;

use super::ImportedFile;
use crate::library::layout::{self, OwnKind};
use serde::Deserialize;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// The two drawing documents. Both are the same `oculus-whiteboard` JSON; a
/// notebook has its own extension and always has
/// `paper`. Only a notebook has pages: `paper` in a whiteboard is kept as written
/// and means nothing, and the kind always comes from the path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DrawingKind {
    #[default]
    Whiteboard,
    Notebook,
}

impl DrawingKind {
    fn own(self) -> OwnKind {
        match self {
            Self::Whiteboard => OwnKind::Whiteboard,
            Self::Notebook => OwnKind::Notebook,
        }
    }

    pub(crate) fn extension(self) -> &'static str {
        self.own().extension().unwrap_or_default()
    }

    /// The `files` row's `file_type`, the same word as its category.
    pub fn file_type(self) -> &'static str {
        match self {
            Self::Whiteboard => "whiteboard",
            Self::Notebook => "notebook",
        }
    }

    /// The kind a library path names: an own file of either kind
    /// (`layout::own_file_ok`), and nothing else.
    pub fn of(rel: &str) -> Option<Self> {
        [Self::Whiteboard, Self::Notebook]
            .into_iter()
            .find(|kind| layout::own_file_ok(rel, kind.own()))
    }
}

const EXPECTED: &str =
    "Expected one of your whiteboards (<title>.whiteboard) or notebooks (<title>.notebook)";

pub(super) fn is_whiteboard_rel(rel: &str) -> bool {
    DrawingKind::of(rel).is_some()
}

fn name(kind: DrawingKind, title: &str) -> String {
    let ext = kind.extension();
    let stem = crate::library::paths::safe_filename(title.trim().trim_end_matches(ext));
    let stem = stem.trim_end_matches('.');
    format!(
        "{}{ext}",
        if stem.trim_matches('_').is_empty() {
            "Untitled"
        } else {
            stem
        }
    )
}

// Reject symlinks even when they point inside the root: a drawing must never
// alias a different library file.
fn checked(root: &Path, rel: &str, create_parents: bool) -> Result<PathBuf, String> {
    if !is_whiteboard_rel(rel) {
        return Err(EXPECTED.into());
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut path = root.clone();
    let parts: Vec<_> = rel.split('/').collect();
    let last = parts.len() - 1;
    for (i, part) in parts.iter().enumerate() {
        path.push(part);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err("Whiteboard paths cannot contain symlinks".into())
            }
            Ok(meta) if i < last && !meta.is_dir() => {
                return Err("Whiteboard parent is not a folder".into())
            }
            Ok(meta) if i == last && !meta.is_file() => {
                return Err("Whiteboard is not a file".into())
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && i < last && create_parents => {
                std::fs::create_dir(&path).map_err(|e| e.to_string())?;
                let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                if meta.file_type().is_symlink() || !meta.is_dir() {
                    return Err("Unsafe whiteboard folder".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && i == last => {}
            Err(e) => return Err(e.to_string()),
        }
        if i < last
            && !path
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(&root)
        {
            return Err("Whiteboard path is outside the library".into());
        }
    }
    Ok(path)
}

fn file(kind: DrawingKind, rel: String, size_bytes: u64) -> ImportedFile {
    ImportedFile::new(rel, kind.file_type(), size_bytes)
}

/// The envelope every drawing has; a notebook also needs valid `paper`.
fn validated_content(kind: DrawingKind, content: &str) -> Result<Cow<'_, str>, String> {
    let mut value: serde_json::Value = serde_json::from_str(content).map_err(|e| e.to_string())?;
    if !authority::model::drawing_type(&value)
        || value.get("version").and_then(|v| v.as_u64()) != Some(1)
        || !value.get("elements").is_some_and(|v| v.is_array())
        || !value.get("viewport").is_some_and(|v| v.is_object())
    {
        return Err("Unsupported whiteboard document".into());
    }
    if kind == DrawingKind::Notebook {
        let paper = value.get("paper").ok_or("A notebook needs pages")?;
        authority::model::validate_paper(paper)?;
    }
    if authority::model::normalize_type(&mut value) {
        Ok(Cow::Owned(authority::format::serialize(&value)?))
    } else {
        Ok(Cow::Borrowed(content))
    }
}

pub(super) fn write_at(root: &Path, rel: &str, content: &str) -> Result<u64, String> {
    let content = validated_content(DrawingKind::of(rel).ok_or(EXPECTED)?, content)?;
    let path = checked(root, rel, false)?;
    if !path.is_file() {
        return Err("Whiteboard no longer exists".into());
    }
    let tmp = super::documents::note_temp_path(&path)?;
    crate::runtime::atomic_write::write(&path, &tmp, content.as_bytes())?;
    Ok(content.len() as u64)
}

/// `create_in` at the subject's own folder for `kind`, for the tests.
#[cfg(test)]
pub(super) fn create_at(
    root: &Path,
    kind: DrawingKind,
    code: &str,
    title: &str,
    content: &str,
) -> Result<ImportedFile, String> {
    create_in(
        root,
        kind,
        &layout::own_dir_rel(code, kind.own()),
        title,
        content,
    )
}

/// A new drawing in the folder `prefix` (data-dir-relative).
pub(super) fn create_in(
    root: &Path,
    kind: DrawingKind,
    prefix: &str,
    title: &str,
    content: &str,
) -> Result<ImportedFile, String> {
    let content = validated_content(kind, content)?;
    let wanted = name(kind, title);
    let ext = kind.extension();
    for index in 1.. {
        let candidate = if index == 1 {
            wanted.clone()
        } else {
            format!("{}-{index}{ext}", wanted.trim_end_matches(ext))
        };
        let rel = format!("{prefix}/{candidate}");
        let path = checked(root, &rel, true)?;
        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
        {
            Ok(_) => {
                let write = super::documents::note_temp_path(&path).and_then(|tmp| {
                    crate::runtime::atomic_write::write(&path, &tmp, content.as_bytes())
                });
                if let Err(error) = write {
                    std::fs::remove_file(path).ok();
                    return Err(error);
                }
                return Ok(file(kind, rel, content.len() as u64));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    unreachable!()
}

/// A new whiteboard, or with `kind: "notebook"` a new notebook, in `target_dir`
/// (data-dir-relative, in the subject) or the subject's root.
#[tauri::command]
pub async fn create_whiteboard(
    subject_code: String,
    title: String,
    content: String,
    kind: Option<DrawingKind>,
    target_dir: Option<String>,
) -> Result<ImportedFile, String> {
    crate::runtime::blocking::run(move || {
        let root = crate::library::paths::data_dir();
        let target = crate::library::drive::ops::guard::own_target(
            &root,
            &subject_code,
            target_dir.as_deref(),
        )?;
        create_in(
            &root,
            kind.unwrap_or_default(),
            &target.rel,
            &title,
            &content,
        )
    })
    .await
}

#[tauri::command]
pub async fn read_whiteboard(relative_path: String) -> Result<String, String> {
    crate::runtime::blocking::run(move || {
        std::fs::read_to_string(checked(
            &crate::library::paths::data_dir(),
            &relative_path,
            false,
        )?)
        .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub async fn write_whiteboard(relative_path: String, content: String) -> Result<u64, String> {
    crate::runtime::blocking::run(move || {
        let root = crate::library::paths::data_dir();
        authority::global().unshared(&root, &relative_path, || {
            write_at(&root, &relative_path, &content)
        })
    })
    .await
}

#[tauri::command]
pub async fn rename_whiteboard(
    relative_path: String,
    title: String,
) -> Result<ImportedFile, String> {
    crate::runtime::blocking::run(move || {
        let root = crate::library::paths::data_dir();
        authority::global().unshared(&root, &relative_path, || {
            rename_at(&root, &relative_path, &title)
        })
    })
    .await
}

/// Renames within the file's own folder, keeping its extension: a rename
/// never turns a whiteboard into a notebook or back.
pub(super) fn rename_at(root: &Path, rel: &str, title: &str) -> Result<ImportedFile, String> {
    let kind = DrawingKind::of(rel).ok_or(EXPECTED)?;
    let path = checked(root, rel, false)?;
    let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    let (prefix, current) = rel.rsplit_once('/').ok_or("Missing filename")?;
    let wanted = name(kind, title);
    if wanted == current {
        return Ok(file(kind, rel.into(), size));
    }
    let destination =
        super::uploads::step_aside(path.parent().ok_or("Missing parent")?, &wanted, |_| false);
    let moved = format!("{prefix}/{destination}");
    if DrawingKind::of(&moved) != Some(kind) {
        return Err(kind_changed());
    }
    let target = checked(root, &moved, false)?;
    std::fs::rename(path, target).map_err(|e| e.to_string())?;
    Ok(file(kind, moved, size))
}

fn kind_changed() -> String {
    "A rename cannot change what kind of drawing a file is".into()
}

#[tauri::command]
pub async fn delete_whiteboard(relative_path: String) -> Result<(), String> {
    crate::runtime::blocking::run(move || {
        let root = crate::library::paths::data_dir();
        authority::global().unshared(&root, &relative_path, || {
            std::fs::remove_file(checked(&root, &relative_path, false)?).map_err(|e| e.to_string())
        })
    })
    .await
}

/// A subject's whiteboards, or with `kind: "notebook"` its notebooks; never both.
#[tauri::command]
pub async fn list_whiteboards(
    subject_code: String,
    kind: Option<DrawingKind>,
) -> Result<Vec<ImportedFile>, String> {
    crate::runtime::blocking::run(move || {
        list_at(
            &crate::library::paths::data_dir(),
            kind.unwrap_or_default(),
            &subject_code,
        )
    })
    .await
}

/// Every drawing of `kind` in the subject's own files, at any depth
/// (`layout::own_files`); one behind a symlink is not listed.
pub(super) fn list_at(
    root: &Path,
    kind: DrawingKind,
    code: &str,
) -> Result<Vec<ImportedFile>, String> {
    let mut files = Vec::new();
    for (rel, meta) in layout::own_files(root, code) {
        if DrawingKind::of(&rel) != Some(kind) || checked(root, &rel, false).is_err() {
            continue;
        }
        files.push(file(kind, rel, meta.len()));
    }
    files.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(files)
}

#[cfg(test)]
mod tests;
