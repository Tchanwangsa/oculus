//! Files and folders brought in from outside the library: copied into one of
//! the student's folders (`targetDir`, else the subject's root), a folder
//! with everything inside it. The rest of the pipeline keys off the path.

use std::path::Path;

use crate::library::drive::ops::guard::{own_target, Target};
use crate::library::drive::ops::names::{entry_name, free_name};
use crate::library::layout::{self, OwnKind};

/// A picked folder is read this many folders down.
const MAX_DEPTH: usize = 32;

/// One file that landed, in the shape the frontend writes its row from.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ImportedFile {
    pub filename: String,
    pub relative_path: String,
    pub file_type: String,
    pub size_bytes: u64,
    /// The row's `files.category`, from its path (`layout::category_of`):
    /// a dropped `.md` is a note, a `.canvas` a canvas.
    pub category: String,
}

impl ImportedFile {
    pub fn new(relative_path: String, file_type: &str, size_bytes: u64) -> Self {
        Self {
            filename: relative_path
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string(),
            category: layout::category_of(layout::within_course(&relative_path)).to_string(),
            relative_path,
            file_type: file_type.to_string(),
            size_bytes,
        }
    }
}

/// What became of one picked file. `file` and `error` are both set when the
/// bytes landed but the PDF conversion did not.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ImportOutcome {
    /// The name the user picked it under; inside a picked folder, its path
    /// from that folder (`Week 1/slides.pdf`).
    pub source: String,
    pub file: Option<ImportedFile>,
    pub error: Option<String>,
}

/// Copy picked files and folders into `target_dir` (data-dir-relative, in
/// the subject), or its root; one outcome per file, and one failure does not
/// fail the rest. A target that is not the student's is the whole command's
/// typed error (`DriveError`).
#[tauri::command]
pub async fn import_uploads(
    subject_code: String,
    paths: Vec<String>,
    target_dir: Option<String>,
) -> Result<Vec<ImportOutcome>, String> {
    crate::runtime::blocking::run(move || {
        let data_dir = crate::library::paths::data_dir();
        let target = own_target(&data_dir, &subject_code, target_dir.as_deref())?;
        let paths: Vec<&Path> = paths.iter().map(Path::new).collect();
        Ok(import_into(&data_dir, &target, &paths))
    })
    .await
}

fn picked_name(src: &Path) -> String {
    src.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| src.display().to_string())
}

/// Stores each of `paths` in `target`, the one way files enter the student's
/// folders from outside (picked files and folders, Finder drops, browser
/// downloads). Imports share name allocation, so one lock keeps simultaneous
/// batches from choosing the same unused name while conversion runs.
pub(crate) fn import_into(data_dir: &Path, target: &Target, paths: &[&Path]) -> Vec<ImportOutcome> {
    static IMPORT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = IMPORT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut out = Vec::new();
    for src in paths {
        let source = picked_name(src);
        if std::fs::metadata(src).is_ok_and(|m| m.is_dir()) {
            import_folder(data_dir, target, src, &source, 0, &mut out);
        } else {
            out.push(import_one(data_dir, target, src, source));
        }
    }
    out
}

fn import_one(data_dir: &Path, target: &Target, src: &Path, source: String) -> ImportOutcome {
    match store_upload(data_dir, target, src) {
        Ok((file, error)) => ImportOutcome {
            source,
            file: Some(file),
            error,
        },
        Err(e) => ImportOutcome {
            source,
            file: None,
            error: Some(e),
        },
    }
}

/// A picked folder as a new folder in `target` (` (2)` when the name is
/// taken), everything in it below. Dot-names and symlinks are left out.
fn import_folder(
    data_dir: &Path,
    target: &Target,
    src: &Path,
    source: &str,
    depth: usize,
    out: &mut Vec<ImportOutcome>,
) {
    let name = free_name(&target.path, &folder_name(target, src), true, |_| false);
    let folder = Target {
        rel: target.child_rel(&name),
        path: target.path.join(&name),
    };
    if let Err(e) = std::fs::create_dir(&folder.path) {
        out.push(ImportOutcome {
            source: source.to_string(),
            file: None,
            error: Some(format!("could not make the folder — {e}")),
        });
        return;
    }
    let Ok(read) = std::fs::read_dir(src) else {
        return;
    };
    let mut children: Vec<_> = read
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter(|e| e.file_type().is_ok_and(|t| !t.is_symlink()))
        .collect();
    children.sort_by_key(|e| e.file_name());
    for child in children {
        let child_source = format!("{source}/{}", child.file_name().to_string_lossy());
        let path = child.path();
        if child.file_type().is_ok_and(|t| t.is_dir()) {
            if depth + 1 < MAX_DEPTH {
                import_folder(data_dir, &folder, &path, &child_source, depth + 1, out);
            }
        } else {
            out.push(import_one(data_dir, &folder, &path, child_source));
        }
    }
}

