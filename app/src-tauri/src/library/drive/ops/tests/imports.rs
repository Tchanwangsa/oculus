use super::*;
use crate::library::drive::ops::guard::own_target;
use crate::library::files::uploads::import_into;

#[tokio::test]
async fn a_folder_imports_recursively_one_outcome_per_file() {
    let lib = Lib::new("drive-import").await;
    let src = crate::test_support::Scratch::new("drive-import-src");
    let picked = src.join("Week 1");
    crate::test_support::write_creating(picked.join("slides.pdf"), b"%PDF");
    crate::test_support::write_creating(picked.join("Readings/paper.pdf"), b"%PDF");
    crate::test_support::write_creating(picked.join("notes.md"), b"# notes");
    crate::test_support::write_creating(picked.join(".DS_Store"), b"x");
    crate::test_support::write_creating(src.join("loose.pdf"), b"%PDF");
    lib.mkdir(&own(CODE, "Exam"));
    let target = own_target(&lib.dir, CODE, Some(&own(CODE, "Exam"))).unwrap();

    let outcomes = import_into(
        &lib.dir,
        &target,
        &[picked.as_path(), &src.join("loose.pdf")],
    );
    let landed: Vec<(String, String, String)> = outcomes
        .iter()
        .map(|o| {
            let f = o.file.as_ref().unwrap();
            (
                o.source.clone(),
                f.relative_path.clone(),
                f.category.clone(),
            )
        })
        .collect();
    assert_eq!(
        landed,
        [
            (
                "Week 1/Readings/paper.pdf".into(),
                own(CODE, "Exam/Week 1/Readings/paper.pdf"),
                "upload".into()
            ),
            (
                "Week 1/notes.md".into(),
                own(CODE, "Exam/Week 1/notes.md"),
                "document".into()
            ),
            (
                "Week 1/slides.pdf".into(),
                own(CODE, "Exam/Week 1/slides.pdf"),
                "upload".into()
            ),
            (
                "loose.pdf".into(),
                own(CODE, "Exam/loose.pdf"),
                "upload".into()
            ),
        ]
    );
    assert!(!lib.exists(&own(CODE, "Exam/Week 1/.DS_Store")));

    let again = import_into(&lib.dir, &target, &[picked.as_path()]);
    assert!(again[0]
        .file
        .as_ref()
        .unwrap()
        .relative_path
        .starts_with(&own(CODE, "Exam/Week 1 (2)/")));
}

#[tokio::test]
async fn a_clash_steps_aside_and_identical_bytes_reuse_the_name() {
    let lib = Lib::new("drive-import-clash").await;
    let src = crate::test_support::Scratch::new("drive-import-clash-src");
    crate::test_support::write_creating(src.join("a/notes.pdf"), b"one");
    crate::test_support::write_creating(src.join("b/notes.pdf"), b"two");
    let target = own_target(&lib.dir, CODE, None).unwrap();

    let first = import_into(&lib.dir, &target, &[&src.join("a/notes.pdf")]);
    let same = import_into(&lib.dir, &target, &[&src.join("a/notes.pdf")]);
    let other = import_into(&lib.dir, &target, &[&src.join("b/notes.pdf")]);
    let path = |o: &[crate::library::files::uploads::ImportOutcome]| {
        o[0].file.as_ref().unwrap().relative_path.clone()
    };
    assert_eq!(path(&first), own(CODE, "notes.pdf"));
    assert_eq!(path(&same), own(CODE, "notes.pdf"));
    assert_eq!(path(&other), own(CODE, "notes (2).pdf"));
}
