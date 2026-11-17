use super::*;
use crate::library::drive::ops::copy::copy_to_own_at;
use crate::library::drive::ops::error::Kind;
use crate::library::layout::{derived_rel, DerivedKind};

#[tokio::test]
async fn a_copy_out_of_course_has_its_own_row_and_no_parse() {
    let lib = Lib::new("drive-copy").await;
    let synced = course_file("lecture.pdf");
    let source = lib.parsed_pdf(&synced).await;
    lib.mkdir(&own(CODE, "Exam"));

    let copies = copy_to_own_at(&lib.dir, &lib.pool, &rels(&[&synced]), &own(CODE, "Exam"))
        .await
        .unwrap();
    let copy = own(CODE, "Exam/lecture.pdf");
    assert_eq!(copies, std::slice::from_ref(&copy));

    let id: i64 = sqlx::query_scalar("SELECT id FROM files WHERE relative_path = ?1")
        .bind(&copy)
        .fetch_one(&lib.pool)
        .await
        .unwrap();
    assert_ne!(id, source);
    let (subject, _, _, file_type, category) = lib.row_of(id).await.unwrap();
    assert_eq!(
        (subject, file_type.as_str(), category.as_str()),
        (1, "pdf", "upload")
    );
    for kind in DerivedKind::ALL {
        assert!(
            !lib.exists(&derived_rel(&copy, kind)),
            "{kind:?} is not copied"
        );
    }
    assert!(lib.exists(&synced), "the original stays");

    let again = copy_to_own_at(&lib.dir, &lib.pool, &rels(&[&synced]), &own(CODE, "Exam"))
        .await
        .unwrap();
    assert_eq!(again, [own(CODE, "Exam/lecture (2).pdf")]);
}

#[tokio::test]
async fn a_folder_copy_skips_artifacts_and_keeps_converted_pdfs_and_pictures() {
    let lib = Lib::new("drive-copy-folder").await;
    let week = crate::library::layout::in_course(CODE, "Course/Files/Week 1");
    lib.parsed_pdf(&format!("{week}/a.pdf")).await;
    lib.put(&format!("{week}/deck.pptx"), b"pptx");
    lib.put(&format!("{week}/deck.pptx.pdf"), b"%PDF");
    lib.put(&format!("{week}/.assets/p.png"), b"png");

    let copies = copy_to_own_at(&lib.dir, &lib.pool, &rels(&[&week]), &root(OTHER))
        .await
        .unwrap();
    assert_eq!(copies, [own(OTHER, "Week 1")]);
    let mut names: Vec<String> = std::fs::read_dir(lib.path(&own(OTHER, "Week 1")))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, [".assets", "a.pdf", "deck.pptx", "deck.pptx.pdf"]);
    let copied: Vec<String> = sqlx::query_scalar(
        "SELECT relative_path FROM files WHERE subject_id = 2 ORDER BY relative_path",
    )
    .fetch_all(&lib.pool)
    .await
    .unwrap();
    assert_eq!(
        copied,
        [own(OTHER, "Week 1/a.pdf"), own(OTHER, "Week 1/deck.pptx")]
    );
}

#[tokio::test]
async fn nothing_is_copied_into_course() {
    let lib = Lib::new("drive-copy-readonly").await;
    let mine = own(CODE, "mine.pdf");
    lib.file(&mine, b"%PDF").await;
    let course = crate::library::layout::in_course(CODE, "Course/Files");
    lib.mkdir(&course);
    let error = copy_to_own_at(&lib.dir, &lib.pool, &rels(&[&mine]), &course)
        .await
        .unwrap_err();
    assert_eq!(error.kind, Kind::ReadOnly);
    let error = copy_to_own_at(
        &lib.dir,
        &lib.pool,
        &rels(&[&own(CODE, ".oculus/agents")]),
        &root(CODE),
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, Kind::ReadOnly);
}
