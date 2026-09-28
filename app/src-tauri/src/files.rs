use std::io::Write;
use std::path::{Path, PathBuf};

// Imports and deletes share a namespace with generated PDF/parse siblings.
// Serialize allocation and deletion so two windows cannot claim the same name.
static UPLOAD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const DELETED_NAMES_DIR: &str = ".oculus-upload-reservations";

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
pub async fn import_uploads(
    app: AppHandle,
    subject_code: String,
    paths: Vec<String>,
) -> Result<Vec<ImportOutcome>, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    // Office conversion can take seconds per file. Keep the native window and
    // progress UI responsive while file I/O and LibreOffice run.
    tauri::async_runtime::spawn_blocking(move || paths
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
        .await.map_err(|e| format!("upload worker stopped — {e}"))
}

fn store_upload(
    data_dir: &Path,
    code: &str,
    src: &Path,
) -> Result<(ImportedFile, Option<String>), String> {
    let _guard = UPLOAD_LOCK.lock().map_err(|_| "upload lock unavailable")?;
    let picked = src
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "that file has no name".to_string())?;
    let bytes = std::fs::read(src).map_err(|e| format!("could not read it — {e}"))?;

    let code = crate::paths::safe_dir(code);
    if code.is_empty() {
        return Err("that subject has no code".into());
    }
    let dir = upload_dir(data_dir, &code, true)?;
    let name = free_name(&dir, &crate::paths::safe_filename(picked), &bytes)?;
    for name in owned_names(&name) {
        reject_link(&dir.join(name))?;
    }

    let course_rel = format!("{}/{name}", crate::paths::UPLOADS_DIR);
    let rel = format!("courses/{code}/{course_rel}");
    let target = dir.join(&name);
    reject_link(&target)?;
    if !target.exists() {
        // Even another process claiming the path between allocation and this
        // write must never turn an import into an overwrite.
        write_new_file(&target, &bytes).map_err(|e| format!("could not add it — {e}"))?;
    }

    // The derived sibling PDF the scraper writes for Office documents, written
    // here for the same reason: it is what the parser, the embedder and the
    // in-app viewer actually read (`doc_pdf_rel` in paths.rs).
    let warning = match crate::sync::office_ext_of(&name) {
        None => None,
        Some(_) if dir.join(format!("{name}.pdf")).is_file() => None,
        Some(ext) => match crate::sync::office_to_pdf(&bytes, ext) {
            Ok(pdf) => {
                let result = write_new_file(&dir.join(format!("{name}.pdf")), &pdf);
                result.err().map(|e| format!("added, but could not save its PDF — {e}"))
            }
            Err(e) => Some(format!("added, but not converted for search — {e}")),
        },
    };

    let file_type = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();

    Ok((ImportedFile { filename: name, relative_path: rel, file_type, size_bytes: bytes.len() as u64 }, warning))
}

fn write_new_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(error) = file.write_all(bytes) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}

/// Reject reparse points as well as Rust symlinks: Windows directory junctions
/// are reparse points, but not necessarily reported as `is_symlink()`.
fn reject_link(path: &Path) -> Result<(), String> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    #[cfg(windows)]
    let reparse = {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let reparse = false;
    if meta.file_type().is_symlink() || reparse {
        return Err("uploads cannot follow a symbolic link or junction".into());
    }
    Ok(())
}

fn upload_dir(data_dir: &Path, code: &str, create: bool) -> Result<PathBuf, String> {
    // Preserve the library's logical root. Windows package virtualization can
    // resolve a database file physically without relocating its sibling files.
    let mut dir = if data_dir.is_absolute() { data_dir.to_path_buf() } else {
        std::env::current_dir().map_err(|e| e.to_string())?.join(data_dir)
    };
    if !dir.is_dir() { return Err("the library folder is unavailable".into()); }
    for component in ["courses", code, crate::paths::UPLOADS_DIR] {
        dir.push(component);
        reject_link(&dir)?;
        if create && !dir.exists() {
            std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
        }
        reject_link(&dir)?;
        if !dir.is_dir() {
            return Err("the uploads folder is unavailable".into());
        }
    }
    Ok(dir)
}

