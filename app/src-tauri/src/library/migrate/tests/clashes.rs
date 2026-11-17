//! Names that meet on the way into one folder: stale rows, Unicode forms,
//! a renamed sibling a note links to, and folders named like the layout's.

use super::fixture::{block, exists, file, library, pool, read, S};
use crate::library::migrate::{run, Hooks, Outcome};

fn s(rel: &str) -> String {
    format!("{S}/{rel}")
}

fn rows(d: &std::path::Path, sql: &str) {
    block(async {
        let pool = pool(d).await;
        sqlx::raw_sql(sql).execute(&pool).await.unwrap();
    });
}

fn path_of(d: &std::path::Path, id: i64) -> String {
    block(async {
        let pool = pool(d).await;
        sqlx::query_scalar("SELECT relative_path FROM files WHERE id = ?1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
    })
}

#[test]
fn a_stale_row_never_takes_the_path_a_moved_file_lands_on() {
    let d = library("migrate-stale-row");
    file(&d, &s("uploads/old.pdf"), "%PDF on disk");
    rows(
        &d,
        &format!(
            "INSERT INTO files (id, subject_id, filename, relative_path, file_type, category) VALUES
               (30, 1, 'old.pdf', '{S}/documents/old.pdf', 'pdf', 'upload'),
               (31, 1, 'old.pdf', '{S}/uploads/old.pdf', 'pdf', 'upload');"
        ),
    );
    assert!(matches!(
        run(&d, &Hooks::default()).unwrap(),
        Outcome::Moved(_)
    ));
    assert_eq!(
        path_of(&d, 31),
        s("old.pdf"),
        "the file on disk keeps its name"
    );
    assert_eq!(
        path_of(&d, 30),
        s("old (2).pdf"),
        "the stale row steps aside"
    );
    assert!(matches!(
        run(&d, &Hooks::default()).unwrap(),
        Outcome::AlreadyDone
    ));
}

#[test]
fn names_differing_only_in_unicode_form_clash_like_the_volume_says() {
    let d = library("migrate-nfd");
    let (nfc, nfd) = ("caf\u{e9}.md", "cafe\u{301}.md");
    file(&d, &s(&format!("uploads/{nfc}")), "nfc");
    file(&d, &s(&format!("documents/{nfd}")), "nfd");
    assert!(matches!(
        run(&d, &Hooks::default()).unwrap(),
        Outcome::Moved(_)
    ));
    let names: Vec<String> = std::fs::read_dir(d.join(S))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("caf"))
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.iter().any(|n| n.contains(" (2)")), "{names:?}");
}

#[test]
fn a_link_follows_a_sibling_renamed_on_a_clash_not_the_file_now_beside_it() {
    let d = library("migrate-sibling-link");
    file(&d, &s("uploads/brief.pdf"), "%PDF the upload");
    file(&d, &s("documents/brief.pdf"), "%PDF the note's");
    file(
        &d,
        &s("documents/plan.md"),
        "see [brief](brief.pdf) and [lec](../files/lec1.pdf)\n",
    );
    assert!(matches!(
        run(&d, &Hooks::default()).unwrap(),
        Outcome::Moved(_)
    ));
    assert_eq!(read(&d, &s("brief (2).pdf")), "%PDF the note's");
    assert_eq!(
        read(&d, &s("plan.md")),
        "see [brief](<brief (2).pdf>) and [lec](Course/Files/lec1.pdf)\n"
    );
}

#[test]
fn a_root_folder_named_like_the_course_folder_steps_aside_first() {
    let d = library("migrate-reserved-root");
    file(&d, &s("course/mine.txt"), "the student's");
    file(&d, &s(".Oculus/x.txt"), "the student's too");
    assert!(matches!(
        run(&d, &Hooks::default()).unwrap(),
        Outcome::Moved(_)
    ));
    assert_eq!(read(&d, &s("course (mine)/mine.txt")), "the student's");
    assert_eq!(read(&d, &s(".Oculus (mine)/x.txt")), "the student's too");
    assert!(
        !exists(&d, &s("course (mine)/Files")),
        "nothing synced moved in"
    );
    let course: Vec<String> = std::fs::read_dir(d.join(S))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.eq_ignore_ascii_case("course"))
        .collect();
    assert_eq!(
        course,
        ["Course"],
        "the synced folder takes the layout's spelling"
    );
    assert!(exists(&d, &s("Course/Files/lec1.pdf")));
    assert!(exists(
        &d,
        &s(".oculus/derived/Course/Files/lec1.pages.json")
    ));
}

#[cfg(unix)]
#[test]
fn a_linked_recordings_folder_is_refused() {
    let d = library("migrate-linked-lectures");
    let elsewhere = crate::test_support::Scratch::new("migrate-lectures-elsewhere");
    std::fs::rename(d.join("lectures"), elsewhere.join("lectures")).unwrap();
    std::os::unix::fs::symlink(elsewhere.join("lectures"), d.join("lectures")).unwrap();
    let err = run(&d, &Hooks::default()).unwrap_err();
    assert!(err.contains("is a link to"), "{err}");
    assert!(exists(&d, &s("files/lec1.pdf")), "nothing moved");
    assert!(crate::library::migrate::dry_run(&d).is_err());
}
