use super::{inspect, journal::Journal, state};
use crate::db::projects::{self, NewTask, ParentRef};
use serde_json::Value;
use sqlx::SqliteConnection;

pub(super) async fn run(
    conn: &mut SqliteConnection,
    kind: &str,
    value: &Value,
    journal: &mut Journal,
) -> Result<(), String> {
    state::validate_shape(kind, value)?;
    match kind {
        "NewTasks" => {
            let project = state::nullable_id(value.get("project"))?;
            if let Some(id) = project {
                journal.watch(conn, "project", &id.to_string()).await?;
            }
            for task in state::rows(value, "items")? {
                if !state::included(task) {
                    continue;
                }
                if task.get("task").is_some() {
                    state::object(task, &["task", "subtasks"])?;
                    let parent = state::id(&task["task"])?;
                    journal.task(conn, parent, false).await?;
                    let parent_row = inspect::current(conn, "task", &parent.to_string()).await?;
                    if parent_row.is_null() {
                        return Err(format!("task {parent} does not exist"));
                    }
                    let parent_project = state::nullable_id(parent_row.get("project"))?;
                    for child in state::rows(task, "subtasks")? {
                        create_task(conn, parent_project, Some(parent), child, journal).await?;
                    }
                } else {
                    create_task(conn, project, None, task, journal).await?;
                }
            }
        }
        "NewProject" => {
            let name = state::text(value, "name")?;
            let code = state::nullable_text(value, "subject")?;
            let subject = inspect::subject(conn, code.as_deref()).await?;
            let tags = state::strings(value, "tags")?;
            let watching = journal.tag_names(conn, &tags).await?;
            let id = projects::create_project_on(
                conn,
                &projects::NewProject {
                    name,
                    subject_id: subject.map(|s| s.0),
                    brief: state::nullable_text(value, "brief")?,
                    starts_at: state::date(value, "starts")?,
                    due_at: state::date(value, "due")?,
                    tags,
                    event_id: state::nullable_text(value, "event")?,
                    source: "agent".into(),
                },
            )
            .await?;
            journal.created("project", id);
            journal.added_tags(conn, &watching).await?;
            for task in state::rows(value, "tasks")? {
                create_task(conn, Some(id), None, task, journal).await?;
            }
        }
        "TaskChanges" | "ProjectChanges" | "TagChanges" | "EventChanges" => {
            super::changes::run(conn, kind, value, journal).await?
        }
        "DeleteTasks" => {
            for row in state::rows(value, "tasks")? {
                if !state::included(row) {
                    continue;
                }
                let id = delete_id(row)?;
                let id = state::id(id)?;
                journal.task(conn, id, true).await?;
                // Explicit deletes keep links/children correct even on FK-off connections.
                let mut ids = super::journal::children_ids(conn, id).await?;
                ids.push(id);
                for target in ids {
                    for table in ["task_tag_links", "task_files"] {
                        sqlx::query(&format!("DELETE FROM {table} WHERE task_id=?1"))
                            .bind(target)
                            .execute(&mut *conn)
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                }
                sqlx::query("DELETE FROM project_tasks WHERE parent_id=?1")
                    .bind(id)
                    .execute(&mut *conn)
                    .await
                    .map_err(|e| e.to_string())?;
                projects::delete_task_on(conn, id).await?;
            }
        }
        "NewEvents" => {
            for event in state::rows(value, "events")? {
                if state::included(event) {
                    let id = create_event(conn, event).await?;
                    journal.created("event", format!("local_{id}"));
                }
            }
        }
        "DeleteEvents" => {
            for row in state::rows(value, "events")? {
                if !state::included(row) {
                    continue;
                }
                let id = delete_id(row)?
                    .as_str()
                    .ok_or("event id must be a local_ string")?;
                let n = inspect::numeric("event", id)?;
                journal.watch(conn, "event", id).await?;
                if inspect::snapshot(conn, "event", id).await?.is_null() {
                    return Err(format!("event {id} does not exist"));
                }
                sqlx::query("DELETE FROM local_events WHERE id=?1")
                    .bind(n)
                    .execute(&mut *conn)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        "NewNote" => return Err("notes need the document apply path".into()),
        _ => return Err("this is a job card; start its job instead".into()),
    }
    Ok(())
}
fn delete_id(v: &Value) -> Result<&Value, String> {
    if v.is_object() {
        state::object(v, &["id"])?;
        v.get("id").ok_or_else(|| "missing item id".into())
    } else {
        Ok(v)
    }
}

async fn create_task(
    conn: &mut SqliteConnection,
    project: Option<i64>,
    parent: Option<i64>,
    v: &Value,
    journal: &mut Journal,
) -> Result<(), String> {
    if !state::included(v) {
        return Ok(());
    }
    state::object(
        v,
        &[
            "title", "due", "estimate", "column", "subtasks", "tags", "body", "starts", "files",
        ],
    )?;
    let children = match v.get("subtasks") {
        None | Some(Value::Null) => &[][..],
        Some(Value::Array(a)) if a.len() <= 500 => a.as_slice(),
        _ => return Err("subtasks needs a list".into()),
    };
    if parent.is_some() && !children.is_empty() {
        return Err("subtasks are one level deep".into());
    }
    let estimate = match v.get("estimate") {
        None | Some(Value::Null) => None,
        Some(n) => Some(
            n.as_i64()
                .filter(|n| *n >= 0)
                .ok_or("estimate needs whole, nonnegative minutes")?,
        ),
    };
    let tags = state::strings(v, "tags")?;
    let watching = journal.tag_names(conn, &tags).await?;
    if let Some(id) = parent {
        journal.task(conn, id, false).await?;
    }
    let item = NewTask {
        title: state::text(v, "title")?,
        column: state::nullable_text(v, "column")?,
        parent: parent.map(ParentRef::Id),
        body: state::nullable_text(v, "body")?,
        due: state::date(v, "due")?,
        starts: state::date(v, "starts")?,
        estimate,
        tags: Some(tags),
        files: Some(state::strings(v, "files")?),
        key: None,
    };
    let id = projects::create_tasks_on(conn, project, &[item], "agent").await?[0];
    journal.created("task", id);
    journal.added_tags(conn, &watching).await?;
    for child in children {
        if !state::included(child) {
            continue;
        }
        state::object(
            child,
            &[
                "title", "due", "estimate", "column", "subtasks", "tags", "body", "starts", "files",
            ],
        )?;
        if child
            .get("subtasks")
            .is_some_and(|v| !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty()))
        {
            return Err("subtasks are one level deep".into());
        }
        // Recursion is boxed: one call only, with the depth guard above.
        Box::pin(create_task(conn, project, Some(id), child, journal)).await?;
    }
    Ok(())
}

pub(super) async fn event_fields(
    conn: &mut SqliteConnection,
    v: &Value,
) -> Result<
    (
        Option<i64>,
        String,
        String,
        String,
        Option<String>,
        bool,
        Option<String>,
    ),
    String,
> {
    state::object(v, &["title", "start", "end", "kind", "subject", "notes"])?;
    let title = state::text(v, "title")?;
    let start = state::date(v, "start")?.ok_or("event needs a start date")?;
    let end = state::date(v, "end")?;
    let kind = state::nullable_text(v, "kind")?.unwrap_or_else(|| "note".into());
    if !["note", "due", "class"].contains(&kind.as_str()) {
        return Err("event kind must be note, due or class".into());
    }
    if kind != "class" && end.is_some() {
        return Err("only class events have an end".into());
    }
    let code = state::nullable_text(v, "subject")?;
    let subject = inspect::subject(conn, code.as_deref()).await?.map(|s| s.0);
    let all_day = start.len() == 10;
    Ok((
        subject,
        kind,
        title,
        start,
        end,
        all_day,
        state::nullable_text(v, "notes")?,
    ))
}
async fn create_event(conn: &mut SqliteConnection, v: &Value) -> Result<i64, String> {
    let (subject, kind, title, start, end, all_day, notes) = event_fields(conn, v).await?;
    Ok(sqlx::query("INSERT INTO local_events (subject_id,kind,title,start_at,end_at,all_day,notes,source) VALUES (?1,?2,?3,?4,?5,?6,?7,'agent')").bind(subject).bind(kind).bind(title).bind(start).bind(end).bind(i64::from(all_day)).bind(notes).execute(&mut *conn).await.map_err(|e|e.to_string())?.last_insert_rowid())
}
