//! Fresh card context is read when a queued turn is dispatched, never when queued.

use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

pub(super) struct Cards {
    pub section: String,
    versions: Vec<(i64, i64, String, String)>,
}

pub(super) async fn read(pool: &SqlitePool, thread: i64) -> Result<Cards, String> {
    let rows = sqlx::query("SELECT item_id, block, kind, status, state, initial_state FROM chat_proposals WHERE thread_id = ?1 AND (status = 'pending' OR told IS NULL OR told != status) ORDER BY item_id, block")
        .bind(thread).fetch_all(pool).await.map_err(|e| e.to_string())?;
    let mut versions = Vec::new();
    let mut cards = Vec::new();
    for row in rows {
        let item: i64 = row.get("item_id");
        let block: i64 = row.get("block");
        let id = format!("{item}.{block}");
        let status: String = row.get("status");
        let state: String = row.get("state");
        let mut card = json!({"id": id, "kind": row.get::<String,_>("kind"), "status": status});
        let parsed: Value = serde_json::from_str(&state).map_err(|e| format!("card {id}: {e}"))?;
        if let Some(job) = parsed.get("job") {
            card["job"] = job.clone();
        }
        if status == "pending" {
            card["card"] = serde_json::from_str(&state).map_err(|e| format!("card {id}: {e}"))?;
            card["edited"] = json!(
                serde_json::from_str::<Value>(&state).ok()
                    != serde_json::from_str::<Value>(&row.get::<String, _>("initial_state")).ok()
            );
        } else {
            let made: Vec<Value> = crate::db::proposals::get(pool, &id)
                .await?
                .made
                .into_iter()
                .map(|made| {
                    let id = if matches!(made.entity.as_str(), "project" | "task" | "tag") {
                        made.id
                            .parse::<i64>()
                            .map(Value::from)
                            .unwrap_or_else(|_| json!(made.id))
                    } else {
                        json!(made.id)
                    };
                    json!({"entity":made.entity,"id":id,"title":made.title,"op":made.op})
                })
                .collect();
            card["made"] = json!(made);
        }
        versions.push((item, block, status, state));
        cards.push(card);
    }
    let section = if cards.is_empty() {
        String::new()
    } else {
        format!("\n\n---\n\n## Cards in this thread (from Oculus, not typed by the student)\n\n{}\n\nTo change a pending card, write a new card with its id as the last argument.", serde_json::to_string(&cards).map_err(|e| e.to_string())?)
    };
    Ok(Cards { section, versions })
}

pub(super) async fn accepted(pool: &SqlitePool, cards: Cards) -> Result<(), String> {
    for (item, block, status, state) in cards.versions {
        sqlx::query("UPDATE chat_proposals SET told = ?3 WHERE item_id = ?1 AND block = ?2 AND status = ?3 AND state = ?4")
            .bind(item).bind(block).bind(status).bind(state).execute(pool).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}
