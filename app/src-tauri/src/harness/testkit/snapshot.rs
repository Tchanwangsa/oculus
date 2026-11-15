use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

pub(super) async fn items(pool: &SqlitePool, thread: i64) -> Vec<Value> {
    sqlx::query("SELECT * FROM harness_items WHERE thread_id = ?1 ORDER BY id ASC")
        .bind(thread).fetch_all(pool).await.unwrap().into_iter().map(|row| json!({
            "id": row.get::<i64,_>("id"), "thread_id": thread,
            "kind": row.get::<String,_>("kind"), "ref_id": row.get::<Option<String>,_>("ref_id"),
            "content": row.get::<Option<String>,_>("content"), "meta": row.get::<Option<String>,_>("meta"),
            "created_at": row.get::<String,_>("created_at"), "anchor": row.get::<Option<String>,_>("anchor"),
            "run_id": row.get::<Option<i64>,_>("run_id"), "attempt_id": row.get::<Option<i64>,_>("attempt_id"),
            "entity_ref": row.get::<Option<String>,_>("entity_ref"),
            "source_ref": row.get::<Option<String>,_>("source_ref"), "native_phase": row.get::<Option<String>,_>("native_phase"),
            "lifecycle": row.get::<Option<String>,_>("lifecycle"), "display_role": row.get::<Option<String>,_>("display_role"),
        })).collect()
}

pub(super) async fn ledger(pool: &SqlitePool, thread: i64) -> Value {
    let rows = |sql: &'static str| async move {
        sqlx::query(sql).bind(thread).fetch_all(pool).await.unwrap()
    };
    let runs = rows("SELECT id, initiating_item_id, binding_id, active_attempt_id, execution_status, origin, delivery_state, answer_item_id, COALESCE(started_at, requested_at) AS started_at, ended_at FROM harness_runs WHERE thread_id = ?1 ORDER BY id").await;
    let attempts = rows("SELECT a.id, a.run_id, a.status FROM harness_attempts a JOIN harness_runs r ON r.id = a.run_id WHERE r.thread_id = ?1 ORDER BY a.id").await;
    let bindings =
        rows("SELECT id, provider, native_session, current, covered_through FROM harness_bindings WHERE thread_id = ?1 ORDER BY id")
            .await;
    let receipts =
        rows("SELECT id, run_id FROM harness_command_receipts WHERE thread_id = ?1 ORDER BY id")
            .await;
    let gaps = rows("SELECT run_id, source_ref, cause, state FROM harness_delivery_gaps WHERE thread_id = ?1 ORDER BY id").await;
    json!({
        "runs": runs.iter().map(|r| json!({
            "id": r.get::<i64,_>("id"), "initiating_item_id": r.get::<Option<i64>,_>("initiating_item_id"),
            "binding_id": r.get::<i64,_>("binding_id"), "active_attempt_id": r.get::<Option<i64>,_>("active_attempt_id"),
            "execution_status": r.get::<String,_>("execution_status"), "origin": r.get::<String,_>("origin"),
            "delivery_state": r.get::<String,_>("delivery_state"), "answer_item_id": r.get::<Option<i64>,_>("answer_item_id"),
            "started_at": r.get::<Option<String>,_>("started_at"), "ended_at": r.get::<Option<String>,_>("ended_at"),
        })).collect::<Vec<_>>(),
        "attempts": attempts.iter().map(|r| json!({
            "id": r.get::<i64,_>("id"), "run_id": r.get::<i64,_>("run_id"), "status": r.get::<String,_>("status"),
        })).collect::<Vec<_>>(),
        "bindings": bindings.iter().map(|r| json!({
            "id": r.get::<i64,_>("id"), "native_session": r.get::<Option<String>,_>("native_session"),
            "provider": r.get::<String,_>("provider"), "current": r.get::<i64,_>("current") != 0,
            "covered_through": r.get::<i64,_>("covered_through"),
        })).collect::<Vec<_>>(),
        "receipts": receipts.iter().map(|r| json!({
            "id": r.get::<String,_>("id"), "run_id": r.get::<i64,_>("run_id"),
        })).collect::<Vec<_>>(),
        "gaps": gaps.iter().map(|r| json!({
            "run_id": r.get::<Option<i64>,_>("run_id"), "source_ref": r.get::<Option<String>,_>("source_ref"),
            "cause": r.get::<String,_>("cause"), "state": r.get::<String,_>("state"),
        })).collect::<Vec<_>>(),
    })
}
