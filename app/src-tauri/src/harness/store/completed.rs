//! Completion is display state. Neither a manual change nor the idle sweep
//! releases, rewinds or otherwise changes a provider session.

use sqlx::{SqliteConnection, SqlitePool};

pub async fn set(conn: &mut SqliteConnection, thread: i64, completed: bool) -> Result<(), String> {
    let sql = if completed {
        "UPDATE harness_threads SET completed_at = COALESCE(completed_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) WHERE id = ?1"
    } else {
        "UPDATE harness_threads SET completed_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1"
    };
    let changed = sqlx::query(sql)
        .bind(thread)
        .execute(conn)
        .await
        .map_err(|e| e.to_string())?;
    if changed.rows_affected() == 0 {
        return Err(format!("no thread {thread}"));
    }
    Ok(())
}

/// One conditional write: a send or Reopen that won the database lock first
/// refreshes the age before this predicate is evaluated. Julian days accept
/// both SQLite timestamps and the ISO timestamps written by the command path.
pub async fn sweep(pool: &SqlitePool) -> Result<Vec<i64>, String> {
    sqlx::query_scalar(
        "UPDATE harness_threads AS t
         SET completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), revision = revision + 1
         WHERE t.completed_at IS NULL AND t.status NOT IN ('running', 'error', 'failed')
           AND julianday(t.updated_at) < julianday('now', '-3 days')
           AND NOT EXISTS (SELECT 1 FROM harness_runs r WHERE r.thread_id = t.id
                           AND r.execution_status IN ('preparing', 'starting', 'running', 'queued'))
           AND NOT EXISTS (SELECT 1 FROM harness_requests q WHERE q.thread_id = t.id AND q.status = 'pending')
           AND COALESCE((SELECT r.execution_status FROM harness_runs r WHERE r.thread_id = t.id
                         AND r.execution_status NOT IN ('queued', 'cancelled') ORDER BY r.ordinal DESC LIMIT 1), '') != 'failed'
         RETURNING id"
    ).fetch_all(pool).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
