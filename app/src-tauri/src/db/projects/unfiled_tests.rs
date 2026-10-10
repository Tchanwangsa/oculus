//! Migration 37, and the writers over the schema it leaves behind.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

use super::*;

/// The tables as migration 27 wrote them (`project_id NOT NULL`), with
/// foreign keys on as in the app.
async fn pre_37() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");
    sqlx::raw_sql(
            "CREATE TABLE subjects (id INTEGER PRIMARY KEY, code TEXT);
             CREATE TABLE projects (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 subject_id  INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
                 name        TEXT    NOT NULL,
                 brief       TEXT,
                 status      TEXT    NOT NULL DEFAULT 'active',
                 starts_at   TEXT,
                 due_at      TEXT,
                 columns     TEXT    NOT NULL,
                 tags        TEXT    NOT NULL DEFAULT '[]',
                 event_id    TEXT,
                 position    REAL    NOT NULL DEFAULT 0,
                 source      TEXT    NOT NULL DEFAULT 'manual',
                 created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
                 updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
             );
             CREATE TABLE project_tasks (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 parent_id   INTEGER REFERENCES project_tasks(id) ON DELETE CASCADE,
                 title       TEXT    NOT NULL,
                 body        TEXT,
                 column_id   TEXT    NOT NULL,
                 position    REAL    NOT NULL,
                 starts_at   TEXT,
                 due_at      TEXT,
                 estimate_minutes INTEGER,
                 done_at     TEXT,
                 source      TEXT    NOT NULL DEFAULT 'manual',
                 created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
                 updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
             );
             CREATE INDEX idx_project_tasks_project ON project_tasks(project_id, column_id, position);
             CREATE INDEX idx_project_tasks_due ON project_tasks(due_at);",
        )
        .execute(&pool)
        .await
        .expect("pre-37 schema");
    pool
}

async fn migrated() -> SqlitePool {
    let pool = pre_37().await;
    sqlx::raw_sql(UNFILED_TASKS_SQL)
        .execute(&pool)
        .await
        .expect("migration 37");
    pool
}

async fn seed_project(pool: &SqlitePool, name: &str) -> i64 {
    create_project(
        pool,
        &NewProject {
            name: name.to_string(),
            subject_id: None,
            brief: None,
            starts_at: None,
            due_at: None,
            tags: vec![],
            source: "manual".to_string(),
        },
    )
    .await
    .expect("project")
}

