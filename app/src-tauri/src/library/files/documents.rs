//! Markdown notes among the student's own files, created in a folder of the
//! student's (`targetDir`) or the subject's root: Rust moves
//! the bytes, the frontend writes the `files` row. A note is rewritten on save
//! and moves on rename, so every mutating command checks the path's shape
//! (`layout::own_file_ok`) before resolving it.

use std::path::Path;

use super::uploads::step_aside;
use super::ImportedFile;
use crate::library::drive::ops::guard::{own_target, Target};
use crate::library::layout::{self, OwnKind};

/// A title's filename: sanitised, `Untitled` when only underscores survive,
/// `.md` always. A trailing dot is dropped, or `notes..md` would be sanitised
/// again on write and land under a different name.
pub(super) fn document_name(title: &str) -> String {
    let stem = crate::library::paths::safe_filename(title.trim());
    // A leading dot would make it a dot-name, which the guard refuses.
    let stem = stem.trim_matches('.');
    let stem = if stem.trim_matches('_').is_empty() {
        "Untitled"
    } else {
        stem
    };
    format!("{stem}.md")
}

/// A document's absolute path, or an error — the one place the guard runs.
fn document_path(relative_path: &str) -> Result<std::path::PathBuf, String> {
    if !layout::own_file_ok(relative_path, OwnKind::Document) {
        return Err(format!("{relative_path} is not one of your documents"));
    }
    Ok(crate::library::paths::data_dir().join(relative_path))
}

/// A note's pictures: `layout::note_assets_dir()` beside it, so they resolve
/// relative to the note for every reader (`useLibraryMdComponents` in
/// `FileViewer.tsx`). Takes the path `document_path` guarded.
pub(super) fn document_assets_dir(note: &Path) -> Result<std::path::PathBuf, String> {
    let dir = note
        .parent()
        .ok_or_else(|| format!("{} has no folder", note.display()))?;
    Ok(dir.join(layout::note_assets_dir()))
}

/// The link the editor writes, relative to the note (survives a rename).
pub(super) fn document_asset_ref(name: &str) -> String {
    format!("{}/{name}", layout::note_assets_dir())
}

fn document_file(relative_path: String, size_bytes: u64) -> ImportedFile {
    ImportedFile::new(relative_path, "md", size_bytes)
}

/// Whether `wanted` already names the file at `current` — on a
/// case-insensitive volume a case-only rename would otherwise step aside.
pub(crate) fn same_entry(current: &Path, wanted: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::metadata(current), std::fs::metadata(wanted)) {
            (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };
        let identity = |path: &Path| -> Option<(u32, u32, u32)> {
            let file = std::fs::File::open(path).ok()?;
            let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
            if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
                return None;
            }
            Some((
                info.dwVolumeSerialNumber,
                info.nFileIndexHigh,
                info.nFileIndexLow,
            ))
        };
        matches!((identity(current), identity(wanted)), (Some(a), Some(b)) if a == b)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (current, wanted);
        false
    }
}

/// Create an empty note in `target_dir` (data-dir-relative, in the subject),
/// or the subject's root.
#[tauri::command]
pub fn create_document(
    subject_code: String,
    title: String,
    target_dir: Option<String>,
) -> Result<ImportedFile, String> {
    let data_dir = crate::library::paths::data_dir();
    let target = own_target(&data_dir, &subject_code, target_dir.as_deref())?;
    create_document_in(&target, &title, b"")
}

/// A note with its whole body at the subject's root.
pub(crate) fn create_document_body_in(
    data_dir: &Path,
    code: &str,
    title: &str,
    body: &[u8],
) -> Result<ImportedFile, String> {
    let target = own_target(data_dir, code, None).map_err(|e| e.message)?;
    create_document_in(&target, title, body)
}

/// Reserve a distinct note in `target` and write its complete body.
/// `create_new` is the final authority, so two views choosing the same title
/// never overwrite one another or an existing document.
fn create_document_in(target: &Target, title: &str, body: &[u8]) -> Result<ImportedFile, String> {
    use std::io::Write;
    let dir = &target.path;
    loop {
        let name = step_aside(dir, &document_name(title), |_| false);
        let path = dir.join(&name);
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        };
        if let Err(e) = file.write_all(body).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(e.to_string());
        }
        return Ok(document_file(target.child_rel(&name), body.len() as u64));
    }
}

