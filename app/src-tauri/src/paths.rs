//! App data locations, resolved without a Tauri `AppHandle`.
//!
//! One definition of the app's data directory, computed from the bundle
//! identifier as Tauri does, so the CLI and the app agree on where the cookie,
//! the database and `courses/` live.

use std::path::PathBuf;

/// `identifier` in tauri.conf.json; a test holds them together.
pub use keyd_core::paths::IDENTIFIER;

/// Defined in `keyd_core`, because keyd's sign-in lands on it too.
pub use keyd_core::paths::CANVAS_BASE;

/// Tauri's `app.path().app_data_dir()`, reachable without an `AppHandle`; the
/// one way every module and the CLI find the data directory. Defined in
/// `keyd_core`, so `oculus-keyd` agrees.
pub fn data_dir() -> PathBuf {
    keyd_core::paths::data_dir()
}

// The session files the headless sign-in writes are named in `keyd_core::paths`,
// which keyd shares; these keep the app's call sites as they were.

pub fn cookie_path(data_dir: &std::path::Path) -> PathBuf {
    keyd_core::paths::cookie(data_dir)
}

/// Okta's cookies for `sso.unimelb.edu.au`, the same bare `name=value; …`
/// header as the Canvas one. Only the in-app browser replays it.
pub fn sso_cookie_path(data_dir: &std::path::Path) -> PathBuf {
    keyd_core::paths::sso_cookie(data_dir)
}

/// Writes a session file readable by this user only, narrowing an existing
/// one before the body lands.
pub fn write_private(path: &std::path::Path, body: &str) -> std::io::Result<()> {
    keyd_core::paths::write_private(path, body)
}

/// Ed's `x-token`, minted from the Canvas session (`ed.rs`).
pub fn ed_token_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("ed-session.token")
}

pub fn auth_flag_path(data_dir: &std::path::Path) -> PathBuf {
    keyd_core::paths::session_dir(data_dir).join("authenticated")
}

/// Present from a sign-out until the next session (`mark_authenticated`).
/// While it is, automatic sign-ins stand down (`okta::sign_in`).
pub fn signed_out_path(data_dir: &std::path::Path) -> PathBuf {
    keyd_core::paths::signed_out(data_dir)
}

/// Drops the saved Canvas, Okta and Ed sessions and the auth flag, and marks
/// the app signed out; returns whether there was anything to drop. The attempt
/// record beside the flag stays: forgetting a lockout pause would let
/// automatic sign-in resume.
pub fn sign_out(data_dir: &std::path::Path) -> std::io::Result<bool> {
    let mut had = false;
    for path in [
        cookie_path(data_dir),
        sso_cookie_path(data_dir),
        ed_token_path(data_dir),
        auth_flag_path(data_dir),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => had = true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    let marker = signed_out_path(data_dir);
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&marker, b"1")?;
    Ok(had)
}

/// Record that we hold a session Canvas has accepted.
///
/// The app's startup probe ignores the cookie without this flag, so every path
/// that establishes a session must write it, the CLI included.
pub fn mark_authenticated(data_dir: &std::path::Path) {
    let flag = auth_flag_path(data_dir);
    if let Some(parent) = flag.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&flag, b"1").ok();
    std::fs::remove_file(signed_out_path(data_dir)).ok();
}

/// Where the LaunchAgent keep-alive logs; shown in Settings → Canvas.
pub fn keepalive_log_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("session-keepalive.log")
}

/// Append one timestamped line to the keep-alive log.
pub fn append_keepalive_log(data_dir: &std::path::Path, message: &str) {
    append_bounded_log(&keepalive_log_path(data_dir), message);
}

/// The attempt record every process checks before an automatic sign-in.
pub fn sign_in_record_path(data_dir: &std::path::Path) -> PathBuf {
    keyd_core::paths::sign_in_record(data_dir)
}

/// Append one timestamped line, keeping the file bounded.
fn append_bounded_log(path: &std::path::Path, message: &str) {
    keyd_core::paths::append_bounded_log(path, message, crate::clock::now_secs());
}

/// `YYYY-MM-DDTHH:MM:SS` from a Unix timestamp, without a date crate.
pub use keyd_core::clock::iso8601_utc;

/// `oculus-keyd`'s endpoint (`keyd.rs`).
pub fn keyd_socket_path(data_dir: &std::path::Path) -> PathBuf {
    keyd_core::paths::socket(data_dir)
}

