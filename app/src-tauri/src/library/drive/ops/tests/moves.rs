use super::*;
use crate::library::drive::ops::error::Kind;
use crate::library::drive::ops::moves::move_entries_at;
use crate::library::layout::{derived_rel, DerivedKind};

#[tokio::test]
async fn a_parsed_pdf_moves_with_its_rows_and_records_and_stays_parsed() {
    let lib = Lib::new("drive-move-parsed").await;
    let from = own(CODE, "slides.pdf");
    let id = lib.parsed_pdf(&from).await;
    lib.mkdir(&own(CODE, "Week 1"));

    let moved = move_entries_at(&lib.dir, &lib.pool, &rels(&[&from]), &own(CODE, "Week 1"))
        .await
        .unwrap();
    let to = own(CODE, "Week 1/slides.pdf");
    assert_eq!(moved, vec![(from.clone(), to.clone())]);

    let (subject, rel, ..) = lib.row_of(id).await.unwrap();
    assert_eq!((subject, rel.as_str()), (1, to.as_str()), "the same row");
    assert_eq!(lib.pages_of(id).await.len(), 1, "pages keep their file id");
    for kind in DerivedKind::ALL {
        assert!(lib.exists(&derived_rel(&to, kind)), "{kind:?} moved");
        assert!(
            !lib.exists(&derived_rel(&from, kind)),
            "{kind:?} left nothing"
        );
    }
    assert!(
        crate::parse::parse_mode(&lib.path(&to)).is_some(),
        "parsed, no re-parse needed"
    );
    assert!(lib
        .read(&derived_rel(&to, DerivedKind::Markdown))
        .contains("](slides_images/x.png)"));
}

#[tokio::test]
async fn a_folder_move_updates_every_row_under_it() {
    let lib = Lib::new("drive-move-folder").await;
    let a = lib.file(&own(CODE, "Exam/notes.md"), b"# n").await;
    let b = lib.parsed_pdf(&own(CODE, "Exam/Past/2024.pdf")).await;
    let outside = lib.file(&own(CODE, "Exam prep.md"), b"x").await;
    lib.mkdir(&own(CODE, "Archive"));

    move_entries_at(
        &lib.dir,
        &lib.pool,
        &rels(&[&own(CODE, "Exam")]),
        &own(CODE, "Archive"),
    )
    .await
    .unwrap();

    assert_eq!(
        lib.row_of(a).await.unwrap().1,
        own(CODE, "Archive/Exam/notes.md")
    );
    assert_eq!(
        lib.row_of(b).await.unwrap().1,
        own(CODE, "Archive/Exam/Past/2024.pdf")
    );
    assert_eq!(
        lib.row_of(outside).await.unwrap().1,
        own(CODE, "Exam prep.md"),
        "a sibling with the same prefix stays"
    );
    let moved_pdf = own(CODE, "Archive/Exam/Past/2024.pdf");
    assert!(
        lib.exists(&derived_rel(&moved_pdf, DerivedKind::Pages)),
        "the derived mirror moved"
    );
    assert!(!lib.exists(&derived_rel(
        &own(CODE, "Exam/Past/2024.pdf"),
        DerivedKind::Pages
    )));
    assert_eq!(lib.pages_of(b).await.len(), 1);
}

#[tokio::test]
async fn a_move_across_subjects_changes_the_subject_and_its_derived_folder() {
    let lib = Lib::new("drive-move-across").await;
    let from = own(CODE, "slides.pdf");
    let id = lib.parsed_pdf(&from).await;

    move_entries_at(&lib.dir, &lib.pool, &rels(&[&from]), &root(OTHER))
        .await
        .unwrap();

    let to = own(OTHER, "slides.pdf");
    let (subject, rel, ..) = lib.row_of(id).await.unwrap();
    assert_eq!((subject, rel), (2, to.clone()));
    assert!(derived_rel(&to, DerivedKind::Pages).contains(OTHER));
    assert!(lib.exists(&derived_rel(&to, DerivedKind::Pages)));
    assert!(lib.exists(&derived_rel(&to, DerivedKind::Embeddings)));
    assert_eq!(lib.pages_of(id).await.len(), 1);
}

#[tokio::test]
async fn synced_and_hidden_paths_are_read_only() {
    let lib = Lib::new("drive-move-readonly").await;
    let synced = course_file("a.pdf");
    lib.file(&synced, b"%PDF").await;
    let mine = own(CODE, "mine.pdf");
    lib.file(&mine, b"%PDF").await;
    lib.put(&own(CODE, ".oculus/agents/x.md"), b"x");

    let refused = |r: Result<_, crate::library::drive::ops::error::DriveError>| r.unwrap_err().kind;
    assert_eq!(
        refused(move_entries_at(&lib.dir, &lib.pool, &rels(&[&synced]), &root(CODE)).await),
        Kind::ReadOnly
    );
    let course = crate::library::layout::in_course(CODE, "Course");
    assert_eq!(
        refused(move_entries_at(&lib.dir, &lib.pool, &rels(&[&mine]), &course).await),
        Kind::ReadOnly
    );
    assert_eq!(
        refused(
            move_entries_at(
                &lib.dir,
                &lib.pool,
                &rels(&[&own(CODE, ".oculus")]),
                &root(OTHER)
            )
            .await
        ),
        Kind::ReadOnly
    );
    assert_eq!(
        refused(move_entries_at(&lib.dir, &lib.pool, &rels(&[&mine]), &own(CODE, ".oculus")).await),
        Kind::ReadOnly
    );
    assert!(lib.exists(&synced) && lib.exists(&mine));
}