/// The hidden sibling a note's text is staged in. It ends in `.tmp`, never
/// `.md` and is a dot-name, so `layout::own_file_ok` rejects a leftover and
/// it never becomes a row.
pub(crate) fn note_temp_path(note: &Path) -> Result<std::path::PathBuf, String> {
    let dir = note
        .parent()
        .ok_or_else(|| format!("{} has no folder", note.display()))?;
    let name = note
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    Ok(dir.join(format!(
        ".{name}.{}-{}.tmp",
        std::process::id(),
        crate::runtime::clock::now_nanos()
    )))
}

/// Replace a note's text whole: a reader, or a crash, sees the old note or
/// the new, never a truncated one. fsynced before the rename (in
/// `atomic_write`), since a rename can otherwise land before the data does.
pub(super) fn write_note(note: &Path, content: &[u8]) -> Result<(), String> {
    let tmp = note_temp_path(note)?;
    if let Some(dir) = tmp.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    crate::runtime::atomic_write::write(note, &tmp, content)
}

/// Save a note's text; returns the byte count for `size_bytes`.
#[tauri::command]
pub async fn write_document(relative_path: String, content: String) -> Result<u64, String> {
    let path = document_path(&relative_path)?;
    crate::runtime::blocking::run(move || {
        write_note(&path, content.as_bytes())?;
        Ok(content.len() as u64)
    })
    .await
}

/// Retitle (rename) a note; a title another note holds steps aside.
#[tauri::command]
pub fn rename_document(relative_path: String, title: String) -> Result<ImportedFile, String> {
    let path = document_path(&relative_path)?;
    let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    let (dir_rel, current) = relative_path
        .rsplit_once('/')
        .ok_or_else(|| format!("{relative_path} has no filename"))?;
    let wanted = document_name(&title);
    if wanted == current {
        return Ok(document_file(relative_path, size));
    }
    let dir = path
        .parent()
        .ok_or_else(|| format!("{relative_path} has no folder"))?;
    let name = if same_entry(&path, &dir.join(&wanted)) {
        wanted
    } else {
        step_aside(dir, &wanted, |_| false)
    };
    std::fs::rename(&path, dir.join(&name)).map_err(|e| e.to_string())?;
    Ok(document_file(format!("{dir_rel}/{name}"), size))
}

/// Every note among a subject's own files at any depth, for
/// `reconcileDocuments` to reconcile against — including ones written outside the app. Only
/// guard-accepted names, and never a `.md` the parser or the sheet converter
/// wrote beside its source (`layout::is_derived_text`).
#[tauri::command]
pub fn list_documents(subject_code: String) -> Result<Vec<ImportedFile>, String> {
    let data_dir = crate::library::paths::data_dir();
    Ok(layout::own_files(&data_dir, &subject_code)
        .into_iter()
        .filter(|(rel, _)| {
            layout::own_file_ok(rel, OwnKind::Document) && !layout::is_derived_text(&data_dir, rel)
        })
        .map(|(rel, meta)| document_file(rel, meta.len()))
        .collect())
}

/// A picture pasted into a note (base64), written now into its pictures
/// folder and returned as the link. A tag later deleted leaves an orphan
/// file — accepted. Cap, sniff and naming are `crate::harness::attach`'s.
#[tauri::command]
pub async fn attach_document_image(relative_path: String, data: String) -> Result<String, String> {
    let dir = document_assets_dir(&document_path(&relative_path)?)?;
    let bytes = crate::harness::attach::decode(&data)?;
    crate::runtime::blocking::run(move || {
        crate::harness::attach::write_image(&dir, &bytes).map(|name| document_asset_ref(&name))
    })
    .await
}

/// The same, for a picture dropped from Finder (a path, read here).
#[tauri::command]
pub async fn attach_document_file(relative_path: String, path: String) -> Result<String, String> {
    let dir = document_assets_dir(&document_path(&relative_path)?)?;
    crate::runtime::blocking::run(move || {
        let bytes = crate::harness::attach::read_dropped(&path)?;
        crate::harness::attach::write_image(&dir, &bytes).map(|name| document_asset_ref(&name))
    })
    .await
}
