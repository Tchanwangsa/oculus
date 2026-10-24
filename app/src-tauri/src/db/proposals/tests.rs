use super::*;
use crate::{
    db::projects,
    harness::{store, Provider},
    test_support::Scratch,
};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

async fn setup() -> (SqlitePool, i64, i64) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for migration in crate::db::migrations::all() {
        sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO subjects (id,code,name,is_current) VALUES (1,'COMP10001_2026_SM2','Subject',1)").execute(&pool).await.unwrap();
    let thread = store::create_thread(&pool, Provider::Codex, None, Some(1), None, "hello")
        .await
        .unwrap();
    let item = reply(&pool, thread).await;
    (pool, thread, item)
}
async fn reply(pool: &SqlitePool, thread: i64) -> i64 {
    sqlx::query(
        "INSERT INTO harness_items (thread_id,kind,content) VALUES (?1,'assistant','proposal')",
    )
    .bind(thread)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}
async fn card(pool: &SqlitePool, thread: i64, item: i64, kind: &str, state: &Value) -> Proposal {
    open(pool, thread, item, 0, kind, state, None)
        .await
        .unwrap()
}
async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn batch_apply_is_atomic_and_a_second_click_cannot_create_again() {
    let (pool, t, i) = setup().await;
    let bad = json!({"project":null,"items":[{"title":"First","tags":["Implicit"]},{"title":"Bad","column":"missing"}]});
    let p = card(&pool, t, i, "NewTasks", &bad).await;
    assert!(apply(&pool, &p.id, &bad).await.is_err());
    assert_eq!(count(&pool, "project_tasks").await, 0);
    assert_eq!(get(&pool, &p.id).await.unwrap().status, "pending");
    let implicit: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_tags WHERE name='Implicit'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(implicit, 0);
    let good = json!({"project":null,"items":[{"title":"First","tags":["Implicit"]}]});
    let (a, b) = tokio::join!(apply(&pool, &p.id, &good), apply(&pool, &p.id, &good));
    assert_ne!(a.is_ok(), b.is_ok());
    assert_eq!(count(&pool, "project_tasks").await, 1);
    undo(&pool, &p.id).await.unwrap();
    assert_eq!(count(&pool, "project_tasks").await, 0);
    assert_eq!(count(&pool, "task_tags").await, 5);
    assert!(apply(&pool, &p.id, &good).await.is_err());
}

#[tokio::test]
async fn amendments_only_supersede_pending_cards_in_the_same_thread() {
    let (pool, t, i) = setup().await;
    let value = json!({"project":null,"items":[{"title":"First"}]});
    let p = card(&pool, t, i, "NewTasks", &value).await;
    let edited = json!({"project":null,"items":[{"title":"Edited by student"}]});
    edit(&pool, &p.id, &edited).await.unwrap();
    let next = reply(&pool, t).await;
    let mut amending = value.clone();
    amending["amends"] = json!(p.id);
    let q = open(&pool, t, next, 0, "NewTasks", &amending, Some(&p.id))
        .await
        .unwrap();
    assert_eq!(
        get(&pool, &p.id).await.unwrap().superseded_by.as_deref(),
        Some(q.id.as_str())
    );
    assert_eq!(q.amended_from.as_ref(), Some(&edited));
    assert!(apply(&pool, &p.id, &value).await.is_err());
    discard(&pool, &q.id).await.unwrap();
    let third = reply(&pool, t).await;
    let mut ignored = value.clone();
    ignored["amends"] = json!(q.id);
    let standalone = open(&pool, t, third, 0, "NewTasks", &ignored, Some(&q.id))
        .await
        .unwrap();
    assert_eq!(standalone.amended_from, None);
    assert_eq!(get(&pool, &q.id).await.unwrap().status, "discarded");
    assert!(open(&pool, t, 99999, 0, "NewTasks", &value, None)
        .await
        .is_err());
    sqlx::query("DELETE FROM harness_items WHERE id=?1")
        .bind(third)
        .execute(&pool)
        .await
        .unwrap();
    assert!(apply(&pool, &format!("{third}.0"), &value).await.is_err());
}