#[tokio::test]
async fn a_taken_name_is_refused_and_nothing_moves() {
    let lib = Lib::new("drive-move-clash").await;
    let a = own(CODE, "a.pdf");
    lib.file(&a, b"one").await;
    lib.file(&own(CODE, "Week 1/a.pdf"), b"two").await;

    let error = move_entries_at(&lib.dir, &lib.pool, &rels(&[&a]), &own(CODE, "Week 1"))
        .await
        .unwrap_err();
    assert_eq!(error.kind, Kind::Exists);
    assert_eq!(lib.read(&a), "one");

    let error = move_entries_at(
        &lib.dir,
        &lib.pool,
        &rels(&[&own(CODE, "Week 1")]),
        &own(CODE, "Week 1"),
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, Kind::Io, "a folder can't go inside itself");
}

#[tokio::test]
async fn a_companion_s_name_taken_at_the_target_refuses_the_move() {
    let lib = Lib::new("drive-move-companion").await;
    let from = own(CODE, "slides.pdf");
    lib.parsed_pdf(&from).await;
    lib.put(&own(CODE, "Week 1/slides.md"), b"my own note");

    let error = move_entries_at(&lib.dir, &lib.pool, &rels(&[&from]), &own(CODE, "Week 1"))
        .await
        .unwrap_err();
    assert_eq!(error.kind, Kind::Exists);
    assert!(lib.exists(&from));
    assert_eq!(lib.read(&own(CODE, "Week 1/slides.md")), "my own note");
}

#[tokio::test]
async fn names_a_volume_folds_together_never_land_on_one_file() {
    let lib = Lib::new("drive-move-fold").await;
    for (one, two) in [
        ("A/Étude.pdf", "B/étude.pdf"),
        ("A/caf\u{e9}.pdf", "B/cafe\u{301}.pdf"),
        ("A/Straße.pdf", "B/STRASSE.pdf"),
    ] {
        let (one, two) = (own(CODE, one), own(CODE, two));
        let first = lib.file(&one, b"one").await;
        let second = lib.file(&two, b"two").await;
        lib.mkdir(&own(CODE, "C"));

        let error = move_entries_at(&lib.dir, &lib.pool, &rels(&[&one, &two]), &own(CODE, "C"))
            .await
            .unwrap_err();
        assert_eq!(error.kind, Kind::Exists, "{one} + {two}");
        assert_eq!(
            (lib.read(&one), lib.read(&two)),
            ("one".into(), "two".into())
        );
        assert_eq!(lib.row_of(first).await.unwrap().1, one);
        assert_eq!(lib.row_of(second).await.unwrap().1, two);
        std::fs::remove_file(lib.path(&one)).unwrap();
        std::fs::remove_file(lib.path(&two)).unwrap();
    }
}

#[tokio::test]
async fn rows_that_cannot_follow_put_a_merged_mirror_back() {
    let lib = Lib::new("drive-move-undo").await;
    let pdf = own(CODE, "Exam/a.pdf");
    lib.parsed_pdf(&pdf).await;
    lib.mkdir(&own(CODE, "Archive"));
    let moved = own(CODE, "Archive/Exam/a.pdf");
    let stale = derived_rel(&moved, DerivedKind::Pages);
    lib.put(&stale, b"stale record");
    let stale_mirror = stale.rsplit_once('/').unwrap().0.to_string();
    lib.put(&format!("{stale_mirror}/old.pages.json"), b"old");
    let record = lib.read(&derived_rel(&pdf, DerivedKind::Pages));
    sqlx::raw_sql(
        "CREATE TRIGGER refuse BEFORE UPDATE ON files BEGIN SELECT RAISE(ABORT, 'refused'); END;",
    )
    .execute(&lib.pool)
    .await
    .unwrap();

    let error = move_entries_at(
        &lib.dir,
        &lib.pool,
        &rels(&[&own(CODE, "Exam")]),
        &own(CODE, "Archive"),
    )
    .await
    .unwrap_err();

    assert_eq!(error.kind, Kind::Io);
    assert!(lib.exists(&pdf) && !lib.exists(&own(CODE, "Archive/Exam")));
    assert_eq!(
        lib.read(&derived_rel(&pdf, DerivedKind::Pages)),
        record,
        "the record is back"
    );
    assert!(lib.exists(&derived_rel(&pdf, DerivedKind::Embeddings)));
    assert_eq!(
        lib.read(&stale),
        "stale record",
        "the replaced file is back"
    );
    let mut left: Vec<String> = std::fs::read_dir(lib.path(&stale_mirror))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(
        left,
        ["a.pages.json", "old.pages.json"],
        "nothing set aside is left"
    );
}

#[tokio::test]
async fn a_committed_merge_leaves_nothing_set_aside() {
    let lib = Lib::new("drive-move-merge").await;
    let pdf = own(CODE, "Exam/a.pdf");
    lib.parsed_pdf(&pdf).await;
    lib.mkdir(&own(CODE, "Archive"));
    let moved = own(CODE, "Archive/Exam/a.pdf");
    let stale = derived_rel(&moved, DerivedKind::Pages);
    lib.put(&stale, b"stale record");

    move_entries_at(
        &lib.dir,
        &lib.pool,
        &rels(&[&own(CODE, "Exam")]),
        &own(CODE, "Archive"),
    )
    .await
    .unwrap();

    assert!(
        crate::parse::parse_mode(&lib.path(&moved)).is_some(),
        "the moved record won"
    );
    let mirror = stale.rsplit_once('/').unwrap().0;
    let mut left: Vec<String> = std::fs::read_dir(lib.path(mirror))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["a.emb.json", "a.pages.json"]);
    assert!(!lib.exists(&derived_rel(&pdf, DerivedKind::Pages)));
}
