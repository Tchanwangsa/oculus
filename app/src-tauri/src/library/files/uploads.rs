//! Copied into `courses/<code>/uploads/`; the rest of the pipeline keys off the
//! path alone.

use std::path::Path;

/// One file that landed, in the shape the frontend writes its row from.
#[derive(serde::Serialize)]
pub struct ImportedFile {
    pub filename: String,
    pub relative_path: String,
    pub file_type: String,
    pub size_bytes: u64,
}

/// What became of one picked file. `file` and `error` are both set when the
/// bytes landed but the PDF conversion did not.
#[derive(serde::Serialize)]
pub struct ImportOutcome {
    /// The name the user picked it under.
    pub source: String,
    pub file: Option<ImportedFile>,
    pub error: Option<String>,
}

/// Copy picked files into a subject's uploads folder; one failure does not
/// fail the rest.
#[tauri::command]
pub async fn import_uploads(
    subject_code: String,
    paths: Vec<String>,
) -> Result<Vec<ImportOutcome>, String> {
    crate::runtime::blocking::run(move || {
        // Imports share name allocation; keep simultaneous batches from choosing
        // the same unused upload name while conversion runs off the command thread.
        static IMPORT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = IMPORT_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let data_dir = crate::library::paths::data_dir();
        Ok(paths
            .iter()
            .map(|p| {
                let src = Path::new(p);
                let source = src
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.clone());
                match store_upload(&data_dir, &subject_code, src) {
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
            })
            .collect())
    })
    .await
}

fn store_upload(
    data_dir: &Path,
    code: &str,
    src: &Path,
) -> Result<(ImportedFile, Option<String>), String> {
    let picked = src
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "that file has no name".to_string())?;
    let bytes = std::fs::read(src).map_err(|e| format!("could not read it — {e}"))?;

    let dir = data_dir
        .join("courses")
        .join(crate::library::paths::safe_dir(code))
        .join(crate::library::paths::UPLOADS_DIR);
    let name = free_name(&dir, &crate::library::paths::safe_filename(picked), &bytes);

    let course_rel = format!("{}/{name}", crate::library::paths::UPLOADS_DIR);
    let (rel, size, action) =
        crate::library::paths::write_course_bytes(data_dir, code, &course_rel, &bytes)?;

    // A reused name may hold a stale parse (a pass in flight across a delete).
    // Only `Unchanged` — identical bytes — provably keeps its own.
    if action != crate::library::paths::WriteAction::Unchanged {
        crate::library::paths::purge_parse_artifacts(data_dir, &rel);
    }

    // The derived sibling PDF for Office documents (`doc_pdf_rel` in library/paths/file_types.rs).
    // A spreadsheet's text is written by the parse kick the frontend sends once
    // the row exists (`parse_file` → `crate::pages::sheets`).
    let warning = match crate::sync::office_ext_of(&name) {
        None => None,
        Some(ext) => match crate::sync::office_to_pdf(&bytes, ext) {
            Ok(pdf) => {
                crate::library::paths::write_course_bytes(
                    data_dir,
                    code,
                    &format!("{course_rel}.pdf"),
                    &pdf,
                )?;
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
        ImportedFile {
            filename: name,
            relative_path: rel,
            file_type,
            size_bytes: size,
        },
        warning,
    ))
}

/// A name in `dir` for these bytes: never overwrites a different file
/// (`notes-2.pdf`), but identical bytes reuse the name and keep their parse.
pub(super) fn free_name(dir: &Path, name: &str, bytes: &[u8]) -> String {
    step_aside(dir, name, |existing| existing == bytes)
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
    if !crate::library::paths::is_upload_rel(&relative_path) {
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