#[tokio::test]
async fn nested_tasks_and_excluded_under_rows_obey_parent_rules() {
    let (pool, t, i) = setup().await;
    let value = json!({"name":"Exam prep","subject":"COMP10001","tasks":[{"title":"Parent","subtasks":[{"title":"Child"},{"title":"Skip","included":false}]}]});
    let p = card(&pool, t, i, "NewProject", &value).await;
    apply(&pool, &p.id, &value).await.unwrap();
    assert_eq!(count(&pool, "project_tasks").await, 2);
    let parent: i64 = sqlx::query_scalar("SELECT id FROM project_tasks WHERE parent_id IS NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    let next = reply(&pool, t).await;
    let v = json!({"project":null,"items":[{"task":parent,"included":false,"subtasks":[{"title":"No"}]}]});
    let q = card(&pool, t, next, "NewTasks", &v).await;
    assert!(apply(&pool, &q.id, &v).await.is_err());
    assert_eq!(count(&pool, "project_tasks").await, 2);
    undo(&pool, &p.id).await.unwrap();
    assert_eq!(count(&pool, "projects").await, 0);
    assert_eq!(count(&pool, "project_tasks").await, 0);
}

#[tokio::test]
async fn changes_compare_from_and_undo_refuses_later_edits() {
    let (pool, t, i) = setup().await;
    let id = projects::create_tasks(
        &pool,
        None,
        &[projects::NewTask {
            title: "Original".into(),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .unwrap()[0];
    let v = json!({"changes":[{"id":id,"from":{"title":"Wrong"},"to":{"title":"New"}}]});
    let p = card(&pool, t, i, "TaskChanges", &v).await;
    assert!(apply(&pool, &p.id, &v)
        .await
        .unwrap_err()
        .contains("changed since"));
    let v = json!({"changes":[{"id":id,"from":{"title":"Original","due":null},"to":{"title":"New","due":"2026-10-27"}}]});
    apply(&pool, &p.id, &v).await.unwrap();
    projects::update_task(
        &pool,
        id,
        &projects::TaskPatch {
            title: Some("Student edit".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(undo(&pool, &p.id)
        .await
        .unwrap_err()
        .contains("changed since"));
    assert_eq!(get(&pool, &p.id).await.unwrap().status, "created");
}

#[tokio::test]
async fn deleted_parent_restores_subtasks_tag_links_and_file_links() {
    let (pool, t, i) = setup().await;
    sqlx::query("INSERT INTO files (id,subject_id,filename,relative_path,file_type) VALUES (11,1,'notes.pdf','courses/COMP10001/notes.pdf','pdf')").execute(&pool).await.unwrap();
    let ids = projects::create_tasks(
        &pool,
        None,
        &[
            projects::NewTask {
                title: "Parent".into(),
                key: Some("p".into()),
                tags: Some(vec!["Study".into()]),
                files: Some(vec!["courses/COMP10001/notes.pdf".into()]),
                ..Default::default()
            },
            projects::NewTask {
                title: "Child".into(),
                parent: Some(projects::ParentRef::Key("p".into())),
                tags: Some(vec!["Exam".into()]),
                ..Default::default()
            },
        ],
        "manual",
    )
    .await
    .unwrap();
    let v = json!({"tasks":[{"id":ids[0],"included":true}]});
    let p = card(&pool, t, i, "DeleteTasks", &v).await;
    apply(&pool, &p.id, &v).await.unwrap();
    assert_eq!(count(&pool, "project_tasks").await, 0);
    undo(&pool, &p.id).await.unwrap();
    assert_eq!(count(&pool, "project_tasks").await, 2);
    assert_eq!(count(&pool, "task_tag_links").await, 2);
    assert_eq!(count(&pool, "task_files").await, 1);
}

#[tokio::test]
async fn deleted_tags_restore_direct_links_and_project_tag_names() {
    let (pool, t, i) = setup().await;
    let tag = projects::create_tag(&pool, "Custom", Some("green"))
        .await
        .unwrap();
    let project = projects::create_project(
        &pool,
        &projects::NewProject {
            name: "P".into(),
            tags: vec!["Custom".into()],
            source: "manual".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    projects::create_tasks(
        &pool,
        Some(project),
        &[projects::NewTask {
            title: "T".into(),
            tags: Some(vec!["Custom".into()]),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .unwrap();
    let v = json!({"changes":[{"tag":tag.id}]});
    let p = card(&pool, t, i, "TagChanges", &v).await;
    apply(&pool, &p.id, &v).await.unwrap();
    assert_eq!(count(&pool, "task_tag_links").await, 0);
    undo(&pool, &p.id).await.unwrap();
    assert_eq!(count(&pool, "task_tag_links").await, 1);
    assert_eq!(
        projects::project(&pool, project)
            .await
            .unwrap()
            .unwrap()
            .tags,
        ["Custom"]
    );
}

#[tokio::test]
async fn refile_carries_children_and_undo_restores_both_boards() {
    let (pool, t, i) = setup().await;
    let p = projects::create_project(
        &pool,
        &projects::NewProject {
            name: "P".into(),
            source: "manual".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let ids = projects::create_tasks(
        &pool,
        None,
        &[
            projects::NewTask {
                title: "P".into(),
                key: Some("p".into()),
                ..Default::default()
            },
            projects::NewTask {
                title: "C".into(),
                parent: Some(projects::ParentRef::Key("p".into())),
                ..Default::default()
            },
        ],
        "manual",
    )
    .await
    .unwrap();
    let v = json!({"changes":[{"id":ids[0],"from":{"project":null},"to":{"project":p}}]});
    let card = card(&pool, t, i, "TaskChanges", &v).await;
    apply(&pool, &card.id, &v).await.unwrap();
    assert_eq!(
        projects::task(&pool, ids[1])
            .await
            .unwrap()
            .unwrap()
            .project_id,
        Some(p)
    );
    undo(&pool, &card.id).await.unwrap();
    assert_eq!(
        projects::task(&pool, ids[1])
            .await
            .unwrap()
            .unwrap()
            .project_id,
        None
    );
}

#[tokio::test]
async fn events_are_local_and_dates_kinds_and_unknown_fields_are_validated() {
    let (pool, t, i) = setup().await;
    let v = json!({"events":[{"title":"Study","start":"2026-10-27T10:00:00+11:00","end":"2026-10-27T11:00:00+11:00","kind":"class","subject":"COMP10001"}]});
    let p = card(&pool, t, i, "NewEvents", &v).await;
    let done = apply(&pool, &p.id, &v).await.unwrap();
    assert!(done.made[0].id.starts_with("local_"));
    assert_eq!(count(&pool, "local_events").await, 1);
    undo(&pool, &p.id).await.unwrap();
    assert_eq!(count(&pool, "local_events").await, 0);
    let next = reply(&pool, t).await;
    let v = json!({"changes":[{"id":"assignment_1","from":{"title":"a"},"to":{"title":"b"}}]});
    let p = card(&pool, t, next, "EventChanges", &v).await;
    assert!(apply(&pool, &p.id, &v)
        .await
        .unwrap_err()
        .contains("local_"));
    for bad in [
        json!({"events":[{"title":"x","start":"Friday"}]}),
        json!({"events":[{"title":"x","start":"2026-10-27","end":"2026-10-28","kind":"note"}]}),
        json!({"events":[{"title":"x","start":"2026-10-27","unknown":1}]}),
    ] {
        let next = reply(&pool, t).await;
        let p = card(&pool, t, next, "NewEvents", &bad).await;
        assert!(apply(&pool, &p.id, &bad).await.is_err());
    }
}

#[tokio::test]
async fn notes_preserve_existing_files_and_guard_live_bytes_on_undo() {
    let (pool, t, i) = setup().await;
    let root = Scratch::new("proposal-note");
    let v = json!({"subject":"COMP10001","title":"Week 6","body":"# Notes\n\n$x^2$\n"});
    let original = crate::library::files::documents::create_document_body_in(
        &root,
        "COMP10001_2026_SM2",
        "Week 6",
        b"existing",
    )
    .unwrap();
    let p = card(&pool, t, i, "NewNote", &v).await;
    let done = apply_in(&pool, &root, &p.id, &v).await.unwrap();
    let path = &done.made[0].id;
    assert_ne!(path, &original.relative_path);
    assert_eq!(
        std::fs::read_to_string(root.join(path)).unwrap(),
        v["body"].as_str().unwrap()
    );
    assert_eq!(count(&pool, "files").await, 1);
    std::fs::write(root.join(path), "Student edit").unwrap();
    assert!(undo_in(&pool, &root, &p.id).await.is_err());
    std::fs::write(root.join(path), v["body"].as_str().unwrap()).unwrap();
    undo_in(&pool, &root, &p.id).await.unwrap();
    assert!(!root.join(path).exists());
    assert_eq!(
        std::fs::read(root.join(original.relative_path)).unwrap(),
        b"existing"
    );
    assert_eq!(count(&pool, "files").await, 0);
}

#[tokio::test]
async fn opening_a_created_note_does_not_disable_undo() {
    let (pool, thread, item) = setup().await;
    let root = Scratch::new("proposal-note-access");
    let state = json!({"subject":"COMP10001","title":"Open then undo","body":"Unchanged notes"});
    let proposal = card(&pool, thread, item, "NewNote", &state).await;
    let created = apply_in(&pool, &root, &proposal.id, &state).await.unwrap();
    let path = &created.made[0].id;
    sqlx::query("UPDATE files SET last_accessed_at='2026-10-24T14:00:00Z' WHERE relative_path=?1")
        .bind(path)
        .execute(&pool)
        .await
        .unwrap();
    undo_in(&pool, &root, &proposal.id).await.unwrap();
    assert!(!root.join(path).exists());
    assert_eq!(count(&pool, "files").await, 0);
}

#[tokio::test]
async fn implicit_tag_and_new_child_usage_block_destructive_undo() {
    let (pool, t, i) = setup().await;
    let v = json!({"project":null,"items":[{"title":"Generated","tags":["New tag"]}]});
    let p = card(&pool, t, i, "NewTasks", &v).await;
    apply(&pool, &p.id, &v).await.unwrap();
    projects::create_tasks(
        &pool,
        None,
        &[projects::NewTask {
            title: "Student".into(),
            tags: Some(vec!["New tag".into()]),
            ..Default::default()
        }],
        "manual",
    )
    .await
    .unwrap();
    assert!(undo(&pool, &p.id)
        .await
        .unwrap_err()
        .contains("changed since"));
    assert_eq!(count(&pool, "project_tasks").await, 2);
}

#[tokio::test]
async fn inspection_reports_parent_children_and_effective_tags() {
    let (pool, t, i) = setup().await;
    let v =
        json!({"project":null,"items":[{"title":"P","tags":["Study"],"subtasks":[{"title":"C"}]}]});
    let p = card(&pool, t, i, "NewTasks", &v).await;
    apply(&pool, &p.id, &v).await.unwrap();
    let id: i64 = sqlx::query_scalar("SELECT id FROM project_tasks WHERE parent_id IS NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    let data = inspect(&pool, "task", &[json!(id)]).await.unwrap();
    assert_eq!(data[&id.to_string()]["children"][0]["title"], "C");
    assert_eq!(data[&id.to_string()]["tags"], json!(["Study"]));
    let tag: i64 = sqlx::query("SELECT id FROM task_tags WHERE name='Study'")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("id");
    let data = inspect(&pool, "tag", &[json!(tag)]).await.unwrap();
    assert_eq!(data[&tag.to_string()]["task_count"], 1);
}

#[tokio::test]
async fn concurrent_connections_apply_exactly_once_with_foreign_keys_enabled() {
    let root = Scratch::new("proposal-concurrent");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(root.join("test.db"))
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5));
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(3)
        .connect_with(options)
        .await
        .unwrap();
    let mut migrating = pool.acquire().await.unwrap();
    for migration in crate::db::migrations::all() {
        sqlx::raw_sql(migration.sql)
            .execute(&mut *migrating)
            .await
            .unwrap();
    }
    drop(migrating);
    let t = store::create_thread(&pool, Provider::Codex, None, None, None, "hello")
        .await
        .unwrap();
    let i = reply(&pool, t).await;
    let v = json!({"project":null,"items":[{"title":"Exactly once"}]});
    let p = card(&pool, t, i, "NewTasks", &v).await;
    let (a, b) = tokio::join!(apply(&pool, &p.id, &v), apply(&pool, &p.id, &v));
    assert_ne!(a.is_ok(), b.is_ok(), "{a:?} {b:?}");
    assert_eq!(count(&pool, "project_tasks").await, 1);
    assert_eq!(count(&pool, "agent_changes").await, 1);
    undo(&pool, &p.id).await.unwrap();
    assert_eq!(count(&pool, "project_tasks").await, 0);
}

#[tokio::test]
async fn a_failed_note_database_insert_cleans_only_the_newly_reserved_file() {
    let (pool, t, i) = setup().await;
    let root = Scratch::new("proposal-note-rollback");
    let original = crate::library::files::documents::create_document_body_in(
        &root,
        "COMP10001_2026_SM2",
        "Week 6",
        b"existing",
    )
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_proposal_note BEFORE INSERT ON files BEGIN SELECT RAISE(FAIL,'test insert failure'); END;").execute(&pool).await.unwrap();
    let v = json!({"subject":"COMP10001","title":"Week 6","body":"new"});
    let p = card(&pool, t, i, "NewNote", &v).await;
    assert!(apply_in(&pool, &root, &p.id, &v).await.is_err());
    assert_eq!(
        std::fs::read(root.join(&original.relative_path)).unwrap(),
        b"existing"
    );
    let parent = root
        .join(&original.relative_path)
        .parent()
        .unwrap()
        .to_path_buf();
    assert_eq!(std::fs::read_dir(parent).unwrap().count(), 1);
    assert_eq!(get(&pool, &p.id).await.unwrap().status, "pending");
}

#[tokio::test]
async fn amendments_keep_explicit_ticks_by_identity_and_occurrence_without_copying_fields() {
    let (pool, thread, item) = setup().await;
    let old = json!({"project":null,"items":[
        {"title":"Repeated","included":false,"due":"2026-10-27"},
        {"title":"Repeated","included":true,"due":"2026-10-27"},
        {"title":"Parent","body":"Student's previous text","subtasks":[{"title":"Child","included":false}]},
        {"task":41,"included":false,"subtasks":[{"title":"Existing parent's child","included":false}]}
    ]});
    let first = card(&pool, thread, item, "NewTasks", &old).await;
    let next_item = reply(&pool, thread).await;
    let next = json!({"project":null,"amends":first.id,"items":[
        {"title":"Repeated","due":"2026-10-30"},
        {"title":"Repeated","due":"2026-10-31"},
        {"title":"Parent","body":"Agent's replacement text","subtasks":[{"title":"Child","due":"2026-10-31"}]},
        {"task":41,"subtasks":[{"title":"Existing parent's child","due":"2026-10-31"}]},
        {"title":"New item","due":"2026-10-31"}
    ]});
    let amended = open(
        &pool,
        thread,
        next_item,
        0,
        "NewTasks",
        &next,
        Some(&first.id),
    )
    .await
    .unwrap();
    assert_eq!(amended.state["items"][0]["included"], false);
    assert_eq!(amended.state["items"][1]["included"], true);
    assert_eq!(amended.state["items"][0]["due"], "2026-10-30");
    assert_eq!(
        amended.state["items"][2]["body"],
        "Agent's replacement text"
    );
    assert_eq!(amended.state["items"][2]["subtasks"][0]["included"], false);
    assert_eq!(amended.state["items"][3]["included"], false);
    assert_eq!(amended.state["items"][3]["subtasks"][0]["included"], false);
    assert!(amended.state["items"][4].get("included").is_none());
}

#[tokio::test]
async fn amendments_preserve_existing_row_unticks_but_ignored_decided_amends_do_not_inherit() {
    let (pool, thread, item) = setup().await;
    let old = json!({"changes":[{"id":7,"from":{"due":"2026-10-27"},"to":{"due":"2026-10-28"},"included":false}]});
    let first = card(&pool, thread, item, "TaskChanges", &old).await;
    let next_item = reply(&pool, thread).await;
    let next = json!({"amends":first.id,"changes":[{"id":7,"from":{"due":"2026-10-27"},"to":{"due":"2026-10-30"}}]});
    let amended = open(
        &pool,
        thread,
        next_item,
        0,
        "TaskChanges",
        &next,
        Some(&first.id),
    )
    .await
    .unwrap();
    assert_eq!(amended.state["changes"][0]["included"], false);
    assert_eq!(amended.state["changes"][0]["to"]["due"], "2026-10-30");
    discard(&pool, &amended.id).await.unwrap();
    let last_item = reply(&pool, thread).await;
    let mut ignored = next.clone();
    ignored["amends"] = json!(amended.id);
    let standalone = open(
        &pool,
        thread,
        last_item,
        0,
        "TaskChanges",
        &ignored,
        Some(&amended.id),
    )
    .await
    .unwrap();
    assert!(standalone.state["changes"][0].get("included").is_none());
    assert_eq!(standalone.amended_from, None);
}

#[tokio::test]
async fn cheat_undo_removes_edited_entries_and_reopens_the_card() {
    let (pool, t, i) = setup().await;
    let input = json!({"notes":[{"title":"Chain rule","body":"Differentiate the outside, multiply by the inside derivative.","subject":"COMP10001","tags":["calculus"]}]});
    let p = card(&pool, t, i, "NewCheatNotes", &input).await;
    let applied = apply(&pool, &p.id, &input).await.unwrap();
    assert_eq!(applied.made.len(), 1);
    assert_eq!(applied.made[0].entity, "cheat");
    assert!(apply(&pool, &p.id, &input).await.is_err());
    let id: i64 = applied.made[0].id.parse().unwrap();
    sqlx::query("UPDATE revision_notes SET body='Student edit' WHERE id=?1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let undone = undo(&pool, &p.id).await.unwrap();
    assert_eq!(undone.status, "pending");
    assert!(undone.made.is_empty());
    assert_eq!(undone.state, input);
    assert_eq!(count(&pool, "revision_notes").await, 0);
    let again = apply(&pool, &p.id, &input).await.unwrap();
    assert_eq!(again.made.len(), 1);
    assert_eq!(count(&pool, "revision_notes").await, 1);
}
#[tokio::test]
async fn cheat_undo_skips_entries_the_student_deleted() {
    let (pool, t, i) = setup().await;
    let input = json!({"notes":[{"title":"One","body":"a","subject":"COMP10001"},{"title":"Two","body":"b","subject":"COMP10001"}]});
    let p = card(&pool, t, i, "NewCheatNotes", &input).await;
    let applied = apply(&pool, &p.id, &input).await.unwrap();
    sqlx::query("DELETE FROM revision_notes WHERE id=?1")
        .bind(applied.made[0].id.parse::<i64>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    undo(&pool, &p.id).await.unwrap();
    assert_eq!(count(&pool, "revision_notes").await, 0);
}
#[tokio::test]
async fn cheat_tags_take_the_registry_spelling() {
    let (pool, t, i) = setup().await;
    let input = json!({"notes":[{"title":"Sum","body":"$a+b$","subject":"COMP10001","tags":["formula","FORMULA","Grover"]}]});
    let p = card(&pool, t, i, "NewCheatNotes", &input).await;
    apply(&pool, &p.id, &input).await.unwrap();
    let tags: String = sqlx::query_scalar("SELECT tags FROM revision_notes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tags, r#"["Formula","Grover"]"#);
    let path = json!({"notes":[{"title":"Bad","body":"x","subject":"COMP10001","tags":["courses/A/files/x.pdf"]}]});
    let q = card(&pool, t, reply(&pool, t).await, "NewCheatNotes", &path).await;
    assert!(apply(&pool, &q.id, &path).await.is_err());
}
#[tokio::test]
async fn cheat_proposal_undo_removes_only_unchanged_entries() {
    let (pool, t, i) = setup().await;
    let input = json!({"notes":[{"title":"Formula","body":"$a+b$"}]});
    let p = card(&pool, t, i, "NewCheatNotes", &input).await;
    apply(&pool, &p.id, &input).await.unwrap();
    undo(&pool, &p.id).await.unwrap();
    assert_eq!(count(&pool, "revision_notes").await, 0);
}
#[tokio::test]
async fn cheat_cards_create_named_topics_and_undo_removes_them() {
    let (pool, t, i) = setup().await;
    let input = json!({"notes":[
        {"title":"The angle","body":"$\\sin\\theta=\\sqrt{M/N}$","subject":"COMP10001","topics":["Grover's algorithm"]},
        {"title":"Iterations","body":"$K\\approx\\frac{\\pi}{4}\\sqrt{N/M}$","subject":"COMP10001","topics":["grover's algorithm"]}
    ]});
    let p = card(&pool, t, i, "NewCheatNotes", &input).await;
    apply(&pool, &p.id, &input).await.unwrap();
    let topics: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM revision_topics WHERE name='Grover''s algorithm'")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(topics.len(), 1);
    let filed: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM revision_note_topics WHERE topic_id=?1")
            .bind(topics[0])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(filed, 2);
    undo(&pool, &p.id).await.unwrap();
    assert_eq!(count(&pool, "revision_notes").await, 0);
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM revision_topics WHERE is_unsorted=0")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
}