/// A picked folder's name; at a subject's root never one the layout keeps.
fn folder_name(target: &Target, src: &Path) -> String {
    let name = entry_name(&picked_name(src));
    if target.is_subject_root() && layout::reserved_name(&name) {
        format!("{name} (2)")
    } else {
        name
    }
}

fn store_upload(
    data_dir: &Path,
    target: &Target,
    src: &Path,
) -> Result<(ImportedFile, Option<String>), String> {
    let picked = src
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "that file has no name".to_string())?;
    let bytes = std::fs::read(src).map_err(|e| format!("could not read it — {e}"))?;

    let name = free_name(
        &target.path,
        &upload_name(target, picked),
        false,
        |existing| std::fs::read(existing).is_ok_and(|b| b == bytes),
    );
    let rel = target.child_rel(&name);
    let path = target.path.join(&name);
    let unchanged = std::fs::read(&path).is_ok_and(|b| b == bytes);

    // A reused name may hold a stale parse (a pass in flight across a delete).
    // Only identical bytes provably keep their own.
    if !unchanged {
        std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
        crate::library::paths::purge_parse_artifacts(data_dir, &rel);
    }

    // The derived sibling PDF for Office documents (`doc_pdf_rel` in library/paths/file_types.rs).
    // A spreadsheet's text is written by the parse kick the frontend sends once
    // the row exists (`parse_file` → `crate::pages::sheets`).
    let warning = match crate::sync::office_ext_of(&name) {
        None => None,
        Some(ext) => match crate::sync::office_to_pdf(&bytes, ext) {
            Ok(pdf) => {
                std::fs::write(target.path.join(format!("{name}.pdf")), &pdf)
                    .map_err(|e| e.to_string())?;
                None
            }
            Err(e) => Some(format!("added, but not converted for search — {e}")),
        },
    };

    let file_type = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    Ok((
        ImportedFile::new(rel, &file_type, bytes.len() as u64),
        warning,
    ))
}

/// A picked file's name in the library (`entry_name`: never a dot-name,
/// which the own-file guard refuses, so it could never be deleted); at a
/// subject's root never a name the layout keeps.
fn upload_name(target: &Target, picked: &str) -> String {
    let name = entry_name(picked);
    if target.is_subject_root() && layout::reserved_name(&name) {
        format!("{name} (2)")
    } else {
        name
    }
}

/// `name`, else `stem-2.ext`, `stem-3.ext`… — the first free, or that `ours`
/// says already holds this file.
pub(super) fn step_aside(dir: &Path, name: &str, ours: impl Fn(&[u8]) -> bool) -> String {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    for n in 1..1000 {
        let candidate = if n == 1 {
            name.to_string()
        } else {
            format!("{stem}-{n}{ext}")
        };
        match std::fs::read(dir.join(&candidate)) {
            Err(_) => return candidate,
            Ok(existing) if ours(&existing) => return candidate,
            Ok(_) => {}
        }
    }
    name.to_string()
}

/// Remove an upload and everything derived from it: the parse's skip checks
/// are existence checks, so a leftover `{stem}.md` would be served as the
/// parse of whatever lands on that name next.
#[tauri::command]
pub fn delete_upload(relative_path: String) -> Result<(), String> {
    if !layout::own_file_ok(&relative_path, OwnKind::Upload) {
        return Err(format!("{relative_path} is not one of your uploads"));
    }
    let base = crate::library::paths::data_dir();

    crate::library::paths::purge_parse_artifacts(&base, &relative_path);
    // The converted sibling of an Office document (a real PDF is itself).
    if let Some(pdf_rel) = crate::library::paths::doc_pdf_rel(&relative_path) {
        if pdf_rel != relative_path {
            let _ = std::fs::remove_file(base.join(&pdf_rel));
        }
    }
    std::fs::remove_file(base.join(&relative_path)).map_err(|e| e.to_string())
}
