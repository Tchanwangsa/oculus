use super::{inspect, journal::Journal, state};
use crate::db::projects::{self, ProjectPatch, TaskPatch};
use serde_json::Value;
use sqlx::SqliteConnection;

const TASK: &[&str] = &[
    "title", "body", "due", "starts", "estimate", "tags", "files", "column", "project",
];
const PROJECT: &[&str] = &[
    "name", "subject", "due", "starts", "brief", "tags", "event", "status",
];
const EVENT: &[&str] = &["title", "start", "end", "kind", "subject", "notes"];
const TAG: &[&str] = &["name", "color"];

pub(super) async fn run(
    conn: &mut SqliteConnection,
    kind: &str,
    v: &Value,
    j: &mut Journal,
) -> Result<(), String> {
    let entity = match kind {
        "TaskChanges" => "task",
        "ProjectChanges" => "project",
        "EventChanges" => "event",
        _ => "tag",
    };
    let keys = match entity {
        "task" => TASK,
        "project" => PROJECT,
        "event" => EVENT,
        _ => TAG,
    };
    for change in state::rows(v, "changes")? {
        if !state::included(change) {
            continue;
        }
        if entity == "tag" && change.get("id").is_none() {
            if change.get("tag").is_some() {
                state::object(change, &["tag", "kind"])?;
                let id = state::id(&change["tag"])?;
                j.tag_dependents(conn, id).await?;
                projects::delete_tag_on(conn, id).await?;
            } else {
                state::object(change, &["name", "color", "kind"])?;
                let name = state::text(change, "name")?;
                let color = state::text(change, "color")?;
                let watching = j.tag_names(conn, std::slice::from_ref(&name)).await?;
                projects::ensure_tag_on(conn, &name, Some(&color)).await?;
                j.added_tags(conn, &watching).await?;
            }
            continue;
        }
        state::object(change, &["id", "from", "to", "kind"])?;
        let id = if entity == "event" {
            change["id"]
                .as_str()
                .ok_or("event id must be a local_ string")?
                .to_owned()
        } else {
            state::id(&change["id"])?.to_string()
        };
        let from = state::object(&change["from"], keys)?;
        let to = state::object(&change["to"], keys)?;
        if from.keys().ne(to.keys()) {
            return Err("from and to must contain the same fields".into());
        }
        let live = inspect::current(conn, entity, &id).await?;
        if live.is_null() {
            return Err(format!("{entity} {id} no longer exists"));
        }
        for (key, expected) in from {
            if live.get(key) != Some(expected) {
                return Err(format!("{entity} {id} changed since this proposal ({key})"));
            }
        }
        if to.is_empty() || to == from {
            continue;
        }
        let n = inspect::numeric(entity, &id)?;
        match entity {
            "task" => task(conn, n, &change["to"], &live, j).await?,
            "project" => project(conn, n, &change["to"], j).await?,
            "tag" => {
                j.tag_dependents(conn, n).await?;
                projects::update_tag_on(
                    conn,
                    n,
                    nonempty_patch(&change["to"], "name")?.as_deref(),
                    nonempty_patch(&change["to"], "color")?.as_deref(),
                )
                .await?;
            }
            "event" => {
                j.watch(conn, "event", &id).await?;
                let mut next = serde_json::Map::new();
                for key in EVENT {
                    next.insert((*key).into(), live[*key].clone());
                }
                for (key, v) in to {
                    next.insert(key.clone(), v.clone());
                }
                let (subject, kind, title, start, end, all_day, notes) =
                    super::mutations::event_fields(conn, &Value::Object(next)).await?;
                sqlx::query("UPDATE local_events SET subject_id=?1,kind=?2,title=?3,start_at=?4,end_at=?5,all_day=?6,notes=?7 WHERE id=?8").bind(subject).bind(kind).bind(title).bind(start).bind(end).bind(i64::from(all_day)).bind(notes).bind(n).execute(&mut *conn).await.map_err(|e|e.to_string())?;
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}
fn nullable_patch(v: &Value, key: &str) -> Result<Option<Option<String>>, String> {
    v.get(key).map(|_| state::nullable_text(v, key)).transpose()
}
fn nonempty_patch(v: &Value, key: &str) -> Result<Option<String>, String> {
    v.get(key).map(|_| state::text(v, key)).transpose()
}
fn date_patch(v: &Value, key: &str) -> Result<Option<Option<String>>, String> {
    v.get(key).map(|_| state::date(v, key)).transpose()
}

async fn task(
    conn: &mut SqliteConnection,
    id: i64,
    v: &Value,
    live: &Value,
    j: &mut Journal,
) -> Result<(), String> {
    j.task(conn, id, v.get("project").is_some()).await?;
    if v.get("project").is_some() {
        let project = state::nullable_id(v.get("project"))?;
        if let Some(p) = project {
            j.watch(conn, "project", &p.to_string()).await?;
        }
        projects::refile_task_on(conn, id, project).await?;
    }
    let tags = v
        .get("tags")
        .map(|_| state::strings(v, "tags"))
        .transpose()?;
    let watching = j.tag_names(conn, tags.as_deref().unwrap_or(&[])).await?;
    let files = v
        .get("files")
        .map(|_| state::strings(v, "files"))
        .transpose()?;
    let old_files: Vec<String> = live["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let estimate = match v.get("estimate") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(n) => Some(Some(
            n.as_i64()
                .filter(|n| *n >= 0)
                .ok_or("estimate needs whole nonnegative minutes")?,
        )),
    };
    let patch = TaskPatch {
        title: nonempty_patch(v, "title")?,
        body: nullable_patch(v, "body")?,
        parent_id: None,
        starts_at: date_patch(v, "starts")?,
        due_at: date_patch(v, "due")?,
        estimate_minutes: estimate,
        tags,
        attach: files.clone().unwrap_or_default(),
        detach: if let Some(wanted) = &files {
            old_files
                .into_iter()
                .filter(|f| !wanted.contains(f))
                .collect()
        } else {
            vec![]
        },
    };
    projects::update_task_on(conn, id, &patch).await?;
    j.added_tags(conn, &watching).await?;
    if let Some(column) = nonempty_patch(v, "column")? {
        let above:Option<i64>=sqlx::query_scalar("SELECT id FROM project_tasks WHERE project_id IS (SELECT project_id FROM project_tasks WHERE id=?1) AND column_id=?2 AND id!=?1 ORDER BY position DESC,id DESC LIMIT 1").bind(id).bind(&column).fetch_optional(&mut *conn).await.map_err(|e|e.to_string())?;
        projects::move_task_on(conn, id, &column, above, None).await?;
    }
    Ok(())
}
async fn project(
    conn: &mut SqliteConnection,
    id: i64,
    v: &Value,
    j: &mut Journal,
) -> Result<(), String> {
    j.watch(conn, "project", &id.to_string()).await?;
    let subject = if v.get("subject").is_some() {
        let code = state::nullable_text(v, "subject")?;
        Some(inspect::subject(conn, code.as_deref()).await?.map(|s| s.0))
    } else {
        None
    };
    let tags = v
        .get("tags")
        .map(|_| state::strings(v, "tags"))
        .transpose()?;
    let watching = j.tag_names(conn, tags.as_deref().unwrap_or(&[])).await?;
    let status = nonempty_patch(v, "status")?;
    if status
        .as_ref()
        .is_some_and(|s| s != "active" && s != "archived")
    {
        return Err("project status must be active or archived".into());
    }
    projects::update_project_on(
        conn,
        id,
        &ProjectPatch {
            name: nonempty_patch(v, "name")?,
            subject_id: subject,
            brief: nullable_patch(v, "brief")?,
            status,
            starts_at: date_patch(v, "starts")?,
            due_at: date_patch(v, "due")?,
            tags,
            event_id: nullable_patch(v, "event")?,
        },
    )
    .await?;
    j.added_tags(conn, &watching).await?;
    Ok(())
}
