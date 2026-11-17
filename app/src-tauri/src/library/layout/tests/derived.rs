use std::path::Path;

use crate::library::layout::{derived, derived_rel, DerivedKind};

#[test]
fn artifacts_sit_beside_the_pdf_named_off_its_stem() {
    let cases = [
        (DerivedKind::Markdown, "courses/X/files/week 1.md"),
        (DerivedKind::Pages, "courses/X/files/week 1.pages.json"),
        (DerivedKind::Embeddings, "courses/X/files/week 1.emb.json"),
        (DerivedKind::Images, "courses/X/files/week 1_images"),
    ];
    for (kind, want) in cases {
        assert_eq!(derived_rel("courses/X/files/week 1.pdf", kind), want);
    }
}

#[test]
fn an_office_file_derives_from_its_converted_pdf() {
    assert_eq!(
        derived_rel("courses/X/files/deck.pptx.pdf", DerivedKind::Markdown),
        "courses/X/files/deck.pptx.md"
    );
    assert_eq!(
        derived_rel("courses/X/files/marks.xlsx.pdf", DerivedKind::Images),
        "courses/X/files/marks.xlsx_images"
    );
    assert_eq!(
        derived(Path::new("/lib/a/SCAN.PDF"), DerivedKind::Pages),
        Path::new("/lib/a/SCAN.pages.json")
    );
    assert_eq!(
        derived_rel("courses/X/uploads/Ü.pdf", DerivedKind::Embeddings),
        "courses/X/uploads/Ü.emb.json"
    );
}
