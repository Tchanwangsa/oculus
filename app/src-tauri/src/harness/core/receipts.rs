//! `harness_receipts`: one row per accepted command, holding its result.

use sqlx::{Row, SqliteConnection, SqlitePool};

use super::command::{Outcome, DELETE_THREAD};

/// A command already accepted under this id.
pub(in crate::harness) struct Receipt {
    pub thread_id: i64,
    pub outcome: Outcome,
}

pub(in crate::harness) async fn find(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<Receipt>, String> {
    let row = sqlx::query("SELECT thread_id, result FROM harness_receipts WHERE id = ?1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    let Some(row) = row else { return Ok(None) };
    let thread_id: i64 = row.get("thread_id");
    let outcome = row
        .get::<Option<String>, _>("result")
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or(Outcome {
            thread_id,
            ..Outcome::default()
        });
    Ok(Some(Receipt { thread_id, outcome }))
}

/// Take the id for this command, first thing in its transaction: a second
/// use of it fails here and the whole commit with it.
pub(super) async fn reserve(
    tx: &mut SqliteConnection,
    id: &str,
    command: &str,
    thread_id: Option<i64>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO harness_receipts (id, thread_id, command, status) VALUES (?1, ?2, ?3, 'accepted')",
    )
    .bind(id)
    .bind(thread_id.unwrap_or_default())
    .bind(command)
    .execute(&mut *tx)
    .await
    .map(|_| ())
    .map_err(|e| format!("command {id} was already accepted: {e}"))
}

/// The command's result, and the thread it turned out to act on (a send
/// that started one learns its id inside the transaction).
pub(super) async fn store(
    tx: &mut SqliteConnection,
    id: &str,
    outcome: &Outcome,
) -> Result<(), String> {
    let result = serde_json::to_string(outcome).map_err(|e| e.to_string())?;
    sqlx::query("UPDATE harness_receipts SET thread_id = ?2, result = ?3 WHERE id = ?1")
        .bind(id)
        .bind(outcome.thread_id)
        .bind(result)
        .execute(&mut *tx)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// A deleted thread's receipts go with it, except the delete's own: a
/// repeat of that command must still find its answer.
pub(super) async fn forget(tx: &mut SqliteConnection, thread_id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM harness_receipts WHERE thread_id = ?1 AND command != ?2")
        .bind(thread_id)
        .bind(DELETE_THREAD)
        .execute(&mut *tx)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}
