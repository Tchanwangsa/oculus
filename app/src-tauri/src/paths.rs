//! App data locations, resolved without a Tauri `AppHandle`.
//!
//! The scrape engine and the `oculus` CLI both need the same directory the app
//! writes to. Tauri computes it from the bundle identifier, so we do too —
//! keeping one definition means the CLI and the app can never disagree about
//! where the cookie, the database, and `courses/` live.

use std::path::PathBuf;

/// Must match `identifier` in tauri.conf.json.
pub const IDENTIFIER: &str = "com.tchan.oculus";

pub const CANVAS_BASE: &str = "https://canvas.lms.unimelb.edu.au";

/// Same directory Tauri's `app.path().app_data_dir()` returns.
pub fn data_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));

    #[cfg(target_os = "macos")]
    let base = home.join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("AppData/Roaming"));
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));

    base.join(IDENTIFIER)
}

pub fn cookie_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("canvas-session.cookie")
}

pub fn auth_flag_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("canvas-session").join("authenticated")
}

/// Record that we hold a session Canvas has accepted.
///
/// The app's startup probe reads this before it reads anything else — no flag
/// means "fresh session", and it will not even look at the cookie beside it. So
/// every path that establishes a session must write it, the CLI included;
/// otherwise `oculus auth auto` leaves a perfectly good cookie on disk and the
/// app still opens disconnected.
pub fn mark_authenticated(data_dir: &std::path::Path) {
    let flag = auth_flag_path(data_dir);
    if let Some(parent) = flag.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&flag, b"1").ok();
}

/// Where the LaunchAgent keep-alive reports what it did. Read back into
/// Settings → Canvas, so the user can see the agent is alive without launchctl.
pub fn keepalive_log_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("session-keepalive.log")
}

/// Append one timestamped line, keeping the file bounded — it is written every
/// few hours forever and nobody prunes it.
pub fn append_keepalive_log(data_dir: &std::path::Path, message: &str) {
    use std::io::Write;

    let path = keepalive_log_path(data_dir);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let line = format!("{}Z {message}\n", iso8601_utc(stamp));

    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        f.write_all(line.as_bytes()).ok();
    }

    // Cheap trim: only rewrite once the file has actually grown past the cap.
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > 64 * 1024 {
            if let Ok(text) = std::fs::read_to_string(&path) {
                let lines: Vec<&str> = text.lines().collect();
                let keep = lines[lines.len().saturating_sub(200)..].join("\n");
                std::fs::write(&path, format!("{keep}\n")).ok();
            }
        }
    }
}

/// `YYYY-MM-DDTHH:MM:SS` from a Unix timestamp — civil-time arithmetic only, to
/// keep a date crate out of a build that needs nothing else from one.
pub fn iso8601_utc(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // Days since 1970-01-01 → civil date (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}")
}

pub fn db_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("oculus.db")
}

/// The database plus the two files SQLite keeps beside it in WAL mode.
///
/// This is the set a *writer* has to be able to open, and it is the one
/// exception the harness's sandboxes make to "nothing outside `agents/` is
/// writable": `oculus project` and `oculus task` are how an agent writes the
/// student's board, and a process that may open `oculus.db` but not
/// `oculus.db-wal` fails with SQLite's "attempt to write a readonly
/// database" — measured under a seatbelt profile with only `agents/`
/// writable, which is exactly what both bridges were handing the CLI.
///
/// Files, not the directory they sit in. Granting the directory would put the
/// session cookie and the Ed token beside them inside the agent's reach, and
/// the sidecars never need creating from in there: nothing runs an agent
/// except the app and the CLI, and both hold the database open — which is
/// what makes the two sidecars exist — for as long as the agent lives.
pub fn db_write_paths(data_dir: &std::path::Path) -> Vec<PathBuf> {
    let db = db_path(data_dir);
    let sidecar = |suffix: &str| {
        let mut p = db.clone().into_os_string();
        p.push(suffix);
        PathBuf::from(p)
    };
    vec![db.clone(), sidecar("-wal"), sidecar("-shm")]
}

// ── Course artifact paths ────────────────────────────────────────────────────
//
// Canvas titles become filenames, so every component is sanitised: they arrive
// with slashes, colons and the occasional "..".

pub fn safe_dir(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

pub fn safe_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect::<String>()
        .replace("..", "_")
}

