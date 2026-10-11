use super::*;

#[test]
fn identifier_matches_tauri_conf() {
    let conf: serde_json::Value =
        serde_json::from_str(include_str!("../../../tauri.conf.json")).unwrap();
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
