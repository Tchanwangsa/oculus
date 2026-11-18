//! A drawing still named `.canvas` that reaches a subject's own files after
//! the launch rename (`migrate::whiteboard`) becomes a whiteboard on the next
//! reconcile, or as it is imported.

use super::*;
use crate::library::drive::ops::guard::own_target;
use crate::library::drive::ops::reconcile::reconcile_own_files_at;
use crate::library::files::uploads::import_into;
use crate::library::files::ImportedFile;

const DRAWING: &str = r##"{"type":"oculus-canvas","version":1,"elements":[],"viewport":{"x":0,"y":0,"zoom":1},"background":"#ffffff"}"##;
const OBSIDIAN: &str = r#"{"nodes":[],"edges":[]}"#;

/// A library the launch rename has finished with.
async fn renamed_library(name: &str) -> Lib {
    let lib = Lib::new(name).await;
    lib.put(".library-move/done", b"drive");
    lib.put(".library-move/whiteboard", b"done");
    lib
}

async fn reconcile(lib: &Lib) -> (Vec<ImportedFile>, Vec<(String, String)>) {
    let (_, inserted, moved) = reconcile_own_files_at(&lib.dir, &lib.pool, CODE)
        .await
        .unwrap();
    let moved = moved.into_iter().map(|m| (m.from, m.to)).collect();
    (inserted, moved)
}

async fn row_at(lib: &Lib, rel: &str) -> Option<(i64, String, String, String, String)> {
    let id: Option<i64> = sqlx::query_scalar("SELECT id FROM files WHERE relative_path = ?1")
        .bind(rel)
        .fetch_optional(&lib.pool)
        .await
        .unwrap();
    match id {
        Some(id) => lib.row_of(id).await,
        None => None,
    }
}

async fn moved_to(lib: &Lib, rel: &str) -> Option<String> {
    crate::library::migrate::resolve_moved(&lib.pool, rel)
        .await
        .unwrap()
}

fn whiteboard_row(rel: &str) -> (i64, String, String, String, String) {
    (
        1,
        rel.to_string(),
        rel.rsplit('/').next().unwrap().to_string(),
        "whiteboard".into(),
        "whiteboard".into(),
    )
}

#[tokio::test]
async fn a_stray_canvas_drawing_becomes_a_whiteboard_with_a_row() {
    let lib = renamed_library("drive-legacy-stray").await;
    let (from, to) = (
        own(CODE, "Week 1/Board.canvas"),
        own(CODE, "Week 1/Board.whiteboard"),
    );
    lib.put(&from, DRAWING.as_bytes());

    let (inserted, moved) = reconcile(&lib).await;
    assert!(!lib.exists(&from));
    // The type stays until the drawing's next save.
    assert_eq!(lib.read(&to), DRAWING);
    let got: Vec<(&str, &str, &str)> = inserted
        .iter()
        .map(|f| {
            (
                f.relative_path.as_str(),
                f.file_type.as_str(),
                f.category.as_str(),
            )
        })
        .collect();
    assert_eq!(got, [(to.as_str(), "whiteboard", "whiteboard")]);
    assert!(moved.is_empty(), "it had no row to follow");
    assert_eq!(row_at(&lib, &to).await, Some(whiteboard_row(&to)));
    assert_eq!(moved_to(&lib, &from).await, Some(to.clone()));

    let (again, moved) = reconcile(&lib).await;
    assert!(again.is_empty() && moved.is_empty());
}

#[tokio::test]
async fn a_taken_name_steps_aside_and_nothing_is_overwritten() {
    let lib = renamed_library("drive-legacy-clash").await;
    lib.put(&own(CODE, "plan.whiteboard"), b"the student's own");
    lib.put(&own(CODE, "Plan.canvas"), DRAWING.as_bytes());
    // A row holds a name whose file is gone: it is not taken either.
    lib.row(&own(CODE, "Gone.whiteboard")).await;
    lib.put(&own(CODE, "Gone.canvas"), DRAWING.as_bytes());
    // The launch rename's rule for an earlier `Board.canvas` stays its own.
    lib.put(&own(CODE, "Board.whiteboard"), DRAWING.as_bytes());
    lib.row(&own(CODE, "Board.whiteboard")).await;
    sqlx::query("INSERT INTO path_moves (old_prefix, new_prefix) VALUES (?1, ?2)")
        .bind(own(CODE, "Board.canvas"))
        .bind(own(CODE, "Board.whiteboard"))
        .execute(&lib.pool)
        .await
        .unwrap();
    lib.put(&own(CODE, "Board.canvas"), DRAWING.as_bytes());

    reconcile(&lib).await;
    assert_eq!(lib.read(&own(CODE, "plan.whiteboard")), "the student's own");
    assert_eq!(lib.read(&own(CODE, "Plan (2).whiteboard")), DRAWING);
    assert_eq!(lib.read(&own(CODE, "Gone (2).whiteboard")), DRAWING);
    assert_eq!(lib.read(&own(CODE, "Board (2).whiteboard")), DRAWING);
    for stem in ["Plan", "Gone", "Board"] {
        assert!(!lib.exists(&own(CODE, &format!("{stem}.canvas"))));
        let rel = own(CODE, &format!("{stem} (2).whiteboard"));
        assert_eq!(row_at(&lib, &rel).await, Some(whiteboard_row(&rel)));
    }
    assert_eq!(
        moved_to(&lib, &own(CODE, "Board.canvas")).await,
        Some(own(CODE, "Board.whiteboard"))
    );
}