async fn task_row(pool: &SqlitePool, id: i64) -> Option<(Option<i64>, Option<i64>, String)> {
    let row = sqlx::query("SELECT project_id, parent_id, title FROM project_tasks WHERE id = ?1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .expect("read");
    row.map(|r| (r.get("project_id"), r.get("parent_id"), r.get("title")))
}

async fn count(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM project_tasks")
        .fetch_one(pool)
        .await
        .expect("count")
}

/// Fails if {@link UNFILED_TASKS_SQL}'s `parent_id` names the old table;
/// then checks the rename kept both cascades.
#[tokio::test]
async fn the_rebuild_keeps_parents_children_and_their_cascades() {
    let pool = pre_37().await;
    let project = seed_project(&pool, "Essay").await;
    sqlx::raw_sql(&format!(
        "INSERT INTO project_tasks (id, project_id, parent_id, title, column_id, position)
             VALUES (1, {project}, NULL, 'Draft', 'todo', 0),
                    (2, {project}, 1, 'Outline', 'todo', 1),
                    (3, {project}, 1, 'Cite', 'done', 0),
                    (4, {project}, NULL, 'Proofread', 'backlog', 0);"
    ))
    .execute(&pool)
    .await
    .expect("seed tasks");

    sqlx::raw_sql(UNFILED_TASKS_SQL)
        .execute(&pool)
        .await
        .expect("migration 37");

    assert_eq!(count(&pool).await, 4, "every row survived the rebuild");
    assert_eq!(
        task_row(&pool, 2).await.unwrap().1,
        Some(1),
        "subtask still names its parent"
    );
    assert_eq!(task_row(&pool, 3).await.unwrap().1, Some(1));
    assert_eq!(
        task_row(&pool, 4).await.unwrap().0,
        Some(project),
        "and its project"
    );

    let fresh = create_tasks(
        &pool,
        Some(project),
        &[NewTask {
            title: "Submit".into(),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .expect("create after the rebuild");
    assert!(
        fresh[0] > 4,
        "new ids continue past the copied ones (got {})",
        fresh[0]
    );

    delete_task(&pool, 1).await.expect("delete the parent");
    assert!(
        task_row(&pool, 2).await.is_none(),
        "parent → subtask cascade"
    );
    assert!(task_row(&pool, 3).await.is_none());

    sqlx::query("DELETE FROM projects WHERE id = ?1")
        .bind(project)
        .execute(&pool)
        .await
        .expect("delete the project");
    assert_eq!(count(&pool).await, 0, "project → tasks cascade");
}

#[tokio::test]
async fn an_unfiled_task_round_trips() {
    let pool = migrated().await;

    let ids = create_tasks(
        &pool,
        None,
        &[
            NewTask {
                title: "Renew Myki".into(),
                ..Default::default()
            },
            NewTask {
                title: "Book a haircut".into(),
                column: Some("todo".into()),
                key: Some("hair".into()),
                ..Default::default()
            },
            NewTask {
                title: "Ring the salon".into(),
                parent: Some(ParentRef::Key("hair".into())),
                ..Default::default()
            },
        ],
        "manual",
    )
    .await
    .expect("unfiled create");

    let first = task(&pool, ids[0]).await.unwrap().unwrap();
    assert_eq!(first.project_id, None);
    assert_eq!(first.column_id, "backlog");

    let child = task(&pool, ids[2]).await.unwrap().unwrap();
    assert_eq!(child.parent_id, Some(ids[1]));
    assert_eq!(
        child.project_id, None,
        "a subtask of an unfiled task is unfiled"
    );
    assert_eq!(child.column_id, "todo", "and inherits its parent's column");

    assert_eq!(first.position, 0.0);

    move_task(&pool, ids[0], "done", None, None)
        .await
        .expect("move");
    let done = task(&pool, ids[0]).await.unwrap().unwrap();
    assert_eq!(done.column_id, "done");
    assert!(
        done.done_at.is_some(),
        "landing in a done column stamps done_at"
    );
    move_task(&pool, ids[0], "doing", None, None)
        .await
        .expect("move back");
    assert!(
        task(&pool, ids[0])
            .await
            .unwrap()
            .unwrap()
            .done_at
            .is_none(),
        "leaving clears it"
    );

    update_task(
        &pool,
        ids[0],
        &TaskPatch {
            title: Some("Renew the Myki".into()),
            ..Default::default()
        },
    )
    .await
    .expect("patch");
    assert_eq!(task_row(&pool, ids[0]).await.unwrap().2, "Renew the Myki");
}

#[tokio::test]
async fn an_unfiled_column_is_checked_against_the_default_board() {
    let pool = migrated().await;
    let err = create_tasks(
        &pool,
        None,
        &[NewTask {
            title: "x".into(),
            column: Some("nowhere".into()),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .expect_err("an unknown column is refused");
    assert!(err.contains("an unfiled task"), "{err}");
    assert!(err.contains("backlog, todo, doing, done"), "{err}");
    assert_eq!(count(&pool).await, 0, "and nothing was written");
}

#[tokio::test]
async fn a_subtask_cannot_cross_between_filed_and_unfiled() {
    let pool = migrated().await;
    let project = seed_project(&pool, "Essay").await;
    let filed = create_tasks(
        &pool,
        Some(project),
        &[NewTask {
            title: "Draft".into(),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .expect("filed parent")[0];
    let unfiled = create_tasks(
        &pool,
        None,
        &[NewTask {
            title: "Errand".into(),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .expect("unfiled parent")[0];

    let err = create_tasks(
        &pool,
        None,
        &[NewTask {
            title: "Outline".into(),
            parent: Some(ParentRef::Id(filed)),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .expect_err("an unfiled subtask of a filed parent");
    assert!(
        err.contains(&format!("belongs to project {project}")),
        "{err}"
    );

    let err = create_tasks(
        &pool,
        Some(project),
        &[NewTask {
            title: "Outline".into(),
            parent: Some(ParentRef::Id(unfiled)),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .expect_err("a filed subtask of an unfiled parent");
    assert!(err.contains("belongs to no project"), "{err}");
}

#[tokio::test]
async fn refiling_a_task_out_of_a_project_maps_the_column_by_kind() {
    let pool = migrated().await;
    let project = seed_project(&pool, "Essay").await;
    let id = create_tasks(
        &pool,
        Some(project),
        &[NewTask {
            title: "Draft".into(),
            column: Some("doing".into()),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .expect("filed")[0];

    assert_eq!(refile_task(&pool, id, None).await.expect("unfile"), 1);
    let row = task(&pool, id).await.unwrap().unwrap();
    assert_eq!(row.project_id, None);
    // `doing` is `active`; the first active column is `todo`.
    assert_eq!(row.column_id, "todo");
    assert!(row.done_at.is_none());
    assert!(tasks(&pool, project).await.unwrap().is_empty());

    assert_eq!(
        refile_task(&pool, id, Some(project)).await.expect("file"),
        1
    );
    let row = task(&pool, id).await.unwrap().unwrap();
    assert_eq!(row.project_id, Some(project));
    assert_eq!(row.column_id, "todo");
    assert_eq!(refile_task(&pool, id, Some(project)).await.unwrap(), 0);
}

#[tokio::test]
async fn refiling_carries_the_subtasks_with_it() {
    let pool = migrated().await;
    let project = seed_project(&pool, "Essay").await;
    let ids = create_tasks(
        &pool,
        None,
        &[
            NewTask {
                title: "Essay".into(),
                column: Some("doing".into()),
                key: Some("p".into()),
                ..Default::default()
            },
            NewTask {
                title: "Outline".into(),
                parent: Some(ParentRef::Key("p".into())),
                column: Some("done".into()),
                ..Default::default()
            },
            NewTask {
                title: "Draft".into(),
                parent: Some(ParentRef::Key("p".into())),
                ..Default::default()
            },
        ],
        "manual",
    )
    .await
    .expect("unfiled breakdown");

    assert_eq!(
        refile_task(&pool, ids[0], Some(project))
            .await
            .expect("refile"),
        3
    );
    for id in &ids {
        let row = task(&pool, *id).await.unwrap().unwrap();
        assert_eq!(row.project_id, Some(project), "task {id} came along");
    }
    assert_eq!(
        task(&pool, ids[1]).await.unwrap().unwrap().column_id,
        "done"
    );
    assert!(task(&pool, ids[1])
        .await
        .unwrap()
        .unwrap()
        .done_at
        .is_some());
    assert_eq!(
        task(&pool, ids[2]).await.unwrap().unwrap().column_id,
        "todo"
    );
    let parent = task(&pool, ids[0]).await.unwrap().unwrap();
    let sibling = task(&pool, ids[2]).await.unwrap().unwrap();
    assert_ne!(parent.position, sibling.position);
    assert!(
        parent.position < sibling.position,
        "the parent goes in first"
    );

    let err = refile_task(&pool, ids[1], None)
        .await
        .expect_err("a lone subtask");
    assert!(
        err.contains(&format!("subtask of task {}", ids[0])),
        "{err}"
    );
    assert_eq!(
        task(&pool, ids[1]).await.unwrap().unwrap().project_id,
        Some(project),
        "and it did not move"
    );
}

#[tokio::test]
async fn refiling_refuses_a_board_with_no_column_of_that_kind() {
    let pool = migrated().await;
    let project = seed_project(&pool, "Essay").await;
    sqlx::query("UPDATE projects SET columns = ?1 WHERE id = ?2")
        .bind(r#"[{"id":"now","name":"Now","kind":"active"}]"#)
        .bind(project)
        .execute(&pool)
        .await
        .expect("narrow board");

    let id = create_tasks(
        &pool,
        None,
        &[NewTask {
            title: "Submit".into(),
            column: Some("done".into()),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .expect("unfiled")[0];

    let err = refile_task(&pool, id, Some(project))
        .await
        .expect_err("no done column to land in");
    assert!(err.contains("has no \"done\" column"), "{err}");
    assert_eq!(
        task(&pool, id).await.unwrap().unwrap().project_id,
        None,
        "nothing moved"
    );
}

#[tokio::test]
async fn all_tasks_spans_projects_and_can_ask_for_the_unfiled_alone() {
    let pool = migrated().await;
    let project = seed_project(&pool, "Essay").await;
    create_tasks(
        &pool,
        Some(project),
        &[NewTask {
            title: "Draft".into(),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .expect("filed");
    create_tasks(
        &pool,
        None,
        &[NewTask {
            title: "Errand".into(),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .expect("unfiled");

    let all = all_tasks(&pool, TaskScope::All).await.expect("all");
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].title, "Errand", "unfiled first");
    assert_eq!(all[1].project_id, Some(project));

    let unfiled = all_tasks(&pool, TaskScope::Unfiled).await.expect("unfiled");
    assert_eq!(unfiled.len(), 1);
    assert_eq!(unfiled[0].title, "Errand");
}

#[tokio::test]
async fn a_move_will_not_take_a_neighbour_from_the_other_side() {
    let pool = migrated().await;
    let project = seed_project(&pool, "Essay").await;
    let filed = create_tasks(
        &pool,
        Some(project),
        &[NewTask {
            title: "Draft".into(),
            column: Some("todo".into()),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .expect("filed")[0];
    let unfiled = create_tasks(
        &pool,
        None,
        &[NewTask {
            title: "Errand".into(),
            column: Some("todo".into()),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .expect("unfiled")[0];

    let err = move_task(&pool, unfiled, "todo", Some(filed), None)
        .await
        .expect_err("a neighbour from another project");
    assert!(err.contains("does not belong to the same project"), "{err}");
}
