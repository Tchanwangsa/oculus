//! Markdown notes in `courses/<code>/documents/`: Rust moves the bytes, the
//! frontend writes the `files` row. A note is rewritten on save and moves on
//! rename, so every mutating command checks the path's shape (`is_document_rel`)
//! before resolving it.

use std::path::Path;

use super::uploads::step_aside;
use super::ImportedFile;

/// A title's filename: sanitised, `Untitled` when only underscores survive,
/// `.md` always. A trailing dot is dropped, or `notes..md` would be sanitised
/// again on write and land under a different name.
pub(super) fn document_name(title: &str) -> String {
    let stem = crate::library::paths::safe_filename(title.trim());
    let stem = stem.trim_end_matches('.');
    let stem = if stem.trim_matches('_').is_empty() {
        "Untitled"
    } else {
        stem
    };
    format!("{stem}.md")
}

fn documents_dir(data_dir: &Path, code: &str) -> std::path::PathBuf {
    data_dir
        .join("courses")
        .join(crate::library::paths::safe_dir(code))
        .join(crate::library::paths::DOCUMENTS_DIR)
}

/// A document's absolute path, or an error — the one place the guard runs.
fn document_path(relative_path: &str) -> Result<std::path::PathBuf, String> {
    if !crate::library::paths::is_document_rel(relative_path) {
        return Err(format!("{relative_path} is not one of your documents"));
    }
    Ok(crate::library::paths::data_dir().join(relative_path))
}

/// A note's pictures: `assets/` beside it, so they resolve relative to the note
/// for every reader (`useLibraryMdComponents` in `FileViewer.tsx`). Takes the
/// path `document_path` guarded.
pub(super) fn document_assets_dir(note: &Path) -> Result<std::path::PathBuf, String> {
    let dir = note
        .parent()
        .ok_or_else(|| format!("{} has no folder", note.display()))?;
    Ok(dir.join(crate::library::paths::DOCUMENT_ASSETS_DIR))
}

/// The link the editor writes, relative to the note (survives a rename).
pub(super) fn document_asset_ref(name: &str) -> String {
    format!("{}/{name}", crate::library::paths::DOCUMENT_ASSETS_DIR)
}

fn document_file(relative_path: String, size_bytes: u64) -> ImportedFile {
    let filename = relative_path
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_string();
    ImportedFile {
        filename,
        relative_path,
        file_type: "md".to_string(),
        size_bytes,
    }
}

/// Whether `wanted` already names the file at `current` — on a
/// case-insensitive volume a case-only rename would otherwise step aside.
fn same_entry(current: &Path, wanted: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::metadata(current), std::fs::metadata(wanted)) {
            (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (current, wanted);
        false
    }
}

/// Create an empty note under a subject.
#[tauri::command]
pub fn create_document(subject_code: String, title: String) -> Result<ImportedFile, String> {
    let data_dir = crate::library::paths::data_dir();
    let dir = documents_dir(&data_dir, &subject_code);
    // Never "ours": an existing empty note is still another note.
    let name = step_aside(&dir, &document_name(&title), |_| false);
    let (rel, size, _) = crate::library::paths::write_course_bytes(
        &data_dir,
        &subject_code,
        &format!("{}/{name}", crate::library::paths::DOCUMENTS_DIR),
        b"",
    )?;
    Ok(document_file(rel, size))
}

/// The hidden sibling a note's text is staged in. It ends in `.tmp`, never
/// `.md`, so `is_document_rel` rejects a leftover and it never becomes a row.
pub(super) fn note_temp_path(note: &Path) -> Result<std::path::PathBuf, String> {
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

/// Remove a note (nothing is derived from it).
#[tauri::command]
pub fn delete_document(relative_path: String) -> Result<(), String> {
    let path = document_path(&relative_path)?;
    std::fs::remove_file(path).map_err(|e| e.to_string())
}

/// Every note in a subject's folder, for the Documents tab to reconcile
/// against — including ones written outside the app. Only guard-accepted names.
#[tauri::command]
pub fn list_documents(subject_code: String) -> Result<Vec<ImportedFile>, String> {
    let data_dir = crate::library::paths::data_dir();
    let dir = documents_dir(&data_dir, &subject_code);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let dir_rel = format!(
        "courses/{}/{}",
        crate::library::paths::safe_dir(&subject_code),
        crate::library::paths::DOCUMENTS_DIR
    );
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let rel = format!("{dir_rel}/{name}");
        if !crate::library::paths::is_document_rel(&rel) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        out.push(document_file(rel, meta.len()));
    }
    out.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(out)
}

/// A picture pasted into a note (base64), written now into `assets/` and
/// returned as the link. A tag later deleted leaves an orphan file — accepted.
/// Cap, sniff and naming are `crate::harness::attach`'s.
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