pub fn safe_rel_path(rel: &str) -> Option<String> {
    let parts: Vec<String> = rel
        .split('/')
        .filter(|s| !s.is_empty())
        .map(safe_filename)
        .filter(|s| s != "." && s != "_" && !s.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The data-dir-relative path an artifact will occupy, computable before any
/// bytes move — lets the scraper announce a download while it is in flight
/// under the same key the write event will use.
pub fn course_rel_path(code: &str, rel_path: &str) -> Option<String> {
    safe_rel_path(rel_path).map(|safe| format!("courses/{}/{}", safe_dir(code), safe))
}

/// What a write did to the file already on disk. The scraper re-fetches
/// everything each run, so this comparison is the only place "nothing actually
/// changed" is knowable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteAction {
    New,
    Updated,
    Unchanged,
}

impl WriteAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            WriteAction::New => "new",
            WriteAction::Updated => "updated",
            WriteAction::Unchanged => "unchanged",
        }
    }
}

/// Write one course artifact. Returns its path relative to `data_dir` (what the
/// database stores), the byte count, and whether the content was new, changed,
/// or identical to what was there. Identical content is not rewritten.
pub fn write_course_bytes(
    data_dir: &std::path::Path,
    code: &str,
    rel_path: &str,
    content: &[u8],
) -> Result<(String, u64, WriteAction), String> {
    let rel = course_rel_path(code, rel_path).ok_or_else(|| format!("invalid path: {rel_path}"))?;
    let path = data_dir.join(&rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let action = match std::fs::read(&path) {
        Ok(existing) if existing == content => WriteAction::Unchanged,
        Ok(_) => WriteAction::Updated,
        Err(_) => WriteAction::New,
    };
    if action != WriteAction::Unchanged {
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
    }
    Ok((rel, content.len() as u64, action))
}

/// Delete the parse/embed artifacts a PDF-backed file leaves beside its PDF —
/// `{stem}.md`, `{stem}.pages.json`, `{stem}.emb.json` and the `{stem}_images/`
/// directory the markdown's figures live in. Both skip checks — `parse_mode`
/// and `embed::is_embedded` — read those records rather than the PDF's bytes,
/// so without this a re-scrape that finds changed bytes would keep serving the
/// old parse and embeddings forever.
///
/// The images go with them because they are only ever referenced *from* that
/// markdown: leaving them is not a fallback, it is a directory of figures for
/// a document that no longer says anything about them, and either engine
/// rebuilds it from scratch anyway.
///
/// `library_rel` is the library file, data-dir-relative (`courses/…`).
pub fn purge_parse_artifacts(data_dir: &std::path::Path, library_rel: &str) {
    let Some(pdf_rel) = doc_pdf_rel(library_rel) else { return };
    let pdf = data_dir.join(&pdf_rel);
    let (Some(stem), Some(parent)) = (pdf.file_stem().and_then(|s| s.to_str()), pdf.parent())
    else {
        return;
    };
    for name in [
        format!("{stem}.md"),
        format!("{stem}.pages.json"),
        format!("{stem}.emb.json"),
    ] {
        let _ = std::fs::remove_file(parent.join(name));
    }
    let _ = std::fs::remove_dir_all(parent.join(format!("{stem}_images")));
}

/// Extensions LibreOffice converts to PDF at download time. The original is
/// the library file; the conversion lives beside it as `{name}.pdf`.
pub const OFFICE_EXTS: &[&str] = &[".pptx", ".docx", ".xlsx", ".ppt", ".doc", ".xls"];

/// The PDF that parsing, embedding and in-app viewing operate on for a library
/// file: the file itself for real PDFs, the converted sibling
/// (`deck.pptx` → `deck.pptx.pdf`) for Office documents, `None` for anything
/// else (markdown, images).
pub fn doc_pdf_rel(rel: &str) -> Option<String> {
    let lower = rel.to_ascii_lowercase();
    if lower.ends_with(".pdf") {
        return Some(rel.to_string());
    }
    OFFICE_EXTS
        .iter()
        .any(|e| lower.ends_with(e))
        .then(|| format!("{rel}.pdf"))
}


/// The one directory inside a course folder the scraper never writes to: the
/// student's own files, added by hand. Everything downstream — conversion,
/// parsing, embedding, search, the agent's view of `courses/` — treats them as
/// ordinary library files, so this constant is the whole of what makes them
/// separate.
pub const UPLOADS_DIR: &str = "uploads";

/// True for a data-dir-relative path inside some subject's uploads folder.
///
/// This is the delete command's entire guard. Nothing else under `courses/` is
/// the user's to throw away — a scraped file deleted from disk comes back on
/// the next sync, minus its parse — so deletion is scoped here by construction
/// rather than by asking the caller to be careful.
pub fn is_upload_rel(rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    !rel.contains("..") && parts.len() > 3 && parts[0] == "courses" && parts[2] == UPLOADS_DIR
}

pub fn category_from_path(path: &str) -> &'static str {
    match path {
        "home.md" => "home",
        "syllabus.md" => "syllabus",
        p if p.starts_with("uploads/") => "upload",
        p if p.starts_with("pages/") => "page",
        p if p.starts_with("assignments/") => "assignment",
        p if p.starts_with("quizzes/") => "quiz",
        p if p.starts_with("announcements/") => "announcement",
        p if p.starts_with("ed/") => "ed",
        p if p.starts_with("files/") => "file",
        p if p.starts_with("modules/") => "module",
        p if p.starts_with("images/") => "image",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_match_the_shell_agent_they_replaced() {
        // `date -u +%Y-%m-%dT%H:%M:%SZ` at these instants.
        assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00");
        assert_eq!(iso8601_utc(1_756_886_400), "2025-09-03T08:00:00");
        // A leap day, where naive day-count arithmetic goes wrong.
        assert_eq!(iso8601_utc(1_709_164_800), "2024-02-29T00:00:00");
    }

    #[test]
    fn path_components_are_sanitised() {
        assert_eq!(safe_filename("Lecture 1: Intro.pdf"), "Lecture_1__Intro.pdf");
        // No traversal survives: dots collapse, separators become underscores.
        assert_eq!(safe_filename("../../etc/passwd"), "____etc_passwd");
        assert_eq!(safe_rel_path("files/a.pdf").unwrap(), "files/a.pdf");
        assert_eq!(safe_rel_path("../../x").unwrap(), "x");
        assert!(safe_rel_path("///").is_none());
    }

    #[test]
    fn doc_pdf_resolution() {
        assert_eq!(doc_pdf_rel("files/a.pdf").as_deref(), Some("files/a.pdf"));
        assert_eq!(doc_pdf_rel("files/deck.pptx").as_deref(), Some("files/deck.pptx.pdf"));
        assert_eq!(doc_pdf_rel("files/notes.DOCX").as_deref(), Some("files/notes.DOCX.pdf"));
        assert_eq!(doc_pdf_rel("files/marks.xlsx").as_deref(), Some("files/marks.xlsx.pdf"));
        assert_eq!(doc_pdf_rel("files/legacy.xls").as_deref(), Some("files/legacy.xls.pdf"));
        assert_eq!(doc_pdf_rel("pages/intro.md"), None);
        assert_eq!(doc_pdf_rel("images/x.png"), None);
    }

    #[test]
    fn categories_follow_the_directory() {
        assert_eq!(category_from_path("home.md"), "home");
        assert_eq!(category_from_path("pages/x.md"), "page");
        assert_eq!(category_from_path("files/x.pdf"), "file");
        assert_eq!(category_from_path("assignments/a1.md"), "assignment");
        assert_eq!(category_from_path("quizzes/week-3.md"), "quiz");
        assert_eq!(category_from_path("ed/0031-welcome.md"), "ed");
        assert_eq!(category_from_path("uploads/tutor-notes.pdf"), "upload");
        assert_eq!(category_from_path("nope.txt"), "other");
    }

    #[test]
    fn only_a_subjects_uploads_folder_is_deletable() {
        assert!(is_upload_rel("courses/COMP30026/uploads/notes.pdf"));
        // Everything else under courses/ belongs to a sync.
        assert!(!is_upload_rel("courses/COMP30026/files/lecture.pdf"));
        assert!(!is_upload_rel("courses/COMP30026/uploads"));
        assert!(!is_upload_rel("lectures/abc/source1.mp4"));
        assert!(!is_upload_rel("courses/../oculus.db"));
        assert!(!is_upload_rel("courses/X/uploads/../../../oculus.db"));
    }
}
