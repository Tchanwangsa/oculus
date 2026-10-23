use sqlx::{SqliteConnection, SqlitePool};

use crate::db::projects::tags::{ensure_tags_on, replace_task_tags_on};

use super::create::assert_can_parent;
use super::links::{attach_files_on, detach_files_on, resolve_files_on};
use super::lookup::{has_children, task_on, touch_project, touch_task};

/// Everything except where a task sits: `column_id`, `position` and `done_at`
/// are written only by {@link move_task}, which reads the column's kind.
#[derive(Default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub body: Option<Option<String>>,
    pub parent_id: Option<Option<i64>>,
    pub starts_at: Option<Option<String>>,
    pub due_at: Option<Option<String>>,
    pub estimate_minutes: Option<Option<i64>>,
    /// Tag names that replace the whole set (`Some(vec![])` clears it); a name
    /// the registry lacks is made, in the default colour.
    pub tags: Option<Vec<String>>,
    /// Library `relative_path`s to attach; one already attached is a no-op.
    pub attach: Vec<String>,
    /// Library `relative_path`s to detach; one not attached is a no-op.
    pub detach: Vec<String>,
}

pub async fn update_task(pool: &SqlitePool, id: i64, patch: &TaskPatch) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    update_task_on(&mut tx, id, patch).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn update_task_on(
    conn: &mut SqliteConnection,
    id: i64,
    patch: &TaskPatch,
) -> Result<(), String> {
    let existing = task_on(conn, id)
        .await?
        .ok_or_else(|| format!("task {id} does not exist"))?;

    if let Some(Some(parent)) = patch.parent_id {
        if parent == id {
            return Err("a task cannot be its own parent".to_string());
        }
        assert_can_parent(conn, parent, existing.project_id).await?;
        if has_children(conn, id).await? {
            return Err(
                "subtasks are one level deep: a task with children cannot have a parent"
                    .to_string(),
            );
        }
    }

    // Resolved before anything is written, so a bad path or name changes nothing.
    let mut linked = false;
    if let Some(names) = &patch.tags {
        let ids = ensure_tags_on(conn, names).await?;
        replace_task_tags_on(conn, id, &ids).await?;
        linked = true;
    }
    if !patch.attach.is_empty() {
        let ids = resolve_files_on(conn, &patch.attach).await?;
        linked |= attach_files_on(conn, id, &ids).await?;
    }
    if !patch.detach.is_empty() {
        let ids = resolve_files_on(conn, &patch.detach).await?;
        linked |= detach_files_on(conn, id, &ids).await?;
    }

    let mut sets: Vec<String> = Vec::new();
    let mut put = |column: &str| sets.push(format!("{column} = ?{}", sets.len() + 1));
    if patch.title.is_some() {
        put("title");
    }
    if patch.body.is_some() {
        put("body");
    }
    if patch.parent_id.is_some() {
        put("parent_id");
    }
    if patch.starts_at.is_some() {
        put("starts_at");
    }
    if patch.due_at.is_some() {
        put("due_at");
    }
    if patch.estimate_minutes.is_some() {
        put("estimate_minutes");
    }
    if sets.is_empty() {
        if linked {
            touch_task(conn, id).await?;
            touch_project(conn, existing.project_id).await?;
            return Ok(());
        }
        return Ok(());
    }
    let sql = format!(
        "UPDATE project_tasks SET {}, updated_at = datetime('now') WHERE id = ?{}",
        sets.join(", "),
        sets.len() + 1
    );
    let mut q = sqlx::query(&sql);
    if let Some(v) = &patch.title {
        q = q.bind(v);
    }
    if let Some(v) = &patch.body {
        q = q.bind(v);
    }
    if let Some(v) = &patch.parent_id {
        q = q.bind(*v);
    }
    if let Some(v) = &patch.starts_at {
        q = q.bind(v);
    }
    if let Some(v) = &patch.due_at {
        q = q.bind(v);
    }
    if let Some(v) = &patch.estimate_minutes {
        q = q.bind(*v);
    }
    q.bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;

    touch_project(conn, existing.project_id).await?;
    Ok(())
}

/// Deletes the task and (by cascade) its subtasks; returns how many rows went.
pub async fn delete_task(pool: &SqlitePool, id: i64) -> Result<u64, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let result = delete_task_on(&mut tx, id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(result)
}

pub async fn delete_task_on(conn: &mut SqliteConnection, id: i64) -> Result<u64, String> {
    let existing = task_on(conn, id)
        .await?
        .ok_or_else(|| format!("task {id} does not exist"))?;
    let children: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM project_tasks WHERE parent_id = ?1")
            .bind(id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM project_tasks WHERE id = ?1")
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    touch_project(conn, existing.project_id).await?;
    Ok(children as u64 + 1)
}
