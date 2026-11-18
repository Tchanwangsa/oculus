//! The whiteboard rename on a library already in the Drive tree: drawings,
//! their rows and records, notes that link to them and old paths, a name
//! clash, a `.canvas` that is not a drawing, a crash mid-way, and a new
//! library. The last test runs both steps on the old-layout fixture.

use std::path::Path;

use super::fixture::{block, exists, file, library, pool, read, CODE, S};
use super::resume::snapshot;
use crate::library::migrate::apply::STOPPED;
use crate::library::migrate::whiteboard::{dry_run, run, Outcome};
use crate::library::migrate::{at_startup, resolve_moved, Hooks};
use crate::test_support::Scratch;

const DRAWING: &str = r##"{"type":"oculus-canvas","version":1,"elements":[],"viewport":{"x":0,"y":0,"zoom":1},"background":"#ffffff"}"##;
const OBSIDIAN: &str = r#"{"nodes":[],"edges":[]}"#;

fn s(rel: &str) -> String {
    format!("{S}/{rel}")
}

/// A library in the Drive tree, the Drive move marked done, with drawings
/// still named `.canvas`.
fn drive_library(name: &str) -> Scratch {
    let d = Scratch::new(name);
    file(&d, ".library-move/done", "drive");
    file(&d, &s("Board.canvas"), DRAWING);
    file(&d, &s("Exam prep/Plan.canvas"), DRAWING);
    file(&d, &s("Exam prep/plan.whiteboard"), "the student's own");
    file(&d, &s("obsidian.canvas"), OBSIDIAN);
    file(&d, &s("Course/Files/synced.canvas"), DRAWING);
    file(
        &d,
        &s("notes.md"),
        "[board](Board.canvas) and [plan](<Exam prep/Plan.canvas>) and [o](obsidian.canvas)\n",
    );
    file(&d, &s("nb.notebook"), DRAWING);
    file(
        &d,
        "canvas-pages/7/pages.json",
        &format!(r#"{{"canvas":"{S}/nb.notebook","pageCount":1,"pages":[]}}"#),
    );
    file(&d, "canvas-pages/7/page-001.png", "png");
    file(&d, "recovery/canvases/1.json", "draft");
    block(database(&d));
    d
}

async fn database(d: &Path) {
    let pool = pool(d).await;
    crate::library::migrate::db::ensure(&pool).await.unwrap();
    let files = [
        (1, "Board.canvas", "canvas"),
        (2, "Exam prep/Plan.canvas", "canvas"),
        (3, "Exam prep/plan.whiteboard", "whiteboard"),
        (4, "obsidian.canvas", "canvas"),
        (5, "Gone.canvas", "canvas"),
        (6, "notes.md", "document"),
        (7, "nb.notebook", "notebook"),
    ];
    let mut sql = format!("INSERT INTO subjects (id, code, name) VALUES (1, '{CODE}', 'One');");
    for (id, inner, category) in files {
        let name = inner.rsplit('/').next().unwrap();
        let ext = name.rsplit('.').next().unwrap();
        sql.push_str(&format!(
            "INSERT INTO files (id, subject_id, filename, relative_path, file_type, category)
             VALUES ({id}, 1, '{name}', '{S}/{inner}', '{ext}', '{category}');"
        ));
    }
    sql.push_str(&format!(
        "INSERT INTO document_versions (file_id, kind, text, hash) VALUES (6, 'auto', '[b](Board.canvas) old', 'h');
         INSERT INTO revision_notes (subject_id, title, source_kind, source_ref)
           VALUES (1, 'Sketch', 'file', '{S}/Board.canvas');
         INSERT INTO chat_proposals (item_id, block, thread_id, kind, state, initial_state) VALUES
           (1, 0, 1, 'note', '{{\"source\":\"{S}/Exam prep/Plan.canvas\"}}', '{{}}');
         INSERT INTO path_moves (old_prefix, new_prefix) VALUES
           ('{S}/canvases', '{S}'), ('{S}/uploads', '{S}');"
    ));
    sqlx::raw_sql(&sql).execute(&pool).await.unwrap();
    pool.close().await;
}

fn files_row(d: &Path, id: i64) -> (String, String, String, String) {
    block(async {
        let pool = pool(d).await;
        sqlx::query_as(
            "SELECT relative_path, filename, file_type, category FROM files WHERE id = ?1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap()
    })
}

fn moved(d: &Path, rel: &str) -> Option<String> {
    block(async { resolve_moved(&pool(d).await, rel).await.unwrap() })
}

#[test]
fn canvases_become_whiteboards_with_their_rows_and_old_paths() {
    let d = drive_library("whiteboard-rename");
    let plan = dry_run(&d).unwrap().expect("a plan");
    assert!(!exists(&d, ".library-move/whiteboard.json"));
    assert_eq!(plan.rules.len(), 3);
    let Outcome::Renamed(summary) = run(&d, &Hooks::default()).unwrap() else {
        panic!("expected the rename to run");
    };
    assert!(!summary.resumed);

    assert_eq!(read(&d, &s("Board.whiteboard")), DRAWING);
    assert!(!exists(&d, &s("Board.canvas")));
    // `plan.whiteboard` is the same name on the volume: the drawing takes
    // ` (2)` and the student's file is left as it was.
    assert_eq!(read(&d, &s("Exam prep/Plan (2).whiteboard")), DRAWING);
    assert_eq!(
        read(&d, &s("Exam prep/plan.whiteboard")),
        "the student's own"
    );
    // Not an Oculus drawing, or not the student's: kept.
    assert_eq!(read(&d, &s("obsidian.canvas")), OBSIDIAN);
    assert!(exists(&d, &s("Course/Files/synced.canvas")));
    assert!(summary.left.iter().any(|l| l.contains("obsidian.canvas")));

    assert_eq!(
        files_row(&d, 1),
        (
            s("Board.whiteboard"),
            "Board.whiteboard".into(),
            "whiteboard".into(),
            "whiteboard".into()
        )
    );
    assert_eq!(files_row(&d, 2).0, s("Exam prep/Plan (2).whiteboard"));
    assert_eq!(files_row(&d, 3).0, s("Exam prep/plan.whiteboard"));
    assert_eq!(
        files_row(&d, 4),
        (
            s("obsidian.canvas"),
            "obsidian.canvas".into(),
            "canvas".into(),
            "upload".into()
        )
    );
    // A row whose drawing is gone follows the rename too.
    assert_eq!(files_row(&d, 5).0, s("Gone.whiteboard"));
    assert_eq!(files_row(&d, 5).3, "whiteboard");

    assert_eq!(
        read(&d, &s("notes.md")),
        "[board](Board.whiteboard) and [plan](<Exam prep/Plan (2).whiteboard>) and [o](obsidian.canvas)\n"
    );
    block(async {
        let pool = pool(&d).await;
        let version: String = sqlx::query_scalar("SELECT text FROM document_versions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(version, "[b](Board.whiteboard) old");
        let source: String = sqlx::query_scalar("SELECT source_ref FROM revision_notes")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(source, s("Board.whiteboard"));
        let state: String = sqlx::query_scalar("SELECT state FROM chat_proposals")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(state.contains("Plan (2).whiteboard"), "{state}");
    });

    // Old paths resolve, through the Drive move's folder rule too.
    assert_eq!(
        moved(&d, &s("Board.canvas#x")),
        Some(s("Board.whiteboard#x"))
    );
    assert_eq!(
        moved(&d, &s("canvases/Board.canvas")),
        Some(s("Board.whiteboard"))
    );
    assert_eq!(moved(&d, &s("canvases/other.pdf")), Some(s("other.pdf")));

    assert!(!exists(&d, "canvas-pages"));
    assert_eq!(read(&d, "whiteboard-pages/7/page-001.png"), "png");
    let manifest: serde_json::Value =
        serde_json::from_str(&read(&d, "whiteboard-pages/7/pages.json")).unwrap();
    assert_eq!(manifest["whiteboard"], s("nb.notebook"));
    assert!(manifest.get("canvas").is_none());
    assert!(
        crate::library::paths::read_notebook_pages(&d, 7).is_some(),
        "the app reads the manifest"
    );
    assert!(!exists(&d, "recovery/canvases"));
    assert_eq!(read(&d, "recovery/whiteboards/1.json"), "draft");

    assert!(exists(&d, ".library-move/whiteboard"));
    assert_eq!(read(&d, ".library-move/done"), "drive");
    assert!(matches!(
        run(&d, &Hooks::default()).unwrap(),
        Outcome::AlreadyDone
    ));
    assert!(dry_run(&d).unwrap().is_none());
}

#[test]
fn a_run_stopped_mid_rename_resumes_to_the_same_end() {
    let whole = drive_library("whiteboard-whole");
    run(&whole, &Hooks::default()).unwrap();

    let cut = drive_library("whiteboard-cut");
    let stopped = run(
        &cut,
        &Hooks {
            stop_after: Some(3),
            ..Hooks::default()
        },
    );
    assert_eq!(stopped.unwrap_err(), STOPPED);
    assert!(exists(&cut, ".library-move/whiteboard.json"));
    assert!(!exists(&cut, ".library-move/whiteboard"));
    // The rows wait for every rename.
    assert_eq!(files_row(&cut, 1).0, s("Board.canvas"));
    // A dry run shows the journal as it stands.
    let plan = dry_run(&cut).unwrap().unwrap();
    assert_eq!(plan.steps.iter().filter(|s| s.done).count(), 3);

    let Outcome::Renamed(summary) = run(&cut, &Hooks::default()).unwrap() else {
        panic!("expected the rename to finish");
    };
    assert!(summary.resumed);
    assert_eq!(snapshot(&cut), snapshot(&whole));
}

#[test]
fn a_crash_after_the_database_commit_finishes_without_changing_it() {
    let whole = drive_library("whiteboard-whole-2");
    run(&whole, &Hooks::default()).unwrap();

    let cut = drive_library("whiteboard-cut-2");
    run(&cut, &Hooks::default()).unwrap();
    // As if the process died after the commit, before the marker.
    std::fs::remove_file(cut.join(".library-move/whiteboard")).unwrap();
    let Outcome::Renamed(summary) = run(&cut, &Hooks::default()).unwrap() else {
        panic!("expected the rename to finish");
    };
    assert!(summary.resumed);
    assert_eq!(snapshot(&cut), snapshot(&whole));
}

#[test]
fn records_already_under_the_new_name_are_never_overwritten() {
    let d = drive_library("whiteboard-records");
    file(&d, "whiteboard-pages/7/pages.json", "newer");
    file(&d, "recovery/canvases/2.json", "other draft");
    file(&d, "recovery/whiteboards/1.json", "newer draft");
    let Outcome::Renamed(summary) = run(&d, &Hooks::default()).unwrap() else {
        panic!("expected the rename to run");
    };
    assert_eq!(read(&d, "whiteboard-pages/7/pages.json"), "newer");
    assert_eq!(read(&d, "canvas-pages/7/page-001.png"), "png");
    assert_eq!(read(&d, "recovery/whiteboards/1.json"), "newer draft");
    assert_eq!(read(&d, "recovery/whiteboards/2.json"), "other draft");
    assert_eq!(read(&d, "recovery/canvases/1.json"), "draft");
    assert_eq!(
        summary
            .left
            .iter()
            .filter(|l| l.contains("kept, since"))
            .count(),
        2
    );
}

#[test]
fn a_new_library_only_gets_the_marker() {
    let d = Scratch::new("whiteboard-fresh");
    file(&d, "courses/X/Course/Files/a.pdf", "pdf");
    // The Drive move first, then this; each leaves its own marker.
    at_startup(&d).unwrap();
    assert_eq!(read(&d, ".library-move/done"), "drive");
    assert_eq!(read(&d, ".library-move/whiteboard"), "done");
    assert!(!exists(&d, ".library-move/whiteboard.json"));
    assert!(matches!(
        run(&d, &Hooks::default()).unwrap(),
        Outcome::AlreadyDone
    ));
}

#[test]
fn the_rename_waits_for_the_drive_move() {
    let d = Scratch::new("whiteboard-waits");
    let err = run(&d, &Hooks::default()).unwrap_err();
    assert!(err.contains("Drive tree"), "{err}");
    assert!(!exists(&d, ".library-move/whiteboard"));
}

#[test]
fn an_old_library_moves_then_renames_at_launch() {
    let d = library("whiteboard-legacy");
    file(&d, &s("canvases/board.canvas"), DRAWING);
    at_startup(&d).unwrap();
    assert_eq!(read(&d, &s("board.whiteboard")), DRAWING);
    assert!(!exists(&d, &s("canvases")));
    assert_eq!(
        files_row(&d, 13),
        (
            s("board.whiteboard"),
            "board.whiteboard".into(),
            "whiteboard".into(),
            "whiteboard".into()
        )
    );
    assert_eq!(
        moved(&d, &s("canvases/board.canvas")),
        Some(s("board.whiteboard"))
    );
    let manifest: serde_json::Value =
        serde_json::from_str(&read(&d, "whiteboard-pages/14/pages.json")).unwrap();
    assert_eq!(manifest["whiteboard"], s("nb.notebook"));
}

#[test]
fn two_drawings_of_one_name_keep_their_own_old_paths() {
    // The Drive move flattened `canvases/a.canvas` to `a.canvas` and an
    // `uploads/a.canvas` beside it to `a (2).canvas`, with a rule of its own.
    let d = Scratch::new("whiteboard-same-name");
    file(&d, ".library-move/done", "drive");
    file(&d, &s("a.canvas"), DRAWING);
    file(&d, &s("a (2).canvas"), DRAWING);
    block(async {
        let pool = pool(&d).await;
        crate::library::migrate::db::ensure(&pool).await.unwrap();
        sqlx::raw_sql(&format!(
            "INSERT INTO subjects (id, code, name) VALUES (1, '{CODE}', 'One');
             INSERT INTO files (id, subject_id, filename, relative_path, file_type, category) VALUES
               (1, 1, 'a.canvas', '{S}/a.canvas', 'canvas', 'canvas'),
               (2, 1, 'a (2).canvas', '{S}/a (2).canvas', 'canvas', 'canvas');
             INSERT INTO path_moves (old_prefix, new_prefix) VALUES
               ('{S}/uploads/a.canvas', '{S}/a (2).canvas'),
               ('{S}/canvases', '{S}'), ('{S}/uploads', '{S}');"
        ))
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
    });
    run(&d, &Hooks::default()).unwrap();
    assert_eq!(moved(&d, &s("canvases/a.canvas")), Some(s("a.whiteboard")));
    assert_eq!(
        moved(&d, &s("uploads/a.canvas#x")),
        Some(s("a (2).whiteboard#x"))
    );
    assert_eq!(moved(&d, &s("a.canvas")), Some(s("a.whiteboard")));
    assert_eq!(moved(&d, &s("a (2).canvas")), Some(s("a (2).whiteboard")));
    assert_eq!(moved(&d, &s("uploads/b.pdf")), Some(s("b.pdf")));
}

#[test]
fn an_old_librarys_gone_drawing_keeps_a_rule_through_both_steps() {
    let d = library("whiteboard-legacy-gone");
    // Row 13 is `canvases/board.canvas`; its file is gone.
    std::fs::remove_file(d.join(s("canvases/board.canvas"))).unwrap();
    at_startup(&d).unwrap();
    assert_eq!(
        files_row(&d, 13),
        (
            s("board.whiteboard"),
            "board.whiteboard".into(),
            "whiteboard".into(),
            "whiteboard".into()
        )
    );
    assert_eq!(
        moved(&d, &s("canvases/board.canvas")),
        Some(s("board.whiteboard"))
    );
    assert_eq!(moved(&d, &s("board.canvas")), Some(s("board.whiteboard")));
}

#[test]
fn a_row_spelled_in_another_unicode_form_follows_its_drawing() {
    // The disk holds the name decomposed (NFD), the row composed (NFC).
    let (nfd, nfc) = ("Re\u{301}sume\u{301}.canvas", "R\u{e9}sum\u{e9}.canvas");
    let d = Scratch::new("whiteboard-nfd");
    file(&d, ".library-move/done", "drive");
    file(&d, &s(nfd), DRAWING);
    block(async {
        let pool = pool(&d).await;
        crate::library::migrate::db::ensure(&pool).await.unwrap();
        sqlx::raw_sql(&format!(
            "INSERT INTO subjects (id, code, name) VALUES (1, '{CODE}', 'One');
             INSERT INTO files (id, subject_id, filename, relative_path, file_type, category)
               VALUES (1, 1, '{nfc}', '{S}/{nfc}', 'canvas', 'canvas');
             INSERT INTO revision_notes (subject_id, title, source_kind, source_ref)
               VALUES (1, 'CV', 'file', '{S}/{nfc}');"
        ))
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
    });
    run(&d, &Hooks::default()).unwrap();
    let to = s("Re\u{301}sume\u{301}.whiteboard");
    assert_eq!(
        files_row(&d, 1),
        (
            to.clone(),
            "Re\u{301}sume\u{301}.whiteboard".into(),
            "whiteboard".into(),
            "whiteboard".into()
        )
    );
    assert_eq!(moved(&d, &s(nfc)), Some(to.clone()));
    assert_eq!(moved(&d, &s(nfd)), Some(to.clone()));
    let source: String = block(async {
        sqlx::query_scalar("SELECT source_ref FROM revision_notes")
            .fetch_one(&pool(&d).await)
            .await
            .unwrap()
    });
    assert_eq!(source, to);
}
