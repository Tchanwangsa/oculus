use super::*;
use crate::library::drive::ops::create::create_folder_at;
use crate::library::drive::ops::error::Kind;
use crate::library::drive::ops::guard::own_target;

#[tokio::test]
async fn a_folder_is_made_once_and_only_among_own_files() {
    let lib = Lib::new("drive-create").await;
    let rel = own(CODE, "Exam prep");
    assert_eq!(create_folder_at(&lib.dir, &rel).unwrap(), rel);
    assert!(lib.path(&rel).is_dir());
    assert_eq!(
        create_folder_at(&lib.dir, &rel).unwrap_err().kind,
        Kind::Exists
    );
    let nested = own(CODE, "Exam prep/Week 1");
    assert_eq!(create_folder_at(&lib.dir, &nested).unwrap(), nested);

    let course = crate::library::layout::in_course(CODE, "Course/Mine");
    assert_eq!(
        create_folder_at(&lib.dir, &course).unwrap_err().kind,
        Kind::ReadOnly
    );
    assert_eq!(
        create_folder_at(&lib.dir, &own(CODE, "Course"))
            .unwrap_err()
            .kind,
        Kind::InvalidName
    );
    assert_eq!(
        create_folder_at(&lib.dir, &own(CODE, "Missing/x"))
            .unwrap_err()
            .kind,
        Kind::NotFound
    );
}

#[tokio::test]
async fn a_creator_s_target_must_be_an_own_folder_of_its_subject() {
    let lib = Lib::new("drive-target").await;
    lib.mkdir(&own(CODE, "Exam"));
    assert_eq!(own_target(&lib.dir, CODE, None).unwrap().rel, root(CODE));
    assert_eq!(
        own_target(&lib.dir, CODE, Some(&own(CODE, "Exam")))
            .unwrap()
            .rel,
        own(CODE, "Exam")
    );
    assert_eq!(
        own_target(&lib.dir, CODE, Some(&own(OTHER, "")))
            .unwrap_err()
            .kind,
        Kind::InvalidName,
        "an empty segment is no path"
    );
    assert_eq!(
        own_target(&lib.dir, OTHER, Some(&own(CODE, "Exam")))
            .unwrap_err()
            .kind,
        Kind::Io
    );
    let course = crate::library::layout::in_course(CODE, "Course");
    lib.mkdir(&course);
    assert_eq!(
        own_target(&lib.dir, CODE, Some(&course)).unwrap_err().kind,
        Kind::ReadOnly
    );
}

#[tokio::test]
async fn a_subject_segment_that_leaves_or_hides_is_refused() {
    let lib = Lib::new("drive-create-subject").await;
    for rel in [
        "courses/../x",
        "courses/./x",
        "courses//x",
        "courses/.hidden/x",
        "courses/x",
    ] {
        let error = create_folder_at(&lib.dir, rel).unwrap_err();
        assert!(
            matches!(error.kind, Kind::InvalidName | Kind::ReadOnly),
            "{rel}: {error:?}"
        );
    }
    assert!(!lib.dir.join("x").exists(), "nothing beside courses/");
    assert!(!lib.dir.join("courses/x").exists());
    assert!(!lib.dir.join("courses/.hidden").exists());
    assert_eq!(
        create_folder_at(&lib.dir, "courses/GHOST_2026_SM2/x")
            .unwrap_err()
            .kind,
        Kind::NotFound,
        "a subject folder is never made"
    );
    assert!(!lib.dir.join("courses/GHOST_2026_SM2").exists());
}
