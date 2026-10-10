use sqlx::{Row, SqlitePool};

pub(super) async fn insert_item(
    pool: &SqlitePool,
    thread_id: i64,
    kind: &str,
    ref_id: Option<&str>,
    content: Option<&str>,
    meta: Option<String>,
) -> Result<i64, String> {
    let res = sqlx::query(
        "INSERT INTO harness_items (thread_id, kind, ref_id, content, meta) VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(thread_id)
    .bind(kind)
    .bind(ref_id)
    .bind(content)
    .bind(meta)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(res.last_insert_rowid())
}

pub(super) async fn set_status(
    pool: &SqlitePool,
    thread_id: i64,
    status: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE harness_threads SET status = ?2, updated_at = datetime('now') WHERE id = ?1",
    )
    .bind(thread_id)
    .bind(status)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// A question in this thread and its turn's anchor, checked against the row
/// before an edit deletes everything after it. `anchor` is None for a question
/// predating migration 28 or whose turn never started.
pub struct Question {
    pub text: String,
    pub anchor: Option<String>,
}

pub async fn user_item(
    pool: &SqlitePool,
    thread_id: i64,
    item_id: i64,
) -> Result<Question, String> {
    let row = sqlx::query(
        "SELECT kind, content, anchor FROM harness_items WHERE id = ?1 AND thread_id = ?2",
    )
    .bind(item_id)
    .bind(thread_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("no item {item_id} in thread {thread_id}"))?;
    let kind: String = row.get("kind");
    if kind != "user" {
        return Err(format!("item {item_id} is a {kind} row, not a question"));
    }
    Ok(Question {
        text: row.get::<Option<String>, _>("content").unwrap_or_default(),
        anchor: row.get::<Option<String>, _>("anchor"),
    })
}

/// The anchor of the thread's newest question — the last turn the timeline
/// has seen, which Claude needs before it will rewind past later turns.
pub async fn newest_anchor(pool: &SqlitePool, thread_id: i64) -> Result<Option<String>, String> {
    sqlx::query_scalar(
        "SELECT anchor FROM harness_items WHERE thread_id = ?1 AND kind = 'user' ORDER BY id DESC LIMIT 1",
    )
    .bind(thread_id)
    .fetch_optional(pool)
    .await
    .map(Option::flatten)
    .map_err(|e| e.to_string())
}

/// Delete this row and everything after it — the local half of a rewind
/// (`Harness::rewind` rewinds the provider's session).
pub async fn truncate_from(pool: &SqlitePool, thread_id: i64, item_id: i64) -> Result<u64, String> {
    sqlx::query("DELETE FROM harness_items WHERE thread_id = ?1 AND id >= ?2")
        .bind(thread_id)
        .bind(item_id)
        .execute(pool)
        .await
        .map(|r| r.rows_affected())
        .map_err(|e| e.to_string())
}
