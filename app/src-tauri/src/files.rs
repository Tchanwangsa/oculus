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
    step_aside(dir, name, |existing| existing == bytes)
}

/// The step-aside rule itself: `name`, else `stem-2.ext`, `stem-3.ext`… —
/// the first that nothing occupies, or that `ours` says is occupied by the
/// very file being placed. Uploads pass a byte comparison; a document passes
/// `|_| false`, because two empty notes are two notes, not one twice.
fn step_aside(dir: &Path, name: &str, ours: impl Fn(&[u8]) -> bool) -> String {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    for n in 1..1000 {
        let candidate = if n == 1 { name.to_string() } else { format!("{stem}-{n}{ext}") };
        match std::fs::read(dir.join(&candidate)) {
            Err(_) => return candidate,
            Ok(existing) if ours(&existing) => return candidate,
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

// ── The student's own documents ───────────────────────────────────────────────
//
// A document is a markdown note written inside the app. It lives in
// `courses/<code>/documents/`, beside `uploads/`, and works the same way: Rust
// moves the bytes, the frontend writes the `files` row, and from then on the
// path is all the rest of the app needs — `read_course_file`, the mention
// menu, search and the chat agent's `courses/` reach it with no further work.
//
// Two things an upload never does. A document is rewritten on every save, and
// it moves when its title changes — so its relative path is its identity, and
// every command that mutates one takes that path and checks its shape
// (`is_document_rel`) before resolving it against the data dir. The caller's
// string never becomes a filesystem path on its own.

/// The filename a title earns: sanitised the way every course path is,
/// `Untitled` when nothing survives the sanitising, and `.md` always.
///
/// "Survives" means a character that is not the underscore every unsafe one
/// becomes — `???` earns `Untitled.md`, not `___.md`, for the same reason
/// `safe_rel_path` drops a bare `_` segment. A trailing dot goes too:
/// `safe_filename` collapses `..` but leaves a lone one, and `notes.` + `.md`
/// would be `notes..md` — which `write_course_bytes` sanitises again on the
/// way to disk, landing the file under a name other than the one announced.
fn document_name(title: &str) -> String {
    let stem = crate::paths::safe_filename(title.trim());
    let stem = stem.trim_end_matches('.');
    let stem = if stem.trim_matches('_').is_empty() { "Untitled" } else { stem };
    format!("{stem}.md")
}

fn documents_dir(data_dir: &Path, code: &str) -> std::path::PathBuf {
    data_dir
        .join("courses")
        .join(crate::paths::safe_dir(code))
        .join(crate::paths::DOCUMENTS_DIR)
}

/// A document's absolute path, or an error — the one place the guard runs.
fn document_path(app: &AppHandle, relative_path: &str) -> Result<std::path::PathBuf, String> {
    if !crate::paths::is_document_rel(relative_path) {
        return Err(format!("{relative_path} is not one of your documents"));
    }
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(base.join(relative_path))
}

/// Where a note's pictures live: `assets/`, in the folder the note is in.
///
/// Beside the note and not in `agents/attachments/`, where a composer's
/// picture goes, because a note is a *library* file and what it holds has to
/// resolve for everything that reads it. The editor's preview and the file
/// viewer both resolve a relative image against the note's own directory
/// (`useLibraryMdComponents` in `app/src/components/files/FileViewer.tsx`),
/// and an agent — or a text editor, or Finder — handed `documents/` is handed
/// the pictures with it.
///
/// The argument is the note's *guarded* path, so the folder can only ever be
/// under some subject's `documents/`: `document_path` is what proves that,
/// and this only walks up one level from what it returned.
fn document_assets_dir(note: &Path) -> Result<std::path::PathBuf, String> {
    let dir = note.parent().ok_or_else(|| format!("{} has no folder", note.display()))?;
    Ok(dir.join(crate::paths::DOCUMENT_ASSETS_DIR))
}

/// What the editor puts in the note: `assets/<name>`, relative to the note
/// itself. A rename moves the note inside the same folder, so the link it
/// carries keeps resolving without being rewritten.
fn document_asset_ref(name: &str) -> String {
    format!("{}/{name}", crate::paths::DOCUMENT_ASSETS_DIR)
}

fn document_file(relative_path: String, size_bytes: u64) -> ImportedFile {
    let filename = relative_path.rsplit('/').next().unwrap_or_default().to_string();
    ImportedFile { filename, relative_path, file_type: "md".to_string(), size_bytes }
}

/// Whether `wanted` already names the file at `current`. On a
/// case-insensitive volume — the macOS default — a title that only changed
/// case would otherwise be found on disk by the step-aside rule and moved
/// from `Notes.md` to `Notes-2.md` for no reason.
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

/// Create an empty note under a subject. The frontend writes its row from the
/// result, exactly as it does for an upload.
#[tauri::command]
pub fn create_document(
    app: AppHandle,
    subject_code: String,
    title: String,
) -> Result<ImportedFile, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let dir = documents_dir(&data_dir, &subject_code);
    // Unconditional: an empty file already under this name is somebody's note
    // with nothing in it yet, not this one arriving twice.
    let name = step_aside(&dir, &document_name(&title), |_| false);
    let (rel, size, _) = crate::paths::write_course_bytes(
        &data_dir,
        &subject_code,
        &format!("{}/{name}", crate::paths::DOCUMENTS_DIR),
        b"",
    )?;
    Ok(document_file(rel, size))
}

/// Save a note's text. Returns the byte count, which is what the row's
/// `size_bytes` holds.
#[tauri::command]
pub fn write_document(
    app: AppHandle,
    relative_path: String,
    content: String,
) -> Result<u64, String> {
    let path = document_path(&app, &relative_path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, content.as_bytes()).map_err(|e| e.to_string())?;
    Ok(content.len() as u64)
}

/// Give a note a new title, which is a new filename. The same title yields
/// the same file untouched; a title another note already holds steps aside
/// like a colliding upload does.
#[tauri::command]
pub fn rename_document(
    app: AppHandle,
    relative_path: String,
    title: String,
) -> Result<ImportedFile, String> {
    let path = document_path(&app, &relative_path)?;
    let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    let (dir_rel, current) = relative_path
        .rsplit_once('/')
        .ok_or_else(|| format!("{relative_path} has no filename"))?;
    let wanted = document_name(&title);
    if wanted == current {
        return Ok(document_file(relative_path, size));
    }
    let dir = path.parent().ok_or_else(|| format!("{relative_path} has no folder"))?;
    let name = if same_entry(&path, &dir.join(&wanted)) {
        wanted
    } else {
        step_aside(dir, &wanted, |_| false)
    };
    std::fs::rename(&path, dir.join(&name)).map_err(|e| e.to_string())?;
    Ok(document_file(format!("{dir_rel}/{name}"), size))
}

/// Remove a note. Nothing is derived from a markdown file — no parse, no
/// embeddings — so the file is the whole of it.
#[tauri::command]
pub fn delete_document(app: AppHandle, relative_path: String) -> Result<(), String> {
    let path = document_path(&app, &relative_path)?;
    std::fs::remove_file(path).map_err(|e| e.to_string())
}

/// Every note in a subject's folder, by name. This is what the Documents tab
/// reconciles its rows against, so a note something else wrote straight into
/// the folder — the chat agent, the student in a text editor — shows up too.
/// Only names the guard would accept are listed: a file it would refuse to
/// save is not one the tab can offer to edit.
#[tauri::command]
pub fn list_documents(app: AppHandle, subject_code: String) -> Result<Vec<ImportedFile>, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let dir = documents_dir(&data_dir, &subject_code);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let dir_rel =
        format!("courses/{}/{}", crate::paths::safe_dir(&subject_code), crate::paths::DOCUMENTS_DIR);
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else { continue };
        let rel = format!("{dir_rel}/{name}");
        if !crate::paths::is_document_rel(&rel) {
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

/// A picture pasted into a note, written beside it and answered as the path
/// the note links it by.
///
/// The picture is written *now*, while the note is still being typed, rather
/// than deferred the way a composer defers its attachments until send: there
/// is no send here, and an image tag cannot point at a file that will be
/// written later. The cost is a file left in `assets/` when its tag is then
/// deleted from the text, and that is the accepted trade — the same one
/// `TaskPage` makes for a task body.
///
/// The bytes are the clipboard's, base64 across the IPC, and everything that
/// is true of a composer's picture is true here: the cap, the sniff, and a
/// filename that is this app's own stamp rather than anything the caller
/// claimed (`crate::harness::attach`).
#[tauri::command]
pub async fn attach_document_image(
    app: AppHandle,
    relative_path: String,
    data: String,
) -> Result<String, String> {
    let dir = document_assets_dir(&document_path(&app, &relative_path)?)?;
    let bytes = crate::harness::attach::decode(&data)?;
    tokio::task::spawn_blocking(move || {
        crate::harness::attach::write_image(&dir, &bytes).map(|name| document_asset_ref(&name))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The same, for a picture dropped onto a note from Finder: the OS hands the
/// webview a path, so the bytes are read here instead of crossing the IPC.
#[tauri::command]
pub async fn attach_document_file(
    app: AppHandle,
    relative_path: String,
    path: String,
) -> Result<String, String> {
    let dir = document_assets_dir(&document_path(&app, &relative_path)?)?;
    tokio::task::spawn_blocking(move || {
        let bytes = crate::harness::attach::read_dropped(&path)?;
        crate::harness::attach::write_image(&dir, &bytes).map(|name| document_asset_ref(&name))
    })
    .await
    .map_err(|e| e.to_string())?
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

    /// A note's pictures sit in one `assets/` folder beside it — and that
    /// folder is not itself reachable as a note, which is what keeps a save
    /// or a delete off a picture and keeps the Documents list free of one.
    #[test]
    fn a_note_s_pictures_sit_beside_it() {
        let note = Path::new("/data/courses/MULT20015/documents/Week 3.md");
        assert_eq!(
            document_assets_dir(note).unwrap(),
            Path::new("/data/courses/MULT20015/documents/assets")
        );
        // Two notes in a subject share the folder: the names are stamps.
        let other = Path::new("/data/courses/MULT20015/documents/Ideas.md");
        assert_eq!(document_assets_dir(note).unwrap(), document_assets_dir(other).unwrap());

        assert_eq!(document_asset_ref("20260922-101112-0a1b2c3d.png"), "assets/20260922-101112-0a1b2c3d.png");
        assert!(!crate::paths::is_document_rel(
            "courses/MULT20015/documents/assets/20260922-101112-0a1b2c3d.png"
        ));
    }

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

    #[test]
    fn a_title_becomes_a_markdown_filename() {
        assert_eq!(document_name("Week 3 notes"), "Week_3_notes.md");
        assert_eq!(document_name("  padded  "), "padded.md");
        // Nothing to name it by: not an empty stem, and not a dotfile.
        assert_eq!(document_name(""), "Untitled.md");
        assert_eq!(document_name("   "), "Untitled.md");
        assert_eq!(document_name("..."), "Untitled.md");
        assert_eq!(document_name("???"), "Untitled.md");
        // A trailing dot would read as `..md` once the extension lands.
        assert_eq!(document_name("Draft."), "Draft.md");
        // Separators and traversal are sanitised, never honoured.
        assert_eq!(document_name("../../etc/passwd"), "____etc_passwd.md");
        assert_eq!(document_name("a/b"), "a_b.md");
    }

    #[test]
    fn a_new_document_never_lands_on_a_name_already_taken() {
        let dir =
            std::env::temp_dir().join(format!("oculus-documents-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let name = document_name("notes");
        assert_eq!(step_aside(&dir, &name, |_| false), "notes.md");
        std::fs::write(dir.join("notes.md"), b"").unwrap();

        // An empty note under the name is not "the same file again": a second
        // note with the same title is a second note.
        assert_eq!(step_aside(&dir, &name, |_| false), "notes-2.md");
        std::fs::write(dir.join("notes-2.md"), b"some text").unwrap();
        assert_eq!(step_aside(&dir, &name, |_| false), "notes-3.md");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
