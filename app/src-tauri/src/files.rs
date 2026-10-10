use std::path::Path;

#[tauri::command]
pub async fn read_course_file(relative_path: String) -> Result<String, String> {
    crate::blocking::run(move || {
        let path = crate::paths::data_dir().join(&relative_path);
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    })
    .await
}

fn file_has_content(root: &Path, relative_path: &str) -> Result<bool, String> {
    if relative_path.is_empty()
        || Path::new(relative_path)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("expected a path relative to the library".into());
    }
    let path = match root.join(relative_path).canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    if !path.starts_with(&root) {
        return Err("path is outside the library".into());
    }
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    Ok(metadata.is_file() && metadata.len() > 0)
}

/// Check a parsed sibling's presence without transferring its markdown.
#[tauri::command]
pub async fn course_file_has_content(relative_path: String) -> Result<bool, String> {
    crate::blocking::run(move || file_has_content(&crate::paths::data_dir(), &relative_path)).await
}

#[tauri::command]
pub fn open_course_file(relative_path: String) -> Result<(), String> {
    let path = crate::paths::data_dir().join(&relative_path);
    tauri_plugin_opener::open_path(path.to_str().unwrap_or(""), None::<&str>)
        .map_err(|e| e.to_string())
}

// ── The student's own files ───────────────────────────────────────────────────
//
// Copied into `courses/<code>/uploads/`; the rest of the pipeline keys off the
// path alone.

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
    crate::blocking::run(move || {
        // Imports share name allocation; keep simultaneous batches from choosing
        // the same unused upload name while conversion runs off the command thread.
        static IMPORT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = IMPORT_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let data_dir = crate::paths::data_dir();
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
        .join(crate::paths::safe_dir(code))
        .join(crate::paths::UPLOADS_DIR);
    let name = free_name(&dir, &crate::paths::safe_filename(picked), &bytes);

    let course_rel = format!("{}/{name}", crate::paths::UPLOADS_DIR);
    let (rel, size, action) =
        crate::paths::write_course_bytes(data_dir, code, &course_rel, &bytes)?;

    // A reused name may hold a stale parse (a pass in flight across a delete).
    // Only `Unchanged` — identical bytes — provably keeps its own.
    if action != crate::paths::WriteAction::Unchanged {
        crate::paths::purge_parse_artifacts(data_dir, &rel);
    }

    // The derived sibling PDF for Office documents (`doc_pdf_rel` in paths.rs).
    // A spreadsheet's text is written by the parse kick the frontend sends once
    // the row exists (`parse_file` → `crate::sheets`).
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
fn free_name(dir: &Path, name: &str, bytes: &[u8]) -> String {
    step_aside(dir, name, |existing| existing == bytes)
}

/// `name`, else `stem-2.ext`, `stem-3.ext`… — the first free, or that `ours`
/// says already holds this file.
fn step_aside(dir: &Path, name: &str, ours: impl Fn(&[u8]) -> bool) -> String {
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
    if !crate::paths::is_upload_rel(&relative_path) {
        return Err(format!("{relative_path} is not one of your uploads"));
    }
    let base = crate::paths::data_dir();

    crate::paths::purge_parse_artifacts(&base, &relative_path);
    // The converted sibling of an Office document (a real PDF is itself).
    if let Some(pdf_rel) = crate::paths::doc_pdf_rel(&relative_path) {
        if pdf_rel != relative_path {
            let _ = std::fs::remove_file(base.join(&pdf_rel));
        }
    }
    std::fs::remove_file(base.join(&relative_path)).map_err(|e| e.to_string())
}

// ── The student's own documents ───────────────────────────────────────────────
//
// Markdown notes in `courses/<code>/documents/`: Rust moves the bytes, the
// frontend writes the `files` row. A note is rewritten on save and moves on
// rename, so every mutating command checks the path's shape (`is_document_rel`)
// before resolving it.

/// A title's filename: sanitised, `Untitled` when only underscores survive,
/// `.md` always. A trailing dot is dropped, or `notes..md` would be sanitised
/// again on write and land under a different name.
fn document_name(title: &str) -> String {
    let stem = crate::paths::safe_filename(title.trim());
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
        .join(crate::paths::safe_dir(code))
        .join(crate::paths::DOCUMENTS_DIR)
}

/// A document's absolute path, or an error — the one place the guard runs.
fn document_path(relative_path: &str) -> Result<std::path::PathBuf, String> {
    if !crate::paths::is_document_rel(relative_path) {
        return Err(format!("{relative_path} is not one of your documents"));
    }
    Ok(crate::paths::data_dir().join(relative_path))
}

/// A note's pictures: `assets/` beside it, so they resolve relative to the note
/// for every reader (`useLibraryMdComponents` in `FileViewer.tsx`). Takes the
/// path `document_path` guarded.
fn document_assets_dir(note: &Path) -> Result<std::path::PathBuf, String> {
    let dir = note
        .parent()
        .ok_or_else(|| format!("{} has no folder", note.display()))?;
    Ok(dir.join(crate::paths::DOCUMENT_ASSETS_DIR))
}

/// The link the editor writes, relative to the note (survives a rename).
fn document_asset_ref(name: &str) -> String {
    format!("{}/{name}", crate::paths::DOCUMENT_ASSETS_DIR)
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
    let data_dir = crate::paths::data_dir();
    let dir = documents_dir(&data_dir, &subject_code);
    // Never "ours": an existing empty note is still another note.
    let name = step_aside(&dir, &document_name(&title), |_| false);
    let (rel, size, _) = crate::paths::write_course_bytes(
        &data_dir,
        &subject_code,
        &format!("{}/{name}", crate::paths::DOCUMENTS_DIR),
        b"",
    )?;
    Ok(document_file(rel, size))
}

/// The hidden sibling a note's text is staged in. It ends in `.tmp`, never
/// `.md`, so `is_document_rel` rejects a leftover and it never becomes a row.
fn note_temp_path(note: &Path) -> Result<std::path::PathBuf, String> {
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
        crate::clock::now_nanos()
    )))
}

