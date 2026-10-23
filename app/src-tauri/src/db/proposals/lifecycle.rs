use super::{state, Proposal};
use serde_json::Value;
use sqlx::{Row, SqlitePool};

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Proposal, String> {
    let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
    state::on(&mut conn, id).await
}
pub async fn list(pool: &SqlitePool, thread: i64) -> Result<Vec<Proposal>, String> {
    let ids = sqlx::query(
        "SELECT item_id,block FROM chat_proposals WHERE thread_id=?1 ORDER BY item_id,block",
    )
    .bind(thread)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let mut out = vec![];
    for r in ids {
        out.push(
            get(
                pool,
                &format!(
                    "{}.{}",
                    r.get::<i64, _>("item_id"),
                    r.get::<i64, _>("block")
                ),
            )
            .await?,
        );
    }
    Ok(out)
}

pub async fn open(
    pool: &SqlitePool,
    thread: i64,
    item: i64,
    block: i64,
    kind: &str,
    value: &Value,
    amends: Option<&str>,
) -> Result<Proposal, String> {
    if item <= 0 || !(0..=1000).contains(&block) {
        return Err("proposal needs a committed reply and valid block index".into());
    }
    state::validate_shape(kind, value)?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    // Acquiring the write lock before reading serializes two views opening/amending.
    let changed=sqlx::query("INSERT OR IGNORE INTO chat_proposals (item_id,block,thread_id,kind,state,initial_state) SELECT ?1,?2,?3,?4,?5,?5 WHERE EXISTS (SELECT 1 FROM harness_items i JOIN harness_threads t ON t.id=i.thread_id WHERE i.id=?1 AND i.thread_id=?3 AND i.kind='assistant')")
        .bind(item).bind(block).bind(thread).bind(kind).bind(value.to_string()).execute(&mut *tx).await.map_err(|e|e.to_string())?.rows_affected();
    let id = format!("{item}.{block}");
    let stored = state::on(&mut tx, &id).await?;
    if stored.thread_id != thread || stored.kind != kind {
        return Err("proposal does not match its reply".into());
    }
    let origin: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM harness_items WHERE id=?1 AND thread_id=?2 AND kind='assistant'",
    )
    .bind(item)
    .bind(thread)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if origin != 1 {
        return Err("the proposal's reply no longer exists".into());
    }
    if changed > 0 {
        if let Some(previous) = amends {
            if let Ok((old, old_block)) = state::key(previous) {
                if old < item {
                    let previous = sqlx::query("SELECT kind,state FROM chat_proposals WHERE item_id=?1 AND block=?2 AND thread_id=?3 AND status='pending'")
                        .bind(old).bind(old_block).bind(thread).fetch_optional(&mut *tx).await.map_err(|e|e.to_string())?;
                    if let Some(previous) = previous {
                        let old_state: Value =
                            serde_json::from_str(&previous.get::<String, _>("state"))
                                .map_err(|e| e.to_string())?;
                        let mut next = value.clone();
                        super::amendments::inherit(
                            kind,
                            &previous.get::<String, _>("kind"),
                            &mut next,
                            &old_state,
                        );
                        sqlx::query(
                            "UPDATE chat_proposals SET state=?1 WHERE item_id=?2 AND block=?3",
                        )
                        .bind(next.to_string())
                        .bind(item)
                        .bind(block)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                    }
                    sqlx::query("UPDATE chat_proposals SET status='superseded',superseded_by=?1,updated_at=datetime('now') WHERE item_id=?2 AND block=?3 AND thread_id=?4 AND status='pending'")
                .bind(&id).bind(old).bind(old_block).bind(thread).execute(&mut *tx).await.map_err(|e|e.to_string())?;
                }
            }
        }
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    get(pool, &id).await
}

pub async fn edit(pool: &SqlitePool, id: &str, value: &Value) -> Result<Proposal, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let (item, block) = state::key(id)?;
    // Write first: no deferred read-to-write upgrade can race another view.
    let n=sqlx::query("UPDATE chat_proposals SET updated_at=datetime('now') WHERE item_id=?1 AND block=?2 AND status='pending'").bind(item).bind(block).execute(&mut *tx).await.map_err(|e|e.to_string())?.rows_affected();
    if n != 1 {
        return Err("this proposal is already decided".into());
    }
    let row = state::on(&mut tx, id).await?;
    state::validate_shape(&row.kind, value)?;
    sqlx::query("UPDATE chat_proposals SET state=?1 WHERE item_id=?2 AND block=?3")
        .bind(value.to_string())
        .bind(item)
        .bind(block)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    get(pool, id).await
}

pub async fn discard(pool: &SqlitePool, id: &str) -> Result<Proposal, String> {
    let (item, block) = state::key(id)?;
    let n=sqlx::query("UPDATE chat_proposals SET status='discarded',updated_at=datetime('now') WHERE item_id=?1 AND block=?2 AND status='pending'").bind(item).bind(block).execute(pool).await.map_err(|e|e.to_string())?.rows_affected();
    if n != 1 {
        return Err("this proposal is already decided".into());
    }
    get(pool, id).await
}
