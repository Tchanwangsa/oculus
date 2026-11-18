use super::*;
use crate::library::drive::ops::reconcile::reconcile_own_files_at;
use crate::library::layout::{agents_rel, derived_rel, DerivedKind};
use crate::parse::InFlight;

async fn paths_of(lib: &Lib, subject: i64) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT relative_path FROM files WHERE subject_id = ?1 ORDER BY relative_path",
    )
    .bind(subject)
    .fetch_all(&lib.pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn own_files_at_any_depth_get_rows_with_their_kind() {
    let lib = Lib::new("drive-reconcile-depth").await;
    lib.put(&own(CODE, "brief.pdf"), b"%PDF");
    lib.put(&own(CODE, "Assignment 2/notes.md"), b"# hi");
    lib.put(&own(CODE, "Assignment 2/deep/er/board.whiteboard"), b"{}");
    lib.put(&own(OTHER, "theirs.pdf"), b"%PDF");

    let (subject, inserted, _) = reconcile_own_files_at(&lib.dir, &lib.pool, CODE)
        .await
        .unwrap();
    assert_eq!(subject, 1);
    let got: Vec<(&str, &str, &str, u64)> = inserted
        .iter()
        .map(|f| {
            (
                f.relative_path.rsplit('/').next().unwrap(),
                f.file_type.as_str(),
                f.category.as_str(),
                f.size_bytes,
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("board.whiteboard", "whiteboard", "whiteboard", 2),
            ("notes.md", "md", "document", 4),
            ("brief.pdf", "pdf", "upload", 4),
        ]
    );
    assert_eq!(
        paths_of(&lib, 1).await,
        [
            own(CODE, "Assignment 2/deep/er/board.whiteboard"),
            own(CODE, "Assignment 2/notes.md"),
            own(CODE, "brief.pdf"),
        ]
    );
    assert!(
        paths_of(&lib, 2).await.is_empty(),
        "another subject is left"
    );
}

#[tokio::test]
async fn synced_hidden_dot_and_linked_files_get_no_row() {
    let lib = Lib::new("drive-reconcile-skips").await;
    lib.put(&course_file("lecture.pdf"), b"%PDF");
    lib.put(&format!("{}/memory.md", agents_rel(CODE)), b"x");
    lib.put(&own(CODE, ".DS_Store"), b"x");
    lib.put(&own(CODE, ".staging/half.pdf"), b"%PDF");
    lib.put(&own(CODE, "Notes/.draft.md"), b"x");
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        lib.path(&course_file("lecture.pdf")),
        lib.path(&own(CODE, "link.pdf")),
    )
    .unwrap();

    let (_, inserted, _) = reconcile_own_files_at(&lib.dir, &lib.pool, CODE)
        .await
        .unwrap();
    assert!(
        inserted.is_empty(),
        "{:?}",
        inserted
            .iter()
            .map(|f| &f.relative_path)
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn parse_artifacts_and_converted_pdfs_get_no_row() {
    let lib = Lib::new("drive-reconcile-artifacts").await;
    let pdf = own(CODE, "Week 3/slides.pdf");
    lib.parsed_pdf(&pdf).await;
    lib.put(&own(CODE, "Week 3/essay.docx"), b"docx");
    lib.put(&own(CODE, "Week 3/essay.docx.pdf"), b"%PDF");
    // The student's own note beside a PDF nothing has parsed stays theirs.
    lib.put(&own(CODE, "draft.pdf"), b"%PDF");
    lib.put(&own(CODE, "draft.md"), b"mine");

    let (_, inserted, _) = reconcile_own_files_at(&lib.dir, &lib.pool, CODE)
        .await
        .unwrap();
    let mut got: Vec<&str> = inserted.iter().map(|f| f.relative_path.as_str()).collect();
    got.sort();
    assert_eq!(
        got,
        [
            own(CODE, "Week 3/essay.docx"),
            own(CODE, "draft.md"),
            own(CODE, "draft.pdf"),
        ]
    );
    assert!(lib.exists(&derived_rel(&pdf, DerivedKind::Markdown)));
}

#[tokio::test]
async fn markdown_a_running_parse_is_writing_waits_for_its_record() {
    let lib = Lib::new("drive-reconcile-inflight").await;
    let pdf = own(CODE, "reading.pdf");
    lib.put(&pdf, b"%PDF");
    lib.put(&derived_rel(&pdf, DerivedKind::Markdown), b"half");
    let key = crate::sync::parse_key(&lib.dir, &pdf);

    let claim = InFlight::shared().claim(&key);
    let (_, inserted, _) = reconcile_own_files_at(&lib.dir, &lib.pool, CODE)
        .await
        .unwrap();
    drop(claim);
    let got: Vec<&str> = inserted.iter().map(|f| f.relative_path.as_str()).collect();
    assert_eq!(got, [pdf.as_str()]);
}

#[tokio::test]
async fn existing_rows_are_kept_and_a_second_run_adds_nothing() {
    let lib = Lib::new("drive-reconcile-again").await;
    let kept = own(CODE, "kept.pdf");
    let id = lib.file(&kept, b"%PDF-1.4 longer").await;
    sqlx::query("UPDATE files SET parse_status = 'done', size_bytes = 99 WHERE id = ?1")
        .bind(id)
        .execute(&lib.pool)
        .await
        .unwrap();
    // A row whose file is gone is not this command's to remove.
    let gone = lib.row(&own(CODE, "gone.pdf")).await;
    lib.put(&own(CODE, "new.pdf"), b"%PDF");

    let (_, first, _) = reconcile_own_files_at(&lib.dir, &lib.pool, CODE)
        .await
        .unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].relative_path, own(CODE, "new.pdf"));
    let (_, second, _) = reconcile_own_files_at(&lib.dir, &lib.pool, CODE)
        .await
        .unwrap();
    assert!(second.is_empty());

    let (status, size): (Option<String>, i64) =
        sqlx::query_as("SELECT parse_status, size_bytes FROM files WHERE id = ?1")
            .bind(id)
            .fetch_one(&lib.pool)
            .await
            .unwrap();
    assert_eq!((status.as_deref(), size), (Some("done"), 99));
    assert!(lib.row_of(gone).await.is_some());
}

#[tokio::test]
async fn an_unknown_subject_is_refused() {
    let lib = Lib::new("drive-reconcile-unknown").await;
    assert!(reconcile_own_files_at(&lib.dir, &lib.pool, "NOPE10001")
        .await
        .is_err());
}
