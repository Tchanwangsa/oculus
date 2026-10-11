use sqlx::{SqliteConnection, SqlitePool};

use crate::db::projects::rows::{to_task, Task};

pub(super) async fn has_children(conn: &mut SqliteConnection, id: i64) -> Result<bool, String> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_tasks WHERE parent_id = ?1")
        .bind(id)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(n > 0)
}

/// Bump a project's `updated_at`; a no-op for `None`.
pub(super) async fn touch_project(
    conn: &mut SqliteConnection,
    id: Option<i64>,
) -> Result<(), String> {
    let Some(id) = id else { return Ok(()) };
    sqlx::query("UPDATE projects SET updated_at = datetime('now') WHERE id = ?1")
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn task(pool: &SqlitePool, id: i64) -> Result<Option<Task>, String> {
    let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
    task_on(&mut *conn, id).await
}

pub(super) async fn task_on(conn: &mut SqliteConnection, id: i64) -> Result<Option<Task>, String> {
    let row = sqlx::query("SELECT * FROM project_tasks WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.as_ref().map(to_task))
}
