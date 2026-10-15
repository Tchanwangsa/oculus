//! Migration 47 and the writers over it: the tag registry, tag links and the
//! files attached to a task. Runs every migration, so the foreign keys are the
//! real ones.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use super::*;

pub(super) async fn migrated_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for migration in crate::db::migrations::all() {
        sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
    }
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
            event_id: None,
            source: "manual".to_string(),
        },
    )
    .await
    .unwrap()
}

async fn seed_subject(pool: &SqlitePool, id: i64, code: &str) {
    sqlx::query("INSERT INTO subjects (id, code, name) VALUES (?1, ?2, ?2)")
        .bind(id)
        .bind(code)
        .execute(pool)
        .await
        .unwrap();
}

async fn seed_file(pool: &SqlitePool, subject_id: i64, path: &str) -> i64 {
    sqlx::query(
        "INSERT INTO files (subject_id, filename, relative_path, file_type)
         VALUES (?1, 'f.pdf', ?2, 'pdf')",
    )
    .bind(subject_id)
    .bind(path)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

fn item(title: &str) -> NewTask {
    NewTask {
        title: title.to_string(),
        ..NewTask::default()
    }
}

async fn add(pool: &SqlitePool, project: Option<i64>, task: NewTask) -> i64 {
    create_tasks(pool, project, &[task], "agent").await.unwrap()[0]
}

async fn tag_names(pool: &SqlitePool, id: i64) -> Vec<String> {
    let tags = list_tags(pool).await.unwrap();
    task(pool, id)
        .await
        .unwrap()
        .unwrap()
        .tag_ids
        .iter()
        .map(|t| tags.iter().find(|g| g.id == *t).unwrap().name.clone())
        .collect()
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Backdate so a bump in the same second is still visible.
async fn age(pool: &SqlitePool, table: &str, id: i64) {
    sqlx::query(&format!(
        "UPDATE {table} SET updated_at = '2000-01-01 00:00:00' WHERE id = ?1"
    ))
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
}

async fn updated(pool: &SqlitePool, table: &str, id: i64) -> String {
    sqlx::query_scalar(&format!("SELECT updated_at FROM {table} WHERE id = ?1"))
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn the_beginner_tags_are_seeded_in_order() {
    let pool = migrated_pool().await;
    let tags = list_tags(&pool).await.unwrap();
    let seeded: Vec<(&str, &str)> = tags
        .iter()
        .map(|t| (t.name.as_str(), t.color.as_str()))
        .collect();
    assert_eq!(
        seeded,
        [
            ("Assignment", "indigo"),
            ("Exam", "rose"),
            ("Reading", "green"),
            ("Study", "amber"),
            ("Admin", "slate"),
        ]
    );
    assert!(tags.iter().all(|t| t.uses == 0));
}

#[tokio::test]
async fn creating_a_tag_is_idempotent_ignoring_case() {
    let pool = migrated_pool().await;
    let first = create_tag(&pool, "  Lab   report ", None).await.unwrap();
    assert_eq!(first.name, "Lab report");
    assert_eq!(first.color, "slate");
    assert_eq!(first.position, 6.0);

    let again = create_tag(&pool, "LAB REPORT", Some("rose")).await.unwrap();
    assert_eq!(again.id, first.id);
    assert_eq!(again.color, "slate", "an existing tag is not recoloured");
    assert_eq!(create_tag(&pool, "exam", None).await.unwrap().name, "Exam");
    assert_eq!(list_tags(&pool).await.unwrap().len(), 6);
}

#[tokio::test]
async fn tag_names_and_colours_are_checked() {
    let pool = migrated_pool().await;
    assert!(create_tag(&pool, "   ", None).await.is_err());
    assert!(create_tag(&pool, &"x".repeat(41), None).await.is_err());
    assert!(create_tag(&pool, &"x".repeat(40), None).await.is_ok());

    let err = create_tag(&pool, "Quiz", Some("#ff0000"))
        .await
        .unwrap_err();
    assert!(err.contains("slate, indigo, green, amber, rose, purple, teal, orange"));
    assert!(
        find_tag(&pool, "Quiz").await.unwrap().is_none(),
        "a refused colour makes no tag"
    );
    assert_eq!(check_tag_color(" Teal ").unwrap(), "teal");
}

#[tokio::test]
async fn renaming_refuses_a_collision_but_allows_a_recase() {
    let pool = migrated_pool().await;
    let exam = find_tag(&pool, "exam").await.unwrap().unwrap();

    let err = update_tag(&pool, exam.id, Some("reading"), None)
        .await
        .unwrap_err();
    assert!(err.contains("Reading"), "{err}");
    assert_eq!(find_tag(&pool, "Exam").await.unwrap().unwrap().id, exam.id);

    let renamed = update_tag(&pool, exam.id, Some("EXAM"), Some("purple"))
        .await
        .unwrap();
    assert_eq!(
        (renamed.name.as_str(), renamed.color.as_str()),
        ("EXAM", "purple")
    );
    assert!(update_tag(&pool, exam.id, None, Some("pink"))
        .await
        .is_err());
    assert!(update_tag(&pool, 999, Some("x"), None).await.is_err());
}

#[tokio::test]
async fn set_task_tags_replaces_atomically_and_bumps_updated_at() {
    let pool = migrated_pool().await;
    let project = seed_project(&pool, "Essay").await;
    let id = add(&pool, Some(project), item("Draft")).await;
    let tags = list_tags(&pool).await.unwrap();
    let (assignment, exam, reading) = (tags[0].id, tags[1].id, tags[2].id);

    age(&pool, "project_tasks", id).await;
    age(&pool, "projects", project).await;
    set_task_tags(&pool, id, &[reading, assignment, assignment])
        .await
        .unwrap();
    assert_eq!(tag_names(&pool, id).await, ["Assignment", "Reading"]);
    assert_ne!(
        updated(&pool, "project_tasks", id).await,
        "2000-01-01 00:00:00"
    );
    assert_ne!(
        updated(&pool, "projects", project).await,
        "2000-01-01 00:00:00"
    );

    set_task_tags(&pool, id, &[exam]).await.unwrap();
    assert_eq!(tag_names(&pool, id).await, ["Exam"]);

    let err = set_task_tags(&pool, id, &[assignment, 77, 78])
        .await
        .unwrap_err();
    assert!(err.contains("77, 78"), "{err}");
    assert_eq!(
        tag_names(&pool, id).await,
        ["Exam"],
        "a refused call leaves the old set"
    );

    set_task_tags(&pool, id, &[]).await.unwrap();
    assert!(tag_names(&pool, id).await.is_empty());
    assert!(set_task_tags(&pool, 999, &[exam]).await.is_err());
}

#[tokio::test]
async fn tag_links_cascade_from_the_tag_and_from_the_task() {
    let pool = migrated_pool().await;
    let a = add(&pool, None, item("A")).await;
    let b = add(&pool, None, item("B")).await;
    let exam = find_tag(&pool, "Exam").await.unwrap().unwrap().id;
    set_task_tags(&pool, a, &[exam]).await.unwrap();
    set_task_tags(&pool, b, &[exam]).await.unwrap();
    assert_eq!(find_tag(&pool, "Exam").await.unwrap().unwrap().uses, 2);

    age(&pool, "project_tasks", a).await;
    assert_eq!(delete_tag(&pool, exam).await.unwrap(), 2);
    assert!(task(&pool, a).await.unwrap().unwrap().tag_ids.is_empty());
    assert_ne!(
        updated(&pool, "project_tasks", a).await,
        "2000-01-01 00:00:00"
    );
    assert_eq!(count(&pool, "task_tag_links").await, 0);
    assert!(delete_tag(&pool, exam).await.is_err());

    let reading = find_tag(&pool, "Reading").await.unwrap().unwrap().id;
    set_task_tags(&pool, a, &[reading]).await.unwrap();
    delete_task(&pool, a).await.unwrap();
    assert_eq!(count(&pool, "task_tag_links").await, 0);
    assert!(find_tag(&pool, "Reading").await.unwrap().is_some());
}

#[tokio::test]
async fn files_attach_and_detach_by_path() {
    let pool = migrated_pool().await;
    seed_subject(&pool, 1, "COMP1").await;
    seed_subject(&pool, 2, "COMP2").await;
    let brief = seed_file(&pool, 1, "courses/COMP1/assignments/brief.pdf").await;
    seed_file(&pool, 1, "courses/COMP1/lectures/week1.pdf").await;
    seed_file(&pool, 1, "shared/slides.pdf").await;
    seed_file(&pool, 2, "shared/slides.pdf").await;
    let project = seed_project(&pool, "Essay").await;
    let id = add(&pool, Some(project), item("Read")).await;

    age(&pool, "project_tasks", id).await;
    update_task(
        &pool,
        id,
        &TaskPatch {
            attach: vec![
                "courses/COMP1/assignments/brief.pdf".into(),
                "courses/COMP1/lectures/week1.pdf".into(),
                "courses/COMP1/assignments/brief.pdf".into(),
            ],
            ..TaskPatch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(task(&pool, id).await.unwrap().unwrap().file_count, 2);
    let attached = task_files(&pool, id).await.unwrap();
    assert_eq!(attached.len(), 2);
    let attached_brief = attached.iter().find(|file| file.file_id == brief).unwrap();
    assert_eq!(
        attached_brief.relative_path,
        "courses/COMP1/assignments/brief.pdf"
    );
    assert_eq!(attached_brief.subject_code.as_deref(), Some("COMP1"));
    assert_ne!(
        updated(&pool, "project_tasks", id).await,
        "2000-01-01 00:00:00"
    );

    age(&pool, "project_tasks", id).await;
    let again = TaskPatch {
        attach: vec!["courses/COMP1/assignments/brief.pdf".into()],
        ..TaskPatch::default()
    };
    update_task(&pool, id, &again).await.unwrap();
    assert_eq!(task(&pool, id).await.unwrap().unwrap().file_count, 2);
    assert_eq!(
        updated(&pool, "project_tasks", id).await,
        "2000-01-01 00:00:00",
        "attaching what is attached changes nothing"
    );

    let ambiguous = update_task(
        &pool,
        id,
        &TaskPatch {
            attach: vec!["shared/slides.pdf".into()],
            ..TaskPatch::default()
        },
    )
    .await
    .unwrap_err();
    assert!(ambiguous.contains("COMP1, COMP2"), "{ambiguous}");

    let missing = update_task(
        &pool,
        id,
        &TaskPatch {
            attach: vec![
                "courses/COMP1/assignments/brief.pdf".into(),
                "nope.pdf".into(),
            ],
            detach: vec!["courses/COMP1/lectures/week1.pdf".into()],
            ..TaskPatch::default()
        },
    )
    .await
    .unwrap_err();
    assert!(missing.contains("nope.pdf"), "{missing}");
    assert_eq!(
        task(&pool, id).await.unwrap().unwrap().file_count,
        2,
        "a refused patch writes nothing"
    );

    update_task(
        &pool,
        id,
        &TaskPatch {
            detach: vec!["courses/COMP1/lectures/week1.pdf".into()],
            ..TaskPatch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(task(&pool, id).await.unwrap().unwrap().file_count, 1);

    sqlx::query("DELETE FROM files WHERE id = ?1")
        .bind(brief)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        count(&pool, "task_files").await,
        0,
        "the file's links go with it"
    );
}

#[tokio::test]
async fn update_replaces_tags_by_name_and_makes_unknown_ones() {
    let pool = migrated_pool().await;
    let project = seed_project(&pool, "Essay").await;
    let id = add(&pool, Some(project), item("Draft")).await;

    update_task(
        &pool,
        id,
        &TaskPatch {
            tags: Some(vec!["exam".into(), "Group Work".into(), "EXAM".into()]),
            ..TaskPatch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(tag_names(&pool, id).await, ["Exam", "Group Work"]);
    let made = find_tag(&pool, "group work").await.unwrap().unwrap();
    assert_eq!(made.color, "slate");

    update_task(
        &pool,
        id,
        &TaskPatch {
            tags: Some(vec![]),
            ..TaskPatch::default()
        },
    )
    .await
    .unwrap();
    assert!(tag_names(&pool, id).await.is_empty());
    assert!(
        find_tag(&pool, "Group Work").await.unwrap().is_some(),
        "clearing a task's tags leaves the registry alone"
    );
}

#[tokio::test]
async fn a_batch_with_tags_and_files_is_one_transaction() {
    let pool = migrated_pool().await;
    seed_subject(&pool, 1, "COMP1").await;
    seed_file(&pool, 1, "courses/COMP1/brief.pdf").await;
    let project = seed_project(&pool, "Essay").await;

    let good = NewTask {
        tags: Some(vec!["Assignment".into(), "Fresh".into()]),
        files: Some(vec!["courses/COMP1/brief.pdf".into()]),
        key: Some("a".into()),
        ..item("Outline")
    };
    let child = NewTask {
        parent: Some(ParentRef::Key("a".into())),
        tags: Some(vec!["Study".into()]),
        ..item("Sub")
    };
    let ids = create_tasks(&pool, Some(project), &[good.clone(), child], "agent")
        .await
        .unwrap();
    let parent = task(&pool, ids[0]).await.unwrap().unwrap();
    assert_eq!(parent.file_count, 1);
    assert_eq!(tag_names(&pool, ids[0]).await, ["Assignment", "Fresh"]);
    assert_eq!(tag_names(&pool, ids[1]).await, ["Study"]);

    let before = (
        count(&pool, "project_tasks").await,
        count(&pool, "task_tags").await,
        count(&pool, "task_tag_links").await,
        count(&pool, "task_files").await,
    );
    let bad = NewTask {
        tags: Some(vec!["Brand New".into()]),
        files: Some(vec!["courses/COMP1/missing.pdf".into()]),
        ..item("Doomed")
    };
    let err = create_tasks(&pool, Some(project), &[good, bad], "agent")
        .await
        .unwrap_err();
    assert!(
        err.contains("Doomed") && err.contains("missing.pdf"),
        "{err}"
    );
    let after = (
        count(&pool, "project_tasks").await,
        count(&pool, "task_tags").await,
        count(&pool, "task_tag_links").await,
        count(&pool, "task_files").await,
    );
    assert_eq!(
        before, after,
        "no task, tag or link survives a refused batch"
    );
}

#[tokio::test]
async fn refile_and_move_leave_the_links_alone() {
    let pool = migrated_pool().await;
    seed_subject(&pool, 1, "COMP1").await;
    seed_file(&pool, 1, "courses/COMP1/brief.pdf").await;
    let from = seed_project(&pool, "From").await;
    let to = seed_project(&pool, "To").await;
    let parent = NewTask {
        tags: Some(vec!["Exam".into()]),
        files: Some(vec!["courses/COMP1/brief.pdf".into()]),
        key: Some("p".into()),
        ..item("Parent")
    };
    let child = NewTask {
        parent: Some(ParentRef::Key("p".into())),
        tags: Some(vec!["Reading".into()]),
        ..item("Child")
    };
    let ids = create_tasks(&pool, Some(from), &[parent, child], "agent")
        .await
        .unwrap();

    move_task(&pool, ids[0], "done", None, None).await.unwrap();
    assert_eq!(refile_task(&pool, ids[0], Some(to)).await.unwrap(), 2);
    assert_eq!(refile_task(&pool, ids[0], None).await.unwrap(), 2);

    for (id, tag, files) in [(ids[0], "Exam", 1), (ids[1], "Reading", 0)] {
        assert_eq!(tag_names(&pool, id).await, [tag]);
        assert_eq!(task(&pool, id).await.unwrap().unwrap().file_count, files);
    }
    assert_eq!(count(&pool, "task_tag_links").await, 2);
    assert_eq!(count(&pool, "task_files").await, 1);
}

#[tokio::test]
async fn reads_carry_tag_ids_in_registry_order() {
    let pool = migrated_pool().await;
    let project = seed_project(&pool, "Essay").await;
    let id = add(
        &pool,
        Some(project),
        NewTask {
            tags: Some(vec!["Study".into(), "Assignment".into()]),
            ..item("Mixed")
        },
    )
    .await;
    let tags = list_tags(&pool).await.unwrap();
    let want = vec![tags[0].id, tags[3].id];

    assert_eq!(task(&pool, id).await.unwrap().unwrap().tag_ids, want);
    assert_eq!(tasks(&pool, project).await.unwrap()[0].tag_ids, want);
    let all = all_tasks(&pool, TaskScope::All).await.unwrap();
    assert_eq!(all[0].tag_ids, want);
    let json = serde_json::to_value(&all[0]).unwrap();
    assert_eq!(json["tag_ids"], serde_json::json!(want));
}

#[test]
fn batch_items_accept_tags_and_files() {
    let items: Vec<NewTask> = serde_json::from_str(
        r#"[{"title":"Read","tags":["Reading","Week 5"],"files":["courses/X/a.pdf"]},
            {"title":"Plain"}]"#,
    )
    .unwrap();
    assert_eq!(items[0].tags.as_deref().unwrap().len(), 2);
    assert_eq!(items[0].files.as_deref().unwrap(), ["courses/X/a.pdf"]);
    assert!(items[1].tags.is_none() && items[1].files.is_none());
    assert!(serde_json::from_str::<Vec<NewTask>>(r#"[{"title":"x","tag":["a"]}]"#).is_err());
}

#[tokio::test]
async fn untagged_tasks_follow_shared_project_tags_and_explicit_tags_override() {
    let pool = migrated_pool().await;
    let project = seed_project(&pool, "Tagged project").await;
    update_project(
        &pool,
        project,
        &ProjectPatch {
            tags: Some(vec!["assignment".into(), "Project only".into()]),
            ..ProjectPatch::default()
        },
    )
    .await
    .unwrap();
    let inherited = add(&pool, Some(project), item("Inherited")).await;
    let child = add(
        &pool,
        Some(project),
        NewTask {
            parent: Some(ParentRef::Id(inherited)),
            ..item("Child")
        },
    )
    .await;
    let explicit = add(
        &pool,
        Some(project),
        NewTask {
            tags: Some(vec!["Reading".into()]),
            ..item("Explicit")
        },
    )
    .await;
    let unfiled = add(&pool, None, item("Unfiled")).await;
    assert_eq!(
        tag_names(&pool, inherited).await,
        vec!["Assignment", "Project only"]
    );
    assert_eq!(
        tag_names(&pool, child).await,
        vec!["Assignment", "Project only"]
    );
    assert_eq!(tag_names(&pool, explicit).await, vec!["Reading"]);
    assert!(tag_names(&pool, unfiled).await.is_empty());

    let shared = find_tag(&pool, "Project only").await.unwrap().unwrap();
    assert_eq!(shared.uses, 2);
    update_tag(&pool, shared.id, Some("Renamed"), Some("teal"))
        .await
        .unwrap();
    assert_eq!(
        super::project(&pool, project).await.unwrap().unwrap().tags,
        vec!["assignment", "Renamed"]
    );
    assert_eq!(
        tag_names(&pool, inherited).await,
        vec!["Assignment", "Renamed"]
    );
    delete_tag(&pool, shared.id).await.unwrap();
    assert_eq!(tag_names(&pool, inherited).await, vec!["Assignment"]);
    assert_eq!(
        super::project(&pool, project).await.unwrap().unwrap().tags,
        vec!["assignment"]
    );

    update_project(
        &pool,
        project,
        &ProjectPatch {
            tags: Some(vec!["Exam".into()]),
            ..ProjectPatch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(tag_names(&pool, inherited).await, vec!["Exam"]);
    assert_eq!(tag_names(&pool, explicit).await, vec!["Reading"]);
    set_task_tags(&pool, explicit, &[]).await.unwrap();
    assert_eq!(tag_names(&pool, explicit).await, vec!["Exam"]);
    refile_task(&pool, inherited, None).await.unwrap();
    assert!(tag_names(&pool, inherited).await.is_empty());
    pool.close().await;
}
