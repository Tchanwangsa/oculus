//! A chat as a reload reads it: its rows, its runs, the text its open run
//! has streamed so far and the revision that covers them all, from one read
//! transaction. The webview skips the rows of any envelope at or below that
//! revision and the deltas of any below it, so a reload neither misses a
//! change nor applies one twice.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::harness::handoff::store::{self as handoff, HandoffView};
use crate::harness::store::{queue, QueuedMessage};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: i64,
    pub items: Vec<Item>,
    pub runs: Vec<Run>,
    /// The student's messages waiting their turn, in queue order.
    pub queued: Vec<QueuedMessage>,
    /// The answer text the open run has streamed and not committed as a row
    /// yet. It holds every `assistant_delta` told below `revision`, and none
    /// told at or above it.
    pub draft: Option<Draft>,
}

#[derive(Serialize)]
pub struct Draft {
    pub run_id: i64,
    pub text: String,
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
/// that never started reads its interval from its request. Only runs with a
/// timeline row are listed: one that waits is in `queued`, and one that
/// left the queue without starting has nothing to draw.
#[derive(Serialize)]
pub struct Run {
    pub id: i64,
    pub execution_status: String,
    pub delivery_state: String,
    pub answer_item_id: Option<i64>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    /// When the student asked it to stop, if they did.
    pub stop_requested_at: Option<String>,
    /// What it was sent with, and the model its CLI reported where that
    /// differs from the one asked for.
    pub provider: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub resolved_model: Option<String>,
    /// The earlier conversation its first prompt carried, if any.
    pub handoff: Option<HandoffView>,
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
    let mut handoffs = handoff::views(&mut *tx, thread_id).await?;
    let runs = sqlx::query(
        "SELECT r.id, r.execution_status, r.delivery_state, r.answer_item_id,
                COALESCE(r.started_at, r.requested_at) AS started_at, r.ended_at,
                r.stop_requested_at, b.provider, r.model, r.effort, r.resolved_model
           FROM harness_runs r JOIN harness_bindings b ON b.id = r.binding_id
          WHERE r.thread_id = ?1 AND r.execution_status NOT IN ('queued', 'cancelled')
            AND r.initiating_item_id IS NOT NULL
          ORDER BY r.id",
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
        stop_requested_at: r.get("stop_requested_at"),
        provider: r.get("provider"),
        model: r.get("model"),
        effort: r.get("effort"),
        resolved_model: r.get("resolved_model"),
        handoff: handoffs.remove(&r.get::<i64, _>("id")),
    })
    .collect();
    let queued = queue::list(&mut *tx, thread_id).await?;
    let draft = sqlx::query(
        "SELECT d.run_id, d.text FROM harness_stream_drafts d
           JOIN harness_runs r ON r.id = d.run_id
          WHERE r.thread_id = ?1 AND r.execution_status IN ('preparing', 'starting', 'running')
          ORDER BY d.run_id DESC LIMIT 1",
    )
    .bind(thread_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(err)?
    .map(|r| Draft {
        run_id: r.get("run_id"),
        text: r.get("text"),
    });
    tx.commit().await.map_err(err)?;
    Ok(Snapshot {
        revision,
        items,
        runs,
        queued,
        draft,
    })
}