/// All siblings a document owns. Reserve their names too: importing notes.md
/// beside notes.pdf would otherwise let the parser overwrite a student's file.
fn owned_names(name: &str) -> Vec<String> {
    let mut names = vec![name.to_string()];
    if let Some(pdf) = crate::paths::doc_pdf_rel(name) {
        if pdf != name { names.push(pdf.clone()); }
        let stem = Path::new(&pdf).file_stem().unwrap().to_string_lossy();
        names.extend([format!("{stem}.md"), format!("{stem}.pages.json"),
            format!("{stem}.emb.json"), format!("{stem}_images")]);
    }
    names
}

fn deleted_names(dir: &Path) -> Result<Vec<String>, String> {
    let reserved = dir.join(DELETED_NAMES_DIR);
    reject_link(&reserved)?;
    match std::fs::read_dir(reserved) {
        Ok(entries) => entries.map(|entry| entry.map(|e| e.file_name().to_string_lossy().to_string()))
            .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.to_string()),
    }
}

fn reserve_deleted_name(dir: &Path, name: &str) -> Result<(), String> {
    let reserved = dir.join(DELETED_NAMES_DIR);
    reject_link(&reserved)?;
    if !reserved.exists() { std::fs::create_dir(&reserved).map_err(|e| e.to_string())?; }
    reject_link(&reserved)?;
    let marker = reserved.join(name);
    reject_link(&marker)?;
    match std::fs::OpenOptions::new().write(true).create_new(true).open(marker) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// A name in `dir` these bytes may have.
///
/// An upload never overwrites one already there — a second `notes.pdf` becomes
/// `notes-2.pdf`, so adding the wrong file cannot destroy the right one.
/// Identical bytes under the same name are the one exception: that is the same
/// file again, and it keeps its row, its parse and its embeddings instead of
/// growing a copy.
fn free_name(dir: &Path, name: &str, bytes: &[u8]) -> Result<String, String> {
    if name.is_empty() || name == "." {
        return Err("that file has no portable name".into());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().to_string()))
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    let existing: std::collections::HashMap<_, _> = entries.iter()
        .map(|name| (name.to_lowercase(), name)).collect();
    let retired = deleted_names(dir)?;
    let retired_keys: std::collections::HashSet<_> = retired.iter().map(|name| name.to_lowercase()).collect();
    let mut occupied = std::collections::HashSet::new();
    let mut derived = std::collections::HashSet::new();
    for entry in entries.iter().chain(retired.iter()) {
        for (index, name) in owned_names(entry).into_iter().enumerate() {
            let key = name.to_lowercase();
            if index != 0 { derived.insert(key.clone()); }
            occupied.insert(key);
        }
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    for n in 1..=10_000 {
        let candidate = crate::paths::safe_filename(&if n == 1 { name.to_string() } else { format!("{stem}-{n}{ext}") });
        let target = dir.join(&candidate);
        reject_link(&target)?;
        let key = candidate.to_lowercase();
        if let Some(existing) = existing.get(&key) {
            let original = dir.join(existing);
            reject_link(&original)?;
            if !retired_keys.contains(&key) && !derived.contains(&key) && original.is_file()
                && std::fs::read(&original).map_err(|e| e.to_string())? == bytes {
                return Ok((*existing).clone());
            }
        }
        let owned = owned_names(&candidate);
        let conflict = owned.iter().any(|name| occupied.contains(&name.to_lowercase()));
        if !conflict {
            return Ok(candidate);
        }
    }
    Err("too many files already use that name".into())
}

/// Remove an uploaded file and everything derived from it.
///
/// Scoped to `uploads/` by `is_upload_rel` — see the note there. It takes the
/// converted PDF, the parse artifacts and the page images with it, because the
/// artifact skip checks include existence checks: a leftover `{stem}.md`
/// would be served as the parse of whatever lands on that name next.
#[tauri::command]
pub async fn delete_upload(app: AppHandle, relative_path: String) -> Result<(), String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || delete_upload_at(&base, &relative_path))
        .await.map_err(|e| format!("upload deletion worker stopped — {e}"))?
}

