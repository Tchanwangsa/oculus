//! Naming a thread once, the provider accounts' rate-limit windows, and the
//! startup sweep of threads and tool rows a crash left `running`.

use sqlx::{Row, SqlitePool};

use crate::harness::event::{HarnessEvent, Provider};

/// A naming turn's input: the first question and the last answer.
pub struct NamingSeed {
    pub first_message: String,
    pub reply: String,
}

/// The first or last non-empty row of one kind. `order` is a literal, never
/// user input.
async fn one_item(
    pool: &SqlitePool,
    thread_id: i64,
    kind: &str,
    order: &'static str,
) -> Result<Option<String>, String> {
    sqlx::query(&format!(
        "SELECT content FROM harness_items
         WHERE thread_id = ?1 AND kind = ?2 AND content IS NOT NULL AND content != ''
         ORDER BY id {order} LIMIT 1"
    ))
    .bind(thread_id)
    .bind(kind)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())
    .map(|r| r.and_then(|r| r.get::<Option<String>, _>("content")))
}

/// Claim the right to name this thread and return what to name it from. The
/// claim flips `title_generated` 0→1 atomically, so a thread is named once; a
/// failed naming turn is not retried, since each costs a real turn.
pub async fn claim_naming(pool: &SqlitePool, thread_id: i64) -> Result<Option<NamingSeed>, String> {
    let first_message = one_item(pool, thread_id, "user", "ASC").await?;
    let reply = one_item(pool, thread_id, "assistant", "DESC").await?;
    let (Some(first_message), Some(reply)) = (first_message, reply) else {
        // Nothing was said back — a failed first turn. Leave the claim open.
        return Ok(None);
    };
    let claimed = sqlx::query(
        "UPDATE harness_threads SET title_generated = 1 WHERE id = ?1 AND title_generated = 0",
    )
    .bind(thread_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();
    if claimed == 0 {
        return Ok(None);
    }
    Ok(Some(NamingSeed {
        first_message,
        reply,
    }))
}

/// Rate limits are per provider account, not per thread, so they live in
/// `settings` under the provider's key and the page reads them on load.
pub async fn save_rate_limits(
    pool: &SqlitePool,
    provider: Provider,
    ev: &HarnessEvent,
) -> Result<(), String> {
    let HarnessEvent::RateLimits { windows } = ev else {
        return Ok(());
    };
    let key = format!("harness_rate_limits_{}", provider.as_str());
    let value = serde_json::to_string(windows).map_err(|e| e.to_string())?;
    crate::db::store::set_setting(pool, &key, &value).await
}

/// What [`reconcile`] finished: threads set idle and tool rows closed.
pub struct Reconciled {
    pub threads: u64,
    pub tools: u64,
}

/// Threads left `running` and tool rows left open (`meta.ok` null) by a crash
/// or a quit mid-turn. Called at startup; nothing is going to finish them, and
/// an open row draws as running forever.
pub async fn reconcile(pool: &SqlitePool) -> Result<Reconciled, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let threads =
        sqlx::query("UPDATE harness_threads SET status = 'idle' WHERE status = 'running'")
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
            .rows_affected();
    // A `meta` that is not JSON is skipped rather than failing the sweep; only
    // CASE promises `json_valid` runs before `json_extract`.
    let tools = sqlx::query(
        "UPDATE harness_items
         SET meta = json_set(meta, '$.ok', json('false'), '$.output', ?1)
         WHERE kind = 'tool'
           AND CASE WHEN json_valid(meta) THEN json_extract(meta, '$.ok') IS NULL END",
    )
    .bind("the session ended before this finished")
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(Reconciled { threads, tools })
}
