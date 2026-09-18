use std::path::Path;

use tauri::{AppHandle, Manager};

/// Live cookies from the login WebView (only available while it's open).
pub fn canvas_cookie_header(app: &AppHandle) -> String {
    let Some(win) = app.get_webview_window("canvas-auth") else {
        return String::new();
    };
    match win.cookies() {
        Ok(cookies) => cookies
            .iter()
            .map(|c| format!("{}={}", c.name(), c.value()))
            .collect::<Vec<_>>()
            .join("; "),
        Err(e) => {
            eprintln!("[oculus] cookies() failed: {e}");
            String::new()
        }
    }
}

/// Cookie to use for server-side Canvas requests. Prefers the persisted
/// snapshot (survives restart); falls back to the live login WebView if it
/// happens to be open and nothing was saved yet.
pub fn proxy_cookie(app: &AppHandle) -> String {
    let saved = crate::auth::saved_cookie_header(app);
    if !saved.is_empty() {
        return saved;
    }
    canvas_cookie_header(app)
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn read_course_file(app: AppHandle, relative_path: String) -> Result<String, String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join(&relative_path);
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_course_file(app: AppHandle, relative_path: String) -> Result<(), String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join(&relative_path);
    tauri_plugin_opener::open_path(path.to_str().unwrap_or(""), None::<&str>)
        .map_err(|e| e.to_string())
}

// ── The student's own files ───────────────────────────────────────────────────
//
// An upload is a library file that no sync put there. It is copied into
// `courses/<code>/uploads/`, which is enough for the entire pipeline to pick it
// up: the Office converter, the parser, the embedder, search and the chat
// agent all key off the path and know nothing about where the bytes came from.

/// One file that landed, in the shape the frontend needs to write its row.
#[derive(serde::Serialize)]
pub struct ImportedFile {
    pub filename: String,
    pub relative_path: String,
    pub file_type: String,
    pub size_bytes: u64,
}

/// What became of one picked file. `file` and `error` are both set when the
/// bytes landed but the PDF conversion did not: the row is real and the
/// original opens, it just has nothing for the parser to read.
#[derive(serde::Serialize)]
pub struct ImportOutcome {
    /// The name the user picked it under, so a failure can name itself.
    pub source: String,
    pub file: Option<ImportedFile>,
    pub error: Option<String>,
}

/// Copy files the user picked into a subject's uploads folder.
///
/// Per-file results rather than one `Result`: picking six files and having the
/// fifth fail must still leave the other five in the library.
#[tauri::command]
pub fn import_uploads(
    app: AppHandle,
    subject_code: String,
    paths: Vec<String>,
) -> Result<Vec<ImportOutcome>, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(paths
        .iter()
        .map(|p| {
            let src = Path::new(p);
            let source = src
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| p.clone());
            match store_upload(&data_dir, &subject_code, src) {
                Ok((file, error)) => ImportOutcome { source, file: Some(file), error },
                Err(e) => ImportOutcome { source, file: None, error: Some(e) },
            }
        })
        .collect())
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
        .join(crate::paths::safe_dir(code))
        .join(crate::paths::UPLOADS_DIR);
    let name = free_name(&dir, &crate::paths::safe_filename(picked), &bytes);

    let course_rel = format!("{}/{name}", crate::paths::UPLOADS_DIR);
    let (rel, size, action) =
        crate::paths::write_course_bytes(data_dir, code, &course_rel, &bytes)?;

    // A name freed by a delete can be handed out again, and a quality pass that
    // was still in flight when that delete landed writes its `{stem}.md` out
    // afterwards — beside a PDF that no longer exists. Whatever is sitting on
    // this name is therefore not necessarily ours. `Unchanged` is the one case
    // it provably is: identical bytes under the same name, keeping the parse
    // and embeddings they already earned.
    if action != crate::paths::WriteAction::Unchanged {
        crate::paths::purge_parse_artifacts(data_dir, &rel);
    }

    // The derived sibling PDF the scraper writes for Office documents, written
    // here for the same reason: it is what the parser, the embedder and the
    // in-app viewer actually read (`doc_pdf_rel` in paths.rs).
    let warning = match crate::sync::office_ext_of(&name) {
        None => None,
        Some(ext) => match crate::sync::office_to_pdf(&bytes, ext) {
            Ok(pdf) => {
                crate::paths::write_course_bytes(
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

    Ok((ImportedFile { filename: name, relative_path: rel, file_type, size_bytes: size }, warning))
}

/// A name in `dir` these bytes may have.
///
/// An upload never overwrites one already there — a second `notes.pdf` becomes
/// `notes-2.pdf`, so adding the wrong file cannot destroy the right one.
/// Identical bytes under the same name are the one exception: that is the same
/// file again, and it keeps its row, its parse and its embeddings instead of
/// growing a copy.
fn free_name(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    for n in 1..1000 {
        let candidate = if n == 1 { name.to_string() } else { format!("{stem}-{n}{ext}") };
        match std::fs::read(dir.join(&candidate)) {
            Err(_) => return candidate,
            Ok(existing) if existing == bytes => return candidate,
            Ok(_) => {}
        }
    }
    name.to_string()
}

/// Remove an uploaded file and everything derived from it.
///
/// Scoped to `uploads/` by `is_upload_rel` — see the note there. It takes the
/// converted PDF, the parse artifacts and the page images with it, because the
/// sidecar's skip checks are plain existence checks: a leftover `{stem}.md`
/// would be served as the parse of whatever lands on that name next.
#[tauri::command]
pub fn delete_upload(app: AppHandle, relative_path: String) -> Result<(), String> {
    if !crate::paths::is_upload_rel(&relative_path) {
        return Err(format!("{relative_path} is not one of your uploads"));
    }
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;

    crate::paths::purge_parse_artifacts(&base, &relative_path);
    // The converted sibling, for an Office document. `doc_pdf_rel` returns the
    // file itself for a real PDF, which the final remove already covers.
    if let Some(pdf_rel) = crate::paths::doc_pdf_rel(&relative_path) {
        if pdf_rel != relative_path {
            let _ = std::fs::remove_file(base.join(&pdf_rel));
        }
    }
    std::fs::remove_file(base.join(&relative_path)).map_err(|e| e.to_string())
}

/// Derive parse status from disk for a set of PDF-backed relative paths
/// (PDFs, plus Office files parsed via their derived sibling PDF).
/// Returns (relative_path, status); paths with no parse output are omitted.
#[tauri::command]
pub fn scan_parsed_files(
    app: AppHandle,
    relative_paths: Vec<String>,
) -> Result<Vec<(String, String)>, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(relative_paths
        .into_iter()
        .filter_map(|rel| {
            let pdf_rel = crate::paths::doc_pdf_rel(&rel)?;
            crate::parse::parse_mode(&base.join(&pdf_rel)).map(|mode| (rel, mode.to_string()))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_upload_never_lands_on_a_name_already_taken() {
        let dir = std::env::temp_dir().join(format!("oculus-uploads-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Nothing there: the picked name is the name.
        assert_eq!(free_name(&dir, "notes.pdf", b"one"), "notes.pdf");
        std::fs::write(dir.join("notes.pdf"), b"one").unwrap();

        // The same file again is the same file, not a second copy.
        assert_eq!(free_name(&dir, "notes.pdf", b"one"), "notes.pdf");
        // A different file under a taken name steps aside rather than overwrite.
        assert_eq!(free_name(&dir, "notes.pdf", b"two"), "notes-2.pdf");
        // Extensionless names count too, and a dotfile is all stem.
        assert_eq!(free_name(&dir, "README", b"x"), "README");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