fn delete_upload_at(base: &Path, relative_path: &str) -> Result<(), String> {
    let _guard = UPLOAD_LOCK.lock().map_err(|_| "upload lock unavailable")?;
    if !crate::paths::is_upload_rel(&relative_path) {
        return Err(format!("{relative_path} is not one of your uploads"));
    }
    let parts: Vec<&str> = relative_path.split('/').collect();
    let dir = upload_dir(base, parts[1], false)?;
    // Preflight everything before deleting anything. Parse image directories
    // must never make a recursive delete follow a junction outside uploads.
    for name in owned_names(parts[3]) {
        reject_link(&dir.join(name))?;
    }
    let original = dir.join(parts[3]);
    match std::fs::symlink_metadata(&original) {
        Ok(meta) if !meta.is_file() => return Err("that upload is not a file on disk".into()),
        Ok(_) => {},
        // The frontend deletes the database row after this command returns.
        // If that write failed, retry the artifact cleanup and let it finish
        // deleting the stale row even though the original is already gone.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
        Err(error) => return Err(error.to_string()),
    }
    // A parse already running may write after deletion. Never give its old
    // name to another source, including after an app restart: late artifacts
    // must not masquerade as the replacement file's parse.
    reserve_deleted_name(&dir, parts[3])?;

    crate::paths::purge_parse_artifacts(base, relative_path);
    // The converted sibling, for an Office document. `doc_pdf_rel` returns the
    // file itself for a real PDF, which the final remove already covers.
    if let Some(pdf_rel) = crate::paths::doc_pdf_rel(&relative_path) {
        if pdf_rel != relative_path {
            let _ = std::fs::remove_file(base.join(&pdf_rel));
        }
    }
    match std::fs::remove_file(original) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
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

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let mut random = [0; 8];
            getrandom::fill(&mut random).unwrap();
            let path = std::env::temp_dir().join(format!("oculus-uploads-test-{:016x}", u64::from_ne_bytes(random)));
            std::fs::create_dir(&path).unwrap();
            Self(std::fs::canonicalize(path).unwrap())
        }
        fn source(&self, subdir: &str, name: &str, bytes: &[u8]) -> PathBuf {
            let dir = self.0.join(subdir);
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let temp = std::fs::canonicalize(std::env::temp_dir()).unwrap();
            assert!(self.0.starts_with(temp));
            assert!(self.0.file_name().unwrap().to_string_lossy().starts_with("oculus-uploads-test-"));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn an_upload_never_lands_on_a_name_already_taken() {
        let fixture = Fixture::new();
        let dir = &fixture.0;

        // Nothing there: the picked name is the name.
        assert_eq!(free_name(&dir, "notes.pdf", b"one").unwrap(), "notes.pdf");
        std::fs::write(dir.join("notes.pdf"), b"one").unwrap();

        // The same file again is the same file, not a second copy.
        assert_eq!(free_name(&dir, "notes.pdf", b"one").unwrap(), "notes.pdf");
        // A different file under a taken name steps aside rather than overwrite.
        assert_eq!(free_name(&dir, "notes.pdf", b"two").unwrap(), "notes-2.pdf");
        // Extensionless names count too, and a dotfile is all stem.
        assert_eq!(free_name(&dir, "README", b"x").unwrap(), "README");
        assert!(free_name(&dir, "", b"x").is_err());
    }

    #[test]
    fn upload_names_reserve_both_originals_and_generated_siblings() {
        let fixture = Fixture::new();
        let dir = &fixture.0;
        std::fs::write(dir.join("notes.md"), b"my own notes").unwrap();
        assert_eq!(free_name(dir, "notes.pdf", b"pdf").unwrap(), "notes-2.pdf");
        std::fs::write(dir.join("deck.docx.pdf"), b"my own pdf").unwrap();
        assert_eq!(free_name(dir, "deck.docx", b"docx").unwrap(), "deck-2.docx");
        std::fs::write(dir.join("lecture.pdf"), b"pdf").unwrap();
        assert_eq!(free_name(dir, "lecture.md", b"notes").unwrap(), "lecture-2.md");
        // Even identical bytes must not turn a parser-owned sibling into a
        // deletable upload row or overwrite it on the next parse.
        std::fs::write(dir.join("lecture.md"), b"notes").unwrap();
        assert_eq!(free_name(dir, "lecture.md", b"notes").unwrap(), "lecture-2.md");
        std::fs::write(dir.join("REPORT.PDF"), b"old").unwrap();
        assert_eq!(free_name(dir, "report.pdf", b"new").unwrap(), "report-2.pdf");
        assert_eq!(free_name(dir, "report.pdf", b"old").unwrap(), "REPORT.PDF");
    }

    #[test]
    fn imports_preserve_existing_bytes_and_duplicate_parse_artifacts() {
        let fixture = Fixture::new();
        let first = fixture.source("picked-a", "lecture.pdf", b"first");
        let second = fixture.source("picked-b", "lecture.pdf", b"second");
        let (one, _) = store_upload(&fixture.0, "COMP10001", &first).unwrap();
        let md = fixture.0.join(&one.relative_path).with_extension("md");
        std::fs::write(&md, b"existing parse").unwrap();
        let (duplicate, _) = store_upload(&fixture.0, "COMP10001", &first).unwrap();
        let (two, _) = store_upload(&fixture.0, "COMP10001", &second).unwrap();
        assert_eq!(one.relative_path, duplicate.relative_path);
        assert_eq!(two.filename, "lecture-2.pdf");
        assert_eq!(std::fs::read(fixture.0.join(one.relative_path)).unwrap(), b"first");
        assert_eq!(std::fs::read(fixture.0.join(two.relative_path)).unwrap(), b"second");
        assert_eq!(std::fs::read(md).unwrap(), b"existing parse");
    }

    #[test]
    fn concurrent_imports_cannot_overwrite_each_other() {
        let fixture = Fixture::new();
        let one = fixture.source("a", "notes.pdf", b"one");
        let two = fixture.source("b", "notes.pdf", b"two");
        let files = std::thread::scope(|scope| {
            let a = scope.spawn(|| store_upload(&fixture.0, "SUBJECT", &one).unwrap().0);
            let b = scope.spawn(|| store_upload(&fixture.0, "SUBJECT", &two).unwrap().0);
            [a.join().unwrap(), b.join().unwrap()]
        });
        assert_ne!(files[0].relative_path, files[1].relative_path);
        assert_eq!(std::fs::read(fixture.0.join(&files[0].relative_path)).unwrap(), b"one");
        assert_eq!(std::fs::read(fixture.0.join(&files[1].relative_path)).unwrap(), b"two");
    }

    #[test]
    fn deleting_an_upload_removes_only_its_own_artifacts() {
        let fixture = Fixture::new();
        let dir = upload_dir(&fixture.0, "SUBJECT", true).unwrap();
        for name in owned_names("notes.docx") {
            let path = dir.join(&name);
            if name.ends_with("_images") {
                std::fs::create_dir(&path).unwrap();
                std::fs::write(path.join("image.png"), b"image").unwrap();
            } else {
                std::fs::write(path, b"artifact").unwrap();
            }
        }
        std::fs::write(dir.join("other.pdf"), b"keep").unwrap();
        std::fs::write(fixture.0.join("oculus.db"), b"database").unwrap();
        assert!(delete_upload_at(&fixture.0, "courses/SUBJECT/uploads/..\\..\\..\\oculus.db").is_err());
        delete_upload_at(&fixture.0, "courses/SUBJECT/uploads/notes.docx").unwrap();
        assert!(owned_names("notes.docx").iter().all(|name| !dir.join(name).exists()));
        assert_eq!(std::fs::read(dir.join("other.pdf")).unwrap(), b"keep");
        assert_eq!(std::fs::read(fixture.0.join("oculus.db")).unwrap(), b"database");
    }

    #[test]
    fn deleted_names_are_not_reused_before_a_late_parse_can_finish() {
        let fixture = Fixture::new();
        let first = fixture.source("first", "notes.pdf", b"first PDF");
        let (old, _) = store_upload(&fixture.0, "SUBJECT", &first).unwrap();
        delete_upload_at(&fixture.0, &old.relative_path).unwrap();
        let second = fixture.source("second", "notes.pdf", b"different PDF");
        let (new, _) = store_upload(&fixture.0, "SUBJECT", &second).unwrap();
        assert_eq!(new.filename, "notes-2.pdf");
        // Model a previous worker finishing after the deletion and re-import.
        std::fs::write(fixture.0.join(&old.relative_path).with_extension("md"), b"old parse").unwrap();
        assert!(!fixture.0.join(&new.relative_path).with_extension("md").exists());
        assert_eq!(std::fs::read(fixture.0.join(new.relative_path)).unwrap(), b"different PDF");
    }

    #[test]
    fn upload_deletion_can_retry_after_the_database_row_write_failed() {
        let fixture = Fixture::new();
        let dir = upload_dir(&fixture.0, "SUBJECT", true).unwrap();
        let relative_path = "courses/SUBJECT/uploads/notes.docx";
        std::fs::write(dir.join("notes.docx"), b"original").unwrap();
        std::fs::write(dir.join("other.pdf"), b"keep").unwrap();
        delete_upload_at(&fixture.0, relative_path).unwrap();

        // Simulate a failed database-row deletion followed by a parser that
        // was already running when the original disappeared.
        for name in owned_names("notes.docx").into_iter().skip(1) {
            let path = dir.join(&name);
            if name.ends_with("_images") {
                std::fs::create_dir(&path).unwrap();
                std::fs::write(path.join("late.png"), b"late image").unwrap();
            } else {
                std::fs::write(path, b"late artifact").unwrap();
            }
        }
        delete_upload_at(&fixture.0, relative_path).unwrap();
        delete_upload_at(&fixture.0, relative_path).unwrap();
        assert!(owned_names("notes.docx").iter().all(|name| !dir.join(name).exists()));
        assert!(dir.join(DELETED_NAMES_DIR).join("notes.docx").is_file());
        assert_eq!(free_name(&dir, "notes.docx", b"replacement").unwrap(), "notes-2.docx");
        assert_eq!(std::fs::read(dir.join("other.pdf")).unwrap(), b"keep");
    }

    #[test]
    fn missing_uploads_do_not_bypass_path_or_file_type_checks() {
        let fixture = Fixture::new();
        let dir = upload_dir(&fixture.0, "SUBJECT", true).unwrap();
        for path in [
            "courses/SUBJECT/uploads/../missing.pdf",
            "courses/SUBJECT/uploads/..\\..\\missing.pdf",
            "courses/SUBJECT/downloads/missing.pdf",
            "courses/SUBJECT/uploads/C:missing.pdf",
            "courses/MISSING/uploads/missing.pdf",
            "/courses/SUBJECT/uploads/missing.pdf",
        ] {
            assert!(delete_upload_at(&fixture.0, path).is_err(), "{path}");
        }
        std::fs::create_dir(dir.join("directory.pdf")).unwrap();
        assert!(delete_upload_at(&fixture.0, "courses/SUBJECT/uploads/directory.pdf").is_err());
        assert!(dir.join("directory.pdf").is_dir());
        assert!(!dir.join(DELETED_NAMES_DIR).exists());
    }

    #[test]
    fn a_full_namespace_errors_instead_of_overwriting_the_first_file() {
        let fixture = Fixture::new();
        for n in 1..=10_000 {
            let name = if n == 1 { "note.txt".to_string() } else { format!("note-{n}.txt") };
            std::fs::write(fixture.0.join(name), b"keep").unwrap();
        }
        assert!(free_name(&fixture.0, "note.txt", b"new").is_err());
        assert_eq!(std::fs::read(fixture.0.join("note.txt")).unwrap(), b"keep");
    }

    /// Opt-in integration check for locally generated Office fixtures, never
    /// the user's library. Keep its unique output directory for PDF rendering
    /// checks after the Rust test finishes.
    ///
    /// Set OCULUS_OFFICE_SMOKE_DIR to a synthetic fixture directory containing
    /// lecture.docx, lecture.pptx and lecture.xlsx, then run:
    /// cargo test --release --lib real_office_import_smoke -- --ignored --nocapture
    #[test]
    #[ignore = "requires LibreOffice and synthetic OCULUS_OFFICE_SMOKE_DIR fixtures"]
    fn real_office_import_smoke() {
        let fixture_root = PathBuf::from(std::env::var_os("OCULUS_OFFICE_SMOKE_DIR")
            .expect("set OCULUS_OFFICE_SMOKE_DIR to the synthetic fixture directory"));
        assert!(fixture_root.is_absolute() && fixture_root.is_dir(),
            "OCULUS_OFFICE_SMOKE_DIR must be an existing absolute directory");
        for ext in ["docx", "pptx", "xlsx"] {
            assert!(fixture_root.join(format!("lecture.{ext}")).is_file(),
                "missing synthetic lecture.{ext} fixture");
        }

        let mut random = [0; 8];
        getrandom::fill(&mut random).unwrap();
        let library = fixture_root.join(format!("office-smoke-library-{:016x}", u64::from_ne_bytes(random)));
        std::fs::create_dir(&library).unwrap();
        let mut results = Vec::new();
        for ext in ["docx", "pptx", "xlsx"] {
            let source = fixture_root.join(format!("lecture.{ext}"));
            let (imported, warning) = store_upload(&library, "OFFICE_SMOKE_学生", &source)
                .unwrap_or_else(|error| panic!("{ext} import failed: {error}"));
            assert!(warning.is_none(), "{ext} conversion warning: {warning:?}");
            assert_eq!(imported.filename, format!("lecture.{ext}"));
            assert_eq!(imported.file_type, ext);
            let original = library.join(&imported.relative_path);
            let original_bytes = std::fs::read(&original).unwrap();
            assert_eq!(original_bytes, std::fs::read(&source).unwrap());
            assert_eq!(imported.size_bytes, original_bytes.len() as u64);

            let pdf = library.join(crate::paths::doc_pdf_rel(&imported.relative_path).unwrap());
            let pdf_bytes = std::fs::read(&pdf).unwrap();
            assert!(pdf_bytes.starts_with(b"%PDF-") && pdf_bytes.len() > 128,
                "{ext} did not produce a PDF");
            let original_modified = std::fs::metadata(&original).unwrap().modified().unwrap();
            let pdf_modified = std::fs::metadata(&pdf).unwrap().modified().unwrap();
            let (duplicate, warning) = store_upload(&library, "OFFICE_SMOKE_学生", &source).unwrap();
            assert!(warning.is_none(), "{ext} re-import warning: {warning:?}");
            assert_eq!(duplicate.relative_path, imported.relative_path);
            assert_eq!(duplicate.filename, imported.filename);
            assert_eq!(std::fs::read(&original).unwrap(), original_bytes);
            assert_eq!(std::fs::read(&pdf).unwrap(), pdf_bytes);
            assert_eq!(std::fs::metadata(&original).unwrap().modified().unwrap(), original_modified);
            assert_eq!(std::fs::metadata(&pdf).unwrap().modified().unwrap(), pdf_modified);
            results.push(serde_json::json!({
                "format": ext,
                "source": source,
                "original": original,
                "pdf": pdf,
                "relative_path": imported.relative_path,
                "source_bytes": imported.size_bytes,
                "pdf_bytes": pdf_bytes.len(),
                "unchanged_reimport": true,
            }));
        }
        let uploads = upload_dir(&library, "OFFICE_SMOKE_学生", false).unwrap();
        assert_eq!(std::fs::read_dir(uploads).unwrap().count(), 6,
            "expected exactly three originals and three derived PDFs");
        let summary = serde_json::json!({"library": library, "files": results});
        let manifest = library.join("office-import-smoke.json");
        std::fs::write(&manifest, serde_json::to_vec_pretty(&summary).unwrap()).unwrap();
        println!("OCULUS_OFFICE_SMOKE_MANIFEST={}", manifest.display());
        println!("OCULUS_OFFICE_SMOKE_RESULT={summary}");
    }

    #[cfg(windows)]
    fn junction(link: &Path, target: &Path) {
        use base64::Engine;
        let quote = |path: &Path| path.to_string_lossy().replace('\'', "''");
        let script = format!("$ErrorActionPreference='Stop'; $null = New-Item -ItemType Junction -Path '{}' -Target '{}'", quote(link), quote(target));
        let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let output = crate::platform::command("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-EncodedCommand"])
            .arg(base64::engine::general_purpose::STANDARD.encode(bytes)).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }

    #[cfg(windows)]
    #[test]
    fn windows_junctions_cannot_redirect_import_or_delete() {
        let fixture = Fixture::new();
        let outside = fixture.0.join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("notes.pdf"), b"untouched").unwrap();
        let course = fixture.0.join("courses/SUBJECT");
        std::fs::create_dir_all(&course).unwrap();
        let link = course.join("uploads");
        junction(&link, &outside);
        assert!(store_upload(&fixture.0, "SUBJECT", &outside.join("notes.pdf")).is_err());
        assert!(delete_upload_at(&fixture.0, "courses/SUBJECT/uploads/notes.pdf").is_err());
        assert!(delete_upload_at(&fixture.0, "courses/SUBJECT/uploads/missing.pdf").is_err());
        // Remove only the junction itself, never recurse into its target.
        std::fs::remove_dir(&link).unwrap();
        let uploads = upload_dir(&fixture.0, "SUBJECT", true).unwrap();
        std::fs::write(uploads.join("notes.pdf"), b"source").unwrap();
        let images = uploads.join("notes_images");
        junction(&images, &outside);
        assert!(delete_upload_at(&fixture.0, "courses/SUBJECT/uploads/notes.pdf").is_err());
        assert!(uploads.join("notes.pdf").exists());
        std::fs::remove_file(uploads.join("notes.pdf")).unwrap();
        assert!(delete_upload_at(&fixture.0, "courses/SUBJECT/uploads/notes.pdf").is_err());
        assert_eq!(std::fs::read(outside.join("notes.pdf")).unwrap(), b"untouched");
        std::fs::remove_dir(images).unwrap();
    }
}
