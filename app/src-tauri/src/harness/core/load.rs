//! A chat as a reload reads it: its rows, its runs and the revision that
//! covers both, from one read transaction. The webview drops any envelope
//! at or below that revision, so a reload neither misses a change nor
//! applies one twice.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::harness::QueuedMessage;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: i64,
    pub items: Vec<Item>,
    pub runs: Vec<Run>,
    /// The student's messages waiting behind the running turn.
    pub queued: Vec<QueuedMessage>,
}

/// A `harness_items` row, as `HarnessItem` in `app/src/lib/harness/types.ts`.
#[derive(Serialize)]
pub struct Item {
    pub id: i64,
    pub thread_id: i64,
    pub kind: String,
    pub ref_id: Option<String>,
    pub content: Option<String>,
    pub meta: Option<String>,
    pub created_at: String,
    pub display_role: Option<String>,
    pub run_id: Option<i64>,
}

/// A `harness_runs` row, as `RunRow` in `app/src/lib/harness/runs.ts`. A run
/// that never started reads its interval from its request.
#[derive(Serialize)]
pub struct Run {
    pub id: i64,
    pub execution_status: String,
    pub delivery_state: String,
    pub answer_item_id: Option<i64>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

pub async fn read(pool: &SqlitePool, thread_id: i64) -> Result<Snapshot, String> {
    let err = |e: sqlx::Error| e.to_string();
    let mut tx = pool.begin().await.map_err(err)?;
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM harness_threads WHERE id = ?1")
        .bind(thread_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(err)?
        .unwrap_or_default();
    let items = sqlx::query(
        "SELECT id, thread_id, kind, ref_id, content, meta, created_at, display_role, run_id
           FROM harness_items WHERE thread_id = ?1 ORDER BY id ASC",
    )
    .bind(thread_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?
    .iter()
    .map(|r| Item {
        id: r.get("id"),
        thread_id: r.get("thread_id"),
        kind: r.get("kind"),
        ref_id: r.get("ref_id"),
        content: r.get("content"),
        meta: r.get("meta"),
        created_at: r.get("created_at"),
        display_role: r.get("display_role"),
        run_id: r.get("run_id"),
    })
    .collect();
    let runs = sqlx::query(
        "SELECT id, execution_status, delivery_state, answer_item_id,
                COALESCE(started_at, requested_at) AS started_at, ended_at
           FROM harness_runs WHERE thread_id = ?1 ORDER BY id",
    )
    .bind(thread_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?
    .iter()
    .map(|r| Run {
        id: r.get("id"),
        execution_status: r.get("execution_status"),
        delivery_state: r.get("delivery_state"),
        answer_item_id: r.get("answer_item_id"),
        started_at: r.get("started_at"),
        ended_at: r.get("ended_at"),
    })
    .collect();
    tx.commit().await.map_err(err)?;
    Ok(Snapshot {
        revision,
        items,
        runs,
        queued: Vec::new(),
    })
}
