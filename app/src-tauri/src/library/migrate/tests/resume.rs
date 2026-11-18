//! A run cut short, by a crash or a rename across volumes, resumes from the
//! journal and ends where an uninterrupted run does.

use std::path::Path;

use super::fixture::{block, exists, library, pool};
use crate::library::migrate::apply::STOPPED;
use crate::library::migrate::{run, Hooks, Outcome};

/// Everything a run leaves, with the data dir's own path taken out.
pub(super) fn snapshot(d: &Path) -> Vec<String> {
    let home = d.to_string_lossy().into_owned();
    let encoded = crate::library::paths::slash_path(d).replace('/', "%2F");
    let mut out = Vec::new();
    walk(d, d, &mut out);
    out.retain(|line| !line.starts_with(".library-move") && !line.starts_with("oculus.db"));
    out.iter_mut()
        .for_each(|l| *l = l.replace(&home, "<data>").replace(&encoded, "<data>"));
    block(async {
        let pool = pool(d).await;
        let rows: Vec<(i64, String, String, String)> =
            sqlx::query_as("SELECT id, relative_path, category, filename FROM files ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        out.extend(rows.into_iter().map(|r| format!("files {r:?}")));
        let lectures: Vec<(String, Option<String>, Option<String>)> =
            sqlx::query_as("SELECT id, folder_rel, video_path FROM lectures ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        out.extend(
            lectures
                .into_iter()
                .map(|r| format!("lectures {r:?}").replace(&home, "<data>")),
        );
        let moves: Vec<(String, String)> =
            sqlx::query_as("SELECT old_prefix, new_prefix FROM path_moves ORDER BY old_prefix")
                .fetch_all(&pool)
                .await
                .unwrap();
        out.extend(moves.into_iter().map(|r| format!("path_moves {r:?}")));
    });
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let path = e.path();
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let kind = e.file_type().unwrap();
        if kind.is_symlink() {
            out.push(format!(
                "{rel} -> {}",
                std::fs::read_link(&path).unwrap().display()
            ));
        } else if kind.is_dir() {
            out.push(format!("{rel}/"));
            walk(root, &path, out);
        } else {
            let body = std::fs::read(&path).unwrap();
            out.push(format!("{rel}: {}", String::from_utf8_lossy(&body)));
        }
    }
}

#[test]
fn a_run_stopped_mid_move_resumes_to_the_same_end() {
    let whole = library("migrate-whole");
    assert!(matches!(
        run(&whole, &Hooks::default()).unwrap(),
        Outcome::Moved(_)
    ));

    let cut = library("migrate-cut");
    let stopped = run(
        &cut,
        &Hooks {
            stop_after: Some(6),
            ..Hooks::default()
        },
    );
    assert_eq!(stopped.unwrap_err(), STOPPED);
    assert!(exists(&cut, ".library-move/journal.json"));
    assert!(!exists(&cut, ".library-move/done"));
    let Outcome::Moved(summary) = run(&cut, &Hooks::default()).unwrap() else {
        panic!("expected the move to finish");
    };
    assert!(summary.resumed);
    assert_eq!(snapshot(&cut), snapshot(&whole));
}

#[test]
fn a_crash_after_a_rename_before_its_log_line_is_found_done() {
    let whole = library("migrate-whole-2");
    assert!(matches!(
        run(&whole, &Hooks::default()).unwrap(),
        Outcome::Moved(_)
    ));

    let cut = library("migrate-cut-2");
    run(
        &cut,
        &Hooks {
            stop_after: Some(3),
            ..Hooks::default()
        },
    )
    .unwrap_err();
    // The next rename landed but the crash took its log line and the
    // journal's states with it.
    let mut plan = crate::library::migrate::journal::load(&cut)
        .unwrap()
        .unwrap();
    plan.steps.iter_mut().for_each(|s| s.done = false);
    let next = plan
        .steps
        .iter()
        .find_map(|s| match &s.op {
            crate::library::migrate::Op::Rename { from, to } if exists(&cut, from) => {
                Some((from.clone(), to.clone()))
            }
            _ => None,
        })
        .unwrap();
    std::fs::create_dir_all(cut.join(&next.1).parent().unwrap()).unwrap();
    std::fs::rename(cut.join(&next.0), cut.join(&next.1)).unwrap();
    crate::library::migrate::journal::save(&cut, &plan).unwrap();
    assert!(matches!(
        run(&cut, &Hooks::default()).unwrap(),
        Outcome::Moved(_)
    ));
    assert_eq!(snapshot(&cut), snapshot(&whole));
}

#[test]
fn a_rename_across_volumes_stops_the_move_without_copying() {
    let d = library("migrate-xdev");
    let err = run(
        &d,
        &Hooks {
            cross_device_at: Some(2),
            ..Hooks::default()
        },
    )
    .unwrap_err();
    assert!(err.contains("different volumes"), "{err}");
    assert!(!exists(&d, ".library-move/done"));
    // The database is untouched until every rename has landed.
    block(async {
        let pool = pool(&d).await;
        let layout: Option<String> =
            sqlx::query_scalar("SELECT value FROM settings WHERE key = 'library_layout'")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert_eq!(layout, None);
        let rel: String = sqlx::query_scalar("SELECT relative_path FROM files WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rel, "courses/COMP1_2026/files/lec1.pdf");
    });
    // Once the volumes are sorted out, the next launch finishes it.
    assert!(matches!(
        run(&d, &Hooks::default()).unwrap(),
        Outcome::Moved(_)
    ));
    assert!(exists(&d, ".library-move/done"));
}
