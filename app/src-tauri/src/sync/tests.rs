use super::office::{
    is_csv_type, is_generic_binary, is_sheet_type, is_video, office_ext, office_ext_of,
};
use super::render::{
    content_type_of, file_toc_line, locked_until, rel_within_course, slug, up_to_course_root,
};
use super::*;
use crate::library::paths;

#[test]
fn slugs_are_lowercase_dashed_and_capped() {
    assert_eq!(
        slug("Welcome & Executive Summary"),
        "welcome-executive-summary"
    );
    assert_eq!(slug("  --Trim-- "), "trim");
    assert_eq!(slug(""), "untitled");
    assert_eq!(slug("!!!"), "untitled");
    assert_eq!(slug(&"a".repeat(80)).len(), 60);
}

#[test]
fn module_links_are_relative_to_the_course_root() {
    assert_eq!(
        rel_within_course("courses/ABC_2026/files/x.pdf"),
        "files/x.pdf"
    );
    assert_eq!(rel_within_course("files/x.pdf"), "files/x.pdf");
}

#[test]
fn assets_are_addressed_from_the_document_that_references_them() {
    assert_eq!(up_to_course_root("home.md"), "");
    assert_eq!(up_to_course_root("pages/week-one.md"), "../");
    assert_eq!(up_to_course_root("announcements/2026-08-14-x.md"), "../");
}

#[test]
fn spreadsheets_are_text_never_a_libreoffice_conversion() {
    for ct in SHEET_TYPES {
        assert!(is_sheet_type(ct, "Marks.xlsx"), "{ct}");
        assert_eq!(office_ext(ct), None, "{ct}");
    }
    assert!(is_sheet_type("application/vnd.ms-excel", "grades.csv"));
    assert!(is_sheet_type("application/octet-stream", "Marks.XLSM"));
    assert!(is_sheet_type("", "calc.ods"));
    assert!(!is_sheet_type("application/zip", "marks.xlsx"));
    assert!(!is_sheet_type("application/octet-stream", "deck.pptx"));
    assert_eq!(office_ext_of("marks.xlsx"), None);
}

#[test]
fn a_csv_is_a_sheet_whatever_canvas_labels_it() {
    for ct in [
        "text/csv",
        "application/csv",
        "text/plain",
        "application/vnd.ms-excel",
        "",
    ] {
        assert!(is_csv_type(ct, "Grades.CSV"), "{ct}");
    }
    assert!(!is_csv_type("text/csv", "grades.txt"));
    assert!(!is_csv_type("application/zip", "grades.csv"));
    assert!(!paths::is_pdf("grades.csv") && paths::is_sheet("grades.csv"));
}

#[test]
fn every_converted_type_is_a_pdf_backed_extension() {
    for (_, ext) in OFFICE_TYPES {
        assert!(
            paths::OFFICE_EXTS.contains(&format!(".{ext}").as_str()),
            "{ext}"
        );
    }
    assert_eq!(OFFICE_TYPES.len(), paths::OFFICE_EXTS.len());
}

#[test]
fn an_untyped_upload_falls_back_to_its_extension() {
    // The longer extension has to win, or "deck.pptx" converts as "ppt".
    assert_eq!(office_ext_of("deck.pptx"), Some("pptx"));
    assert_eq!(office_ext_of("old deck.PPT"), Some("ppt"));
    // Not an Office format, so the name buys it nothing.
    assert_eq!(office_ext_of("archive.zip"), None);
    assert_eq!(office_ext_of("notes.pdf"), None);

    assert!(is_generic_binary(""));
    assert!(is_generic_binary("application/octet-stream"));
    assert!(!is_generic_binary("application/pdf"));
}

#[test]
fn videos_are_known_by_type_or_an_untyped_upload_s_extension() {
    assert!(is_video("video/mp4", "Matrices Part 1.mp4"));
    assert!(is_video("video/quicktime", "untitled"));
    assert!(is_video("application/octet-stream", "Lecture 3.MP4"));
    assert!(is_video("", "demo.webm"));
    assert!(is_video("binary/octet-stream", "clip.m4v"));
    // A typed file is what its type says, whatever its name.
    assert!(!is_video("application/pdf", "slides.mp4"));
    assert!(!is_video("application/octet-stream", "archive.mkv.zip"));
    assert!(!is_video("audio/mpeg", "talk.mp3"));
    // Never fed to the parse/embed pipeline.
    assert!(paths::doc_pdf_rel("courses/X/files/a.mp4").is_none());
}

#[test]
fn a_module_video_lists_its_canvas_id_and_where_it_will_land() {
    let video = Fetched::Video {
        rel: "courses/MAST_2026/files/Matrices_Part_1.mp4".into(),
        canvas_id: 12345,
    };
    assert_eq!(
        file_toc_line("  ", "Matrices [Part 1]", &video),
        "  - [Matrices \\[Part 1\\]](../files/Matrices_Part_1.mp4) _(video 12345)_"
    );
    let saved = Fetched::Saved("courses/MAST_2026/files/w1.pdf".into());
    assert_eq!(
        file_toc_line("", "Week 1", &saved),
        "- [Week 1](../files/w1.pdf)"
    );
    assert_eq!(
        file_toc_line("", "Gone", &Fetched::Skipped),
        "- Gone _(file)_"
    );
}

#[test]
fn a_locked_file_names_its_unlock_date() {
    let open = serde_json::json!({ "locked_for_user": false });
    let dated = serde_json::json!({
        "locked_for_user": true,
        "lock_info": { "unlock_at": "2026-08-01T00:00:00Z" },
    });
    let undated = serde_json::json!({ "locked_for_user": true });
    assert_eq!(locked_until(&open), None);
    assert_eq!(locked_until(&dated).as_deref(), Some(" until 2026-08-01"));
    assert_eq!(locked_until(&undated).as_deref(), Some(""));
}

#[test]
fn content_type_ignores_charset_and_either_spelling() {
    let a = serde_json::json!({ "content-type": "application/pdf; charset=utf-8" });
    let b = serde_json::json!({ "content_type": "image/png" });
    assert_eq!(content_type_of(&a), "application/pdf");
    assert_eq!(content_type_of(&b), "image/png");
    assert_eq!(content_type_of(&serde_json::json!({})), "");
}