/// Replace a note's text whole: a reader, or a crash, sees the old note or
/// the new, never a truncated one. fsynced before the rename (in
/// `atomic_write`), since a rename can otherwise land before the data does.
fn write_note(note: &Path, content: &[u8]) -> Result<(), String> {
    let tmp = note_temp_path(note)?;
    if let Some(dir) = tmp.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    crate::atomic_write::write(note, &tmp, content)
}

/// Save a note's text; returns the byte count for `size_bytes`.
#[tauri::command]
pub async fn write_document(relative_path: String, content: String) -> Result<u64, String> {
    let path = document_path(&relative_path)?;
    crate::blocking::run(move || {
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
    let data_dir = crate::paths::data_dir();
    let dir = documents_dir(&data_dir, &subject_code);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let dir_rel = format!(
        "courses/{}/{}",
        crate::paths::safe_dir(&subject_code),
        crate::paths::DOCUMENTS_DIR
    );
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
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

/// A picture pasted into a note (base64), written now into `assets/` and
/// returned as the link. A tag later deleted leaves an orphan file — accepted.
/// Cap, sniff and naming are `crate::harness::attach`'s.
#[tauri::command]
pub async fn attach_document_image(relative_path: String, data: String) -> Result<String, String> {
    let dir = document_assets_dir(&document_path(&relative_path)?)?;
    let bytes = crate::harness::attach::decode(&data)?;
    crate::blocking::run(move || {
        crate::harness::attach::write_image(&dir, &bytes).map(|name| document_asset_ref(&name))
    })
    .await
}

/// The same, for a picture dropped from Finder (a path, read here).
#[tauri::command]
pub async fn attach_document_file(relative_path: String, path: String) -> Result<String, String> {
    let dir = document_assets_dir(&document_path(&relative_path)?)?;
    crate::blocking::run(move || {
        let bytes = crate::harness::attach::read_dropped(&path)?;
        crate::harness::attach::write_image(&dir, &bytes).map(|name| document_asset_ref(&name))
    })
    .await
}

/// Derive parse status from disk for a set of PDF-backed relative paths
/// (PDFs, plus Office files parsed via their derived sibling PDF).
/// Returns (relative_path, status); paths with no parse output are omitted.
#[tauri::command]
pub async fn scan_parsed_files(
    relative_paths: Vec<String>,
) -> Result<Vec<(String, String)>, String> {
    crate::blocking::run(move || {
        let base = crate::paths::data_dir();
        Ok(relative_paths
            .into_iter()
            .filter_map(|rel| {
                let pdf_rel = crate::paths::doc_pdf_rel(&rel)?;
                crate::parse::parse_mode(&base.join(&pdf_rel)).map(|mode| (rel, mode.to_string()))
            })
            .collect())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_metadata_distinguishes_files_and_stays_inside_the_library() {
        let scratch = crate::test_support::Scratch::new("file-content");
        std::fs::write(scratch.join("empty.md"), b"").unwrap();
        std::fs::write(scratch.join("full.md"), b"markdown").unwrap();
        std::fs::create_dir(scratch.join("directory")).unwrap();
        assert!(!file_has_content(&scratch, "empty.md").unwrap());
        assert!(file_has_content(&scratch, "full.md").unwrap());
        assert!(!file_has_content(&scratch, "missing.md").unwrap());
        assert!(!file_has_content(&scratch, "directory").unwrap());
        assert!(file_has_content(&scratch, "../outside.md").is_err());
        assert!(file_has_content(&scratch, "/etc/passwd").is_err());
        #[cfg(unix)]
        {
            let outside = crate::test_support::Scratch::new("file-content-outside");
            std::fs::write(outside.join("private.md"), b"outside").unwrap();
            std::os::unix::fs::symlink(outside.join("private.md"), scratch.join("escape.md"))
                .unwrap();
            assert!(file_has_content(&scratch, "escape.md").is_err());
        }
    }

    /// `assets/` sits beside the note and is not itself reachable as a note.
    #[test]
    fn a_note_s_pictures_sit_beside_it() {
        let note = Path::new("/data/courses/MULT20015/documents/Week 3.md");
        assert_eq!(
            document_assets_dir(note).unwrap(),
            Path::new("/data/courses/MULT20015/documents/assets")
        );
        let other = Path::new("/data/courses/MULT20015/documents/Ideas.md");
        assert_eq!(
            document_assets_dir(note).unwrap(),
            document_assets_dir(other).unwrap()
        );

        assert_eq!(
            document_asset_ref("20260922-101112-0a1b2c3d.png"),
            "assets/20260922-101112-0a1b2c3d.png"
        );
        assert!(!crate::paths::is_document_rel(
            "courses/MULT20015/documents/assets/20260922-101112-0a1b2c3d.png"
        ));
    }

    /// A save replaces the note whole and leaves no staging file; a leftover
    /// staging name is never taken for a note.
    #[test]
    fn a_note_is_replaced_whole() {
        let dir = crate::test_support::Scratch::new("note-write");
        let note = dir.join("Week 3.md");
        std::fs::write(&note, b"a much longer old text").unwrap();
        write_note(&note, b"new").unwrap();
        assert_eq!(std::fs::read(&note).unwrap(), b"new");
        assert_eq!(std::fs::read_dir(&*dir).unwrap().count(), 1);

        let tmp = note_temp_path(&note).unwrap();
        let name = tmp.file_name().unwrap().to_str().unwrap();
        assert!(
            name.starts_with(".Week 3.md.") && name.ends_with(".tmp"),
            "{name}"
        );
        assert!(!crate::paths::is_document_rel(&format!(
            "courses/X/documents/{name}"
        )));
    }

    #[test]
    fn an_upload_never_lands_on_a_name_already_taken() {
        let dir = crate::test_support::Scratch::new("uploads");

        assert_eq!(free_name(&dir, "notes.pdf", b"one"), "notes.pdf");
        std::fs::write(dir.join("notes.pdf"), b"one").unwrap();

        assert_eq!(free_name(&dir, "notes.pdf", b"one"), "notes.pdf");
        assert_eq!(free_name(&dir, "notes.pdf", b"two"), "notes-2.pdf");
        assert_eq!(free_name(&dir, "README", b"x"), "README");
    }

    #[test]
    fn a_title_becomes_a_markdown_filename() {
        assert_eq!(document_name("Week 3 notes"), "Week_3_notes.md");
        assert_eq!(document_name("  padded  "), "padded.md");
        assert_eq!(document_name(""), "Untitled.md");
        assert_eq!(document_name("   "), "Untitled.md");
        assert_eq!(document_name("..."), "Untitled.md");
        assert_eq!(document_name("???"), "Untitled.md");
        assert_eq!(document_name("Draft."), "Draft.md");
        assert_eq!(document_name("../../etc/passwd"), "____etc_passwd.md");
        assert_eq!(document_name("a/b"), "a_b.md");
    }

    #[test]
    fn a_new_document_never_lands_on_a_name_already_taken() {
        let dir = crate::test_support::Scratch::new("documents");

        let name = document_name("notes");
        assert_eq!(step_aside(&dir, &name, |_| false), "notes.md");
        std::fs::write(dir.join("notes.md"), b"").unwrap();

        assert_eq!(step_aside(&dir, &name, |_| false), "notes-2.md");
        std::fs::write(dir.join("notes-2.md"), b"some text").unwrap();
        assert_eq!(step_aside(&dir, &name, |_| false), "notes-3.md");
    }
}
