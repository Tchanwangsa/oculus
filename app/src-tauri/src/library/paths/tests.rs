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
fn filenames_are_portable_to_windows_volumes() {
    for name in ["CON", "con.pdf", "NUL.txt", "COM1.md", "LPT9", "COM¹"] {
        assert_eq!(safe_filename(name), format!("_{name}"));
    }
    assert_eq!(safe_filename("notes..."), "notes_");
    assert_eq!(safe_dir("CON"), "_CON");
    assert_eq!(
        slash_path(r"courses\X\files\a.pdf"),
        "courses/X/files/a.pdf"
    );
}

#[test]
fn purging_an_own_pdf_keeps_a_note_the_parser_did_not_write() {
    let scratch = crate::test_support::Scratch::new("purge-own");
    let dir = scratch.join("courses/X/uploads");
    std::fs::create_dir_all(dir.join("notes_images")).unwrap();
    for name in ["notes.pdf", "notes.md", "notes.emb.json"] {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    purge_parse_artifacts(&scratch, "courses/X/uploads/notes.pdf");
    assert!(dir.join("notes.md").is_file(), "the student's note stays");
    assert!(!dir.join("notes.emb.json").exists());
    assert!(!dir.join("notes_images").exists());

    // With a parse record beside it, the markdown is the parser's.
    std::fs::write(dir.join("notes.pages.json"), b"{}").unwrap();
    purge_parse_artifacts(&scratch, "courses/X/uploads/notes.pdf");
    assert!(!dir.join("notes.md").exists());
    assert!(!dir.join("notes.pages.json").exists());
    assert!(dir.join("notes.pdf").is_file());
}

#[test]
fn purging_a_synced_pdf_takes_every_artifact() {
    let scratch = crate::test_support::Scratch::new("purge-synced");
    let dir = scratch.join("courses/X/files");
    std::fs::create_dir_all(dir.join("w1_images")).unwrap();
    for name in ["w1.pdf", "w1.md", "w1.pages.json", "w1.emb.json"] {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    purge_parse_artifacts(&scratch, "courses/X/files/w1.pdf");
    let left: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, vec!["w1.pdf".to_string()]);
    // A path that could leave the data dir purges nothing.
    purge_parse_artifacts(&scratch, "courses/X/files/../../../w1.pdf");
}
