use sqlx::SqlitePool;

use super::create::assert_can_parent;
use super::lookup::{has_children, task_on, touch_project};

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
}

pub async fn update_task(pool: &SqlitePool, id: i64, patch: &TaskPatch) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let existing = task_on(&mut *tx, id)
        .await?
        .ok_or_else(|| format!("task {id} does not exist"))?;

    if let Some(Some(parent)) = patch.parent_id {
        if parent == id {
            return Err("a task cannot be its own parent".to_string());
        }
        assert_can_parent(&mut *tx, parent, existing.project_id).await?;
        if has_children(&mut *tx, id).await? {
            return Err(
                "subtasks are one level deep: a task with children cannot have a parent"
                    .to_string(),
            );
        }
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
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    touch_project(&mut *tx, existing.project_id).await?;
    tx.commit().await.map_err(|e| e.to_string())
}

/// Deletes the task and (by cascade) its subtasks; returns how many rows went.
pub async fn delete_task(pool: &SqlitePool, id: i64) -> Result<u64, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let existing = task_on(&mut *tx, id)
        .await?
        .ok_or_else(|| format!("task {id} does not exist"))?;
    let children: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM project_tasks WHERE parent_id = ?1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM project_tasks WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    touch_project(&mut *tx, existing.project_id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(children as u64 + 1)
}
