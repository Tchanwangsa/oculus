use std::path::Path;

use crate::library::layout::{derived, derived_rel, is_derived_text, DerivedKind};
use crate::test_support::Scratch;

#[test]
fn text_and_pictures_sit_beside_the_records_in_oculus() {
    let cases = [
        (DerivedKind::Markdown, "courses/X/Course/Files/week 1.md"),
        (
            DerivedKind::Pages,
            "courses/X/.oculus/derived/Course/Files/week 1.pages.json",
        ),
        (
            DerivedKind::Embeddings,
            "courses/X/.oculus/derived/Course/Files/week 1.emb.json",
        ),
        (DerivedKind::Images, "courses/X/Course/Files/week 1_images"),
    ];
    for (kind, want) in cases {
        assert_eq!(derived_rel("courses/X/Course/Files/week 1.pdf", kind), want);
    }
}

#[test]
fn an_own_pdf_mirrors_its_folder_under_derived() {
    assert_eq!(
        derived_rel("courses/X/scan.pdf", DerivedKind::Pages),
        "courses/X/.oculus/derived/scan.pages.json"
    );
    assert_eq!(
        derived_rel("courses/X/Exam prep/Ü.pdf", DerivedKind::Embeddings),
        "courses/X/.oculus/derived/Exam prep/Ü.emb.json"
    );
}

#[test]
fn an_office_file_derives_from_its_converted_pdf() {
    assert_eq!(
        derived_rel(
            "courses/X/Course/Files/deck.pptx.pdf",
            DerivedKind::Markdown
        ),
        "courses/X/Course/Files/deck.pptx.md"
    );
    assert_eq!(
        derived_rel("courses/X/Course/Files/marks.xlsx.pdf", DerivedKind::Images),
        "courses/X/Course/Files/marks.xlsx_images"
    );
}

#[test]
fn an_absolute_path_finds_its_subject() {
    assert_eq!(
        derived(
            Path::new("/lib/courses/X/Course/Files/SCAN.PDF"),
            DerivedKind::Pages
        ),
        Path::new("/lib/courses/X/.oculus/derived/Course/Files/SCAN.pages.json")
    );
    // Outside any subject, everything stays beside the PDF.
    assert_eq!(
        derived(Path::new("/lib/a/SCAN.PDF"), DerivedKind::Pages),
        Path::new("/lib/a/SCAN.pages.json")
    );
    assert_eq!(
        derived_rel("courses/X", DerivedKind::Pages),
        "courses/X.pages.json"
    );
}

#[test]
fn parsed_and_sheet_text_is_not_a_note() {
    let dir = Scratch::new("layout-derived-text");
    let write = |rel: &str| {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "x").unwrap();
    };
    write("courses/X/scan.pdf");
    write("courses/X/scan.md");
    write("courses/X/.oculus/derived/scan.pages.json");
    write("courses/X/marks.xlsx");
    write("courses/X/marks.xlsx.md");
    write("courses/X/draft.pdf");
    write("courses/X/draft.md");
    write("courses/X/notes.md");
    assert!(is_derived_text(&dir, "courses/X/scan.md"));
    assert!(is_derived_text(&dir, "courses/X/marks.xlsx.md"));
    // A PDF never parsed leaves the note of the same name a note.
    assert!(!is_derived_text(&dir, "courses/X/draft.md"));
    assert!(!is_derived_text(&dir, "courses/X/notes.md"));
}
