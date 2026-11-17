use super::*;
use crate::library::drive::ops::delete::{delete_entries_at, delete_preview_at};
use crate::library::drive::ops::error::Kind;
use crate::library::layout::{derived_rel, DerivedKind};

#[tokio::test]
async fn the_preview_lists_every_file_below_a_folder_and_nothing_derived() {
    let lib = Lib::new("drive-delete-preview").await;
    lib.parsed_pdf(&own(CODE, "Exam/Past/2024.pdf")).await;
    lib.file(&own(CODE, "Exam/notes.md"), b"x").await;
    lib.file(&own(CODE, "loose.txt"), b"x").await;
    lib.put(&own(CODE, "Exam/.assets/p.png"), b"png");

    let listed = delete_preview_at(
        &lib.dir,
        &rels(&[&own(CODE, "Exam"), &own(CODE, "loose.txt")]),
    )
    .unwrap();
    assert_eq!(
        listed,
        [
            own(CODE, "Exam/Past/2024.pdf"),
            own(CODE, "Exam/notes.md"),
            own(CODE, "loose.txt"),
        ]
    );
    assert_eq!(
        delete_preview_at(&lib.dir, &rels(&[&course_file("x.pdf")]))
            .unwrap_err()
            .kind,
        Kind::ReadOnly
    );
}

#[tokio::test]
async fn a_delete_takes_rows_pages_versions_and_artifacts() {
    let lib = Lib::new("drive-delete").await;
    let pdf = own(CODE, "Exam/2024.pdf");
    let id = lib.parsed_pdf(&pdf).await;
    let note = lib.file(&own(CODE, "Exam/notes.md"), b"x").await;
    sqlx::query(
        "INSERT INTO document_versions (file_id, kind, text, hash) VALUES (?1, 'auto', 'x', 'h')",
    )
    .bind(note)
    .execute(&lib.pool)
    .await
    .unwrap();
    let single = own(CODE, "slides.pdf");
    let single_id = lib.parsed_pdf(&single).await;

    delete_entries_at(&lib.dir, &lib.pool, &rels(&[&own(CODE, "Exam"), &single]))
        .await
        .unwrap();

    for gone in [id, note, single_id] {
        assert!(lib.row_of(gone).await.is_none());
        assert!(lib.pages_of(gone).await.is_empty());
    }
    let versions: i64 = sqlx::query_scalar("SELECT count(*) FROM document_versions")
        .fetch_one(&lib.pool)
        .await
        .unwrap();
    assert_eq!(versions, 0);
    assert!(!lib.exists(&own(CODE, "Exam")));
    for kind in DerivedKind::ALL {
        assert!(!lib.exists(&derived_rel(&pdf, kind)), "{kind:?}");
        assert!(!lib.exists(&derived_rel(&single, kind)), "{kind:?}");
    }
    assert!(!lib.exists(&single));
}

#[tokio::test]
async fn course_files_are_never_deleted() {
    let lib = Lib::new("drive-delete-readonly").await;
    let synced = course_file("a.pdf");
    let id = lib.file(&synced, b"%PDF").await;
    let error = delete_entries_at(&lib.dir, &lib.pool, &rels(&[&synced]))
        .await
        .unwrap_err();
    assert_eq!(error.kind, Kind::ReadOnly);
    let error = delete_entries_at(&lib.dir, &lib.pool, &rels(&[&root(CODE)]))
        .await
        .unwrap_err();
    assert_eq!(error.kind, Kind::ReadOnly);
    assert!(lib.exists(&synced) && lib.row_of(id).await.is_some());
}