#[tokio::test]
async fn another_apps_canvas_and_a_synced_one_stay_as_they_are() {
    let lib = renamed_library("drive-legacy-kept").await;
    let obsidian = own(CODE, "obsidian.canvas");
    lib.put(&obsidian, OBSIDIAN.as_bytes());
    let synced = course_file("synced.canvas");
    lib.put(&synced, DRAWING.as_bytes());

    let (inserted, moved) = reconcile(&lib).await;
    assert!(moved.is_empty());
    assert_eq!(lib.read(&obsidian), OBSIDIAN);
    assert_eq!(lib.read(&synced), DRAWING);
    assert!(!lib.exists(&own(CODE, "obsidian.whiteboard")));
    // Another app's file is an upload like any other; a synced one is not
    // the reconcile's.
    let got: Vec<(&str, &str)> = inserted
        .iter()
        .map(|f| (f.relative_path.as_str(), f.category.as_str()))
        .collect();
    assert_eq!(got, [(obsidian.as_str(), "upload")]);
    assert!(row_at(&lib, &synced).await.is_none());
}

#[tokio::test]
async fn a_canvas_row_keeps_its_id_and_its_old_path_resolves() {
    let lib = renamed_library("drive-legacy-row").await;
    let (from, to) = (own(CODE, "Old.canvas"), own(CODE, "Old.whiteboard"));
    let id = lib.file(&from, DRAWING.as_bytes()).await;
    assert_eq!(lib.row_of(id).await.unwrap().4, "upload");

    let (inserted, moved) = reconcile(&lib).await;
    assert!(inserted.is_empty());
    assert_eq!(moved, [(from.clone(), to.clone())]);
    assert_eq!(lib.row_of(id).await, Some(whiteboard_row(&to)));
    assert_eq!(lib.read(&to), DRAWING);
    assert_eq!(
        moved_to(&lib, &format!("{from}#x")).await,
        Some(format!("{to}#x"))
    );
}

#[tokio::test]
async fn a_half_written_drawing_waits_for_a_later_pass() {
    let lib = renamed_library("drive-legacy-half").await;
    let (from, to) = (own(CODE, "Copying.canvas"), own(CODE, "Copying.whiteboard"));
    lib.put(&from, &DRAWING.as_bytes()[..30]);

    let (inserted, moved) = reconcile(&lib).await;
    assert!(inserted.is_empty() && moved.is_empty());
    assert!(lib.exists(&from) && !lib.exists(&to));
    assert!(
        row_at(&lib, &from).await.is_none(),
        "no upload row meanwhile"
    );

    lib.put(&from, DRAWING.as_bytes());
    let (inserted, _) = reconcile(&lib).await;
    assert_eq!(inserted.len(), 1);
    assert!(!lib.exists(&from));
    assert_eq!(lib.read(&to), DRAWING);
    assert_eq!(row_at(&lib, &to).await, Some(whiteboard_row(&to)));
}

#[tokio::test]
async fn an_imported_canvas_drawing_lands_as_a_whiteboard() {
    let lib = renamed_library("drive-legacy-import").await;
    let src = crate::test_support::Scratch::new("drive-legacy-import-src");
    crate::test_support::write_creating(src.join("Sketch.canvas"), DRAWING);
    crate::test_support::write_creating(src.join("obsidian.canvas"), OBSIDIAN);
    lib.put(&own(CODE, "Sketch.whiteboard"), b"another");
    let target = own_target(&lib.dir, CODE, None).unwrap();

    let outcomes = import_into(
        &lib.dir,
        &target,
        &[&src.join("Sketch.canvas"), &src.join("obsidian.canvas")],
    );
    let landed: Vec<(String, String, String)> = outcomes
        .iter()
        .map(|o| {
            let f = o.file.as_ref().unwrap();
            (
                f.relative_path.clone(),
                f.file_type.clone(),
                f.category.clone(),
            )
        })
        .collect();
    assert_eq!(
        landed,
        [
            (
                own(CODE, "Sketch (2).whiteboard"),
                "whiteboard".into(),
                "whiteboard".into()
            ),
            (
                own(CODE, "obsidian.canvas"),
                "canvas".into(),
                "upload".into()
            ),
        ]
    );
    assert_eq!(lib.read(&own(CODE, "Sketch (2).whiteboard")), DRAWING);
    assert_eq!(lib.read(&own(CODE, "Sketch.whiteboard")), "another");
}

#[tokio::test]
async fn an_old_path_through_the_drive_moves_folder_rule_reaches_the_renamed_drawing() {
    let lib = renamed_library("drive-legacy-chain").await;
    for (old, new) in [
        (own(CODE, "canvases"), course_rel(CODE)),
        (own(CODE, "uploads/b.canvas"), own(CODE, "b (2).canvas")),
    ] {
        sqlx::query("INSERT INTO path_moves (old_prefix, new_prefix) VALUES (?1, ?2)")
            .bind(old)
            .bind(new)
            .execute(&lib.pool)
            .await
            .unwrap();
    }
    lib.file(&own(CODE, "a.canvas"), DRAWING.as_bytes()).await;
    lib.put(&own(CODE, "b.canvas"), DRAWING.as_bytes());

    reconcile(&lib).await;
    assert!(lib.exists(&own(CODE, "a.whiteboard")));
    assert_eq!(
        moved_to(&lib, &own(CODE, "canvases/a.canvas#x")).await,
        Some(own(CODE, "a.whiteboard#x"))
    );
    assert_eq!(
        moved_to(&lib, &own(CODE, "canvases/b.canvas")).await,
        Some(own(CODE, "b.whiteboard"))
    );
    // A file renamed on a clash keeps its own rule.
    assert_eq!(
        moved_to(&lib, &own(CODE, "uploads/b.canvas")).await,
        Some(own(CODE, "b (2).canvas"))
    );
    assert_eq!(
        moved_to(&lib, &own(CODE, "canvases/c.pdf")).await,
        Some(own(CODE, "c.pdf"))
    );
}