pub fn db_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("oculus.db")
}

/// The database plus the two files SQLite keeps beside it in WAL mode.
///
/// The harness sandboxes' one exception to "nothing outside `agents/` is
/// writable", so `oculus project`/`task` can write: SQLite needs the `-wal` and
/// `-shm` files too, or it reports a readonly database. Files, not the
/// directory, which also holds the session cookie and the Ed token.
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
// Canvas titles become filenames, so every component is sanitised.

pub fn safe_dir(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn safe_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
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

/// The data-dir-relative path an artifact will occupy, known before any bytes
/// move, so an in-flight download uses the write event's key.
pub fn course_rel_path(code: &str, rel_path: &str) -> Option<String> {
    safe_rel_path(rel_path).map(|safe| format!("courses/{}/{}", safe_dir(code), safe))
}

/// What a write did to the file on disk: the only place "nothing changed" is
/// knowable, since the scraper re-fetches everything.
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

/// Write one course artifact; returns the data-dir-relative path, the byte count
/// and whether the content was new, changed or identical (and not rewritten).
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

/// Delete a file's derived text and parse/embed artifacts (`{stem}.md`,
/// `.pages.json`, `.emb.json`, `{stem}_images/`). The skip checks read those
/// records, not the source, so changed bytes would otherwise keep the old text.
/// A spreadsheet's stem is its own name, and a `{name}.pdf` beside it is a
/// PDF-route leftover that goes too. `library_rel` is data-dir-relative.
pub fn purge_parse_artifacts(data_dir: &std::path::Path, library_rel: &str) {
    let sheet = is_sheet(library_rel);
    let Some(pdf_rel) =
        doc_pdf_rel(library_rel).or_else(|| sheet.then(|| format!("{library_rel}.pdf")))
    else {
        return;
    };
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
    if sheet {
        let _ = std::fs::remove_file(&pdf);
    }
}

/// Extensions LibreOffice converts to PDF at download, as `{name}.pdf` beside it.
/// Mirrored by `OFFICE_EXTS` in `app/src/lib/fileTypes.ts`.
pub const OFFICE_EXTS: &[&str] = &[".pptx", ".docx", ".ppt", ".doc"];

/// Spreadsheets and CSVs, converted to `{name}.md` text in-process
/// (`crate::sheets`): never PDF-backed, never embedded. Mirrored by
/// `SHEET_EXTS` in `app/src/lib/fileTypes.ts`.
pub const SHEET_EXTS: &[&str] = &[".xlsx", ".xlsm", ".xls", ".ods", ".csv"];

/// A spreadsheet by name, in any case.
pub fn is_sheet(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    SHEET_EXTS.iter().any(|e| lower.ends_with(e))
}

/// A PDF by name, in any case: Canvas keeps whatever the uploader typed.
pub fn is_pdf(rel: &str) -> bool {
    rel.to_ascii_lowercase().ends_with(".pdf")
}

/// `files.file_type` values that go through parse and embed, as a SQL list for
/// `lower(file_type) IN …`; built from `OFFICE_EXTS` so no query drifts.
pub fn pdf_backed_sql_list() -> String {
    let quoted: Vec<String> = std::iter::once("pdf")
        .chain(OFFICE_EXTS.iter().map(|e| e.trim_start_matches('.')))
        .map(|e| format!("'{e}'"))
        .collect();
    format!("({})", quoted.join(", "))
}

/// The PDF that parsing, embedding and viewing use: the file itself, the
/// converted sibling for Office documents, `None` otherwise.
pub fn doc_pdf_rel(rel: &str) -> Option<String> {
    if is_pdf(rel) {
        return Some(rel.to_string());
    }
    let lower = rel.to_ascii_lowercase();
    OFFICE_EXTS
        .iter()
        .any(|e| lower.ends_with(e))
        .then(|| format!("{rel}.pdf"))
}

/// The student's own files, which the scraper never writes to; downstream they
/// are ordinary library files.
pub const UPLOADS_DIR: &str = "uploads";

/// True for a data-dir-relative path inside some subject's uploads folder.
///
/// The delete command's entire guard: nothing else under `courses/` is the
/// user's to throw away.
pub fn is_upload_rel(rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    !rel.contains("..") && parts.len() > 3 && parts[0] == "courses" && parts[2] == UPLOADS_DIR
}

/// The student's own notes, written in the app as markdown; the other folder
/// the scraper never writes to. Downstream, ordinary library files.
pub const DOCUMENTS_DIR: &str = "documents";

/// Where a note's pictures go: `documents/assets/`, one folder for the
///
/// The folder on disk and the `![](assets/…)` prefix a note carries. One folder
/// because the names are stamps and a per-note folder would move on rename.
pub const DOCUMENT_ASSETS_DIR: &str = "assets";

/// True only for `courses/<code>/documents/<name>.md`.
///
/// The entire guard for writing, renaming and deleting a document: exactly
/// four segments, no traversal, a `.md` at the end — which also keeps
/// `documents/assets/` out.
pub fn is_document_rel(rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    !rel.contains("..")
        && parts.len() == 4
        && parts[0] == "courses"
        && !parts[1].is_empty()
        && parts[2] == DOCUMENTS_DIR
        && parts[3].len() > 3
        && parts[3].ends_with(".md")
}

/// Every category `category_from_path` can return, in the order a reader
/// One list, in reading order, that the `--category` flags validate against;
/// the test below fails if it drifts from the match.
pub const CATEGORIES: &[&str] = &[
    "home",
    "syllabus",
    "upload",
    "document",
    "page",
    "assignment",
    "quiz",
    "announcement",
    "ed",
    "file",
    "module",
    "image",
    "other",
];

pub fn category_from_path(path: &str) -> &'static str {
    match path {
        "home.md" => "home",
        "syllabus.md" => "syllabus",
        p if p.starts_with("uploads/") => "upload",
        p if p.starts_with("documents/") => "document",
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
    fn sign_out_keeps_the_attempt_record() {
        let dir = crate::test_support::Scratch::new("sign-out");
        mark_authenticated(&dir);
        write_private(&cookie_path(&dir), "canvas_session=a").unwrap();
        write_private(&sso_cookie_path(&dir), "idx=b").unwrap();
        write_private(&ed_token_path(&dir), "jwt").unwrap();
        std::fs::write(sign_in_record_path(&dir), r#"{"paused":"locked"}"#).unwrap();

        assert!(sign_out(&dir).unwrap());
        assert!(!cookie_path(&dir).exists());
        assert!(!sso_cookie_path(&dir).exists());
        assert!(!ed_token_path(&dir).exists());
        assert!(!auth_flag_path(&dir).exists());
        assert!(sign_in_record_path(&dir).exists());
        assert!(signed_out_path(&dir).exists());
        assert!(!sign_out(&dir).unwrap());

        mark_authenticated(&dir);
        assert!(!signed_out_path(&dir).exists());
    }

    #[cfg(unix)]
    #[test]
    fn private_writes_narrow_an_existing_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = crate::test_support::Scratch::new("write-private");
        let path = dir.join("ed-session.token");
        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        write_private(&path, "fresh").unwrap();
        assert_eq!(mode(&path), 0o600);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_private(&path, "tok").unwrap();
        assert_eq!(mode(&path), 0o600);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "tok");
    }

    #[test]
    fn identifier_matches_tauri_conf() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], IDENTIFIER);
    }

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
        assert_eq!(
            safe_filename("Lecture 1: Intro.pdf"),
            "Lecture_1__Intro.pdf"
        );
        // No traversal survives: dots collapse, separators become underscores.
        assert_eq!(safe_filename("../../etc/passwd"), "____etc_passwd");
        assert_eq!(safe_rel_path("files/a.pdf").unwrap(), "files/a.pdf");
        assert_eq!(safe_rel_path("../../x").unwrap(), "x");
        assert!(safe_rel_path("///").is_none());
    }

    /// Every category the scraper can write is listed.
    #[test]
    fn every_category_the_scraper_writes_is_listed() {
        let paths = [
            "home.md",
            "syllabus.md",
            "uploads/notes.pdf",
            "documents/week-3-notes.md",
            "pages/week-01.md",
            "assignments/a2.md",
            "quizzes/mid.md",
            "announcements/2026-07-14-welcome.md",
            "ed/0001-teams.md",
            "files/week-01.pdf",
            "modules/01-intro.md",
            "images/fig-3.png",
            "something-nobody-planned-for",
        ];
        for p in paths {
            let c = category_from_path(p);
            assert!(CATEGORIES.contains(&c), "{p} -> {c:?} is not in CATEGORIES");
        }
        // And nothing in the list is unreachable: every entry was just hit.
        let hit: Vec<&str> = paths.iter().map(|p| category_from_path(p)).collect();
        for c in CATEGORIES {
            assert!(hit.contains(c), "{c:?} is listed but no path produces it");
        }
    }

    #[test]
    fn doc_pdf_resolution() {
        assert_eq!(doc_pdf_rel("files/a.pdf").as_deref(), Some("files/a.pdf"));
        assert_eq!(
            doc_pdf_rel("files/SCAN.PDF").as_deref(),
            Some("files/SCAN.PDF")
        );
        assert_eq!(
            doc_pdf_rel("files/deck.pptx").as_deref(),
            Some("files/deck.pptx.pdf")
        );
        assert_eq!(
            doc_pdf_rel("files/notes.DOCX").as_deref(),
            Some("files/notes.DOCX.pdf")
        );
        assert_eq!(doc_pdf_rel("files/marks.xlsx"), None);
        assert_eq!(doc_pdf_rel("files/legacy.XLS"), None);
        assert_eq!(doc_pdf_rel("pages/intro.md"), None);
        assert_eq!(doc_pdf_rel("images/x.png"), None);
    }

    #[test]
    fn a_pdf_is_known_by_its_extension_in_any_case() {
        assert!(is_pdf("courses/X/files/a.pdf"));
        assert!(is_pdf("courses/X/files/SCAN.PDF"));
        assert!(is_pdf("courses/X/files/deck.pptx.Pdf"));
        assert!(!is_pdf("courses/X/files/deck.pptx"));
        assert!(!is_pdf("courses/X/files/pdf"));
    }

    #[test]
    fn the_sql_list_is_pdf_plus_every_office_extension() {
        assert_eq!(
            pdf_backed_sql_list(),
            "('pdf', 'pptx', 'docx', 'ppt', 'doc')"
        );
    }

    #[test]
    fn spreadsheets_are_known_by_extension_and_are_never_pdf_backed() {
        for rel in ["f/marks.xlsx", "f/MACROS.XLSM", "f/old.xls", "f/calc.ods"] {
            assert!(is_sheet(rel), "{rel}");
            assert!(doc_pdf_rel(rel).is_none(), "{rel}");
        }
        assert!(!is_sheet("f/marks.xlsx.md"));
        assert!(!is_sheet("f/deck.pptx"));
        assert!(SHEET_EXTS.iter().all(|e| !OFFICE_EXTS.contains(e)));
    }

    #[test]
    fn purging_a_sheet_takes_its_text_and_any_pdf_route_files() {
        let scratch = crate::test_support::Scratch::new("purge-sheet");
        let dir = scratch.join("courses/X/files");
        std::fs::create_dir_all(dir.join("m.xlsx_images")).unwrap();
        for name in [
            "m.xlsx",
            "m.xlsx.md",
            "m.xlsx.pdf",
            "m.xlsx.pages.json",
            "m.xlsx.emb.json",
        ] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        purge_parse_artifacts(&scratch, "courses/X/files/m.xlsx");
        let left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec!["m.xlsx".to_string()]);
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
        assert_eq!(category_from_path("documents/revision.md"), "document");
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

    #[test]
    fn only_a_markdown_note_in_a_subjects_documents_folder_is_writable() {
        assert!(is_document_rel("courses/COMP30026/documents/week-3.md"));
        assert!(is_document_rel(
            "courses/COMP30026_2026_SM2/documents/Untitled-2.md"
        ));
        // Traversal, however it is spelled, never resolves to a document.
        assert!(!is_document_rel(
            "courses/COMP30026/documents/../../oculus.db"
        ));
        assert!(!is_document_rel("courses/../documents/x.md"));
        // Exactly one level deep: a nested folder is not a place the app writes.
        assert!(!is_document_rel("courses/COMP30026/documents/drafts/x.md"));
        assert!(!is_document_rel("courses/COMP30026/documents"));
        // Only markdown, and only with a name in front of the extension.
        assert!(!is_document_rel("courses/COMP30026/documents/x.pdf"));
        assert!(!is_document_rel("courses/COMP30026/documents/.md"));
        // An upload is not a document, however it ends.
        assert!(!is_document_rel("courses/COMP30026/uploads/x.md"));
        assert!(!is_document_rel("courses//documents/x.md"));
        assert!(!is_document_rel("lectures/abc/documents/x.md"));
    }
}
