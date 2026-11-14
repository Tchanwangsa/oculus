//! Runs, attempts, provider bindings and send receipts: the app's own identity
//! for each accepted send, written before the provider hears of it
//! (docs/harness.md, "Runs").

use std::sync::atomic::{AtomicU64, Ordering};

use sqlx::{Row, SqlitePool};

use crate::harness::event::answer::{select, Candidate};
use crate::harness::event::Provider;

/// What [`begin`] wrote: the run, its first attempt, the binding it runs on
/// and the question's row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunStart {
    pub run_id: i64,
    pub attempt_id: i64,
    pub binding_id: i64,
    pub item_id: i64,
}

/// A row's owner. `entity` is a weak, attempt-local ordinal (`weak:3`): the
/// bridges give no native ids yet, so it is never a provider handle.
pub struct Owner {
    pub run_id: i64,
    pub attempt_id: i64,
    pub entity: String,
}

/// One send, a student's or a card's continuation, as [`begin`] records it.
pub struct SendIntent {
    pub thread_id: i64,
    pub provider: Provider,
    pub model: Option<String>,
    /// The session this send resumes; `None` for a fresh one.
    pub native_session: Option<String>,
    pub receipt: String,
    pub text: String,
    pub at: Option<i64>,
    /// The card whose follow-up this run is (`app/continuation.rs`): its row is
    /// a `continuation` row holding `text`, never a question, and the run's
    /// origin is `continuation`. `None` for a student send.
    pub continuation: Option<String>,
}

/// [`begin`]'s refusal of a continuation its card no longer holds a claim
/// for (a rewind took the card, or it was stopped): nothing was written.
pub const NOT_CLAIMED: &str = "the card's continuation is no longer queued";

/// A receipt for a send whose caller named none; unique per process and time.
pub fn fresh_receipt() -> String {
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("send-{nanos:x}-{}", N.fetch_add(1, Ordering::Relaxed))
}

/// The run a receipt already started, if any.
pub async fn receipt_run(pool: &SqlitePool, receipt: &str) -> Result<Option<i64>, String> {
    sqlx::query_scalar("SELECT run_id FROM harness_command_receipts WHERE id = ?1")
        .bind(receipt)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())
}

/// Write the question's row, its run, first attempt, binding and receipt in
/// one transaction. Any failure — a reused receipt, or a continuation whose
/// card lost its claim ([`NOT_CLAIMED`]) — writes nothing, and the caller
/// must not send.
pub async fn begin(pool: &SqlitePool, send: &SendIntent) -> Result<RunStart, String> {
    let err = |e: sqlx::Error| e.to_string();
    let mut tx = pool.begin().await.map_err(err)?;
    let taken: Option<i64> =
        sqlx::query_scalar("SELECT run_id FROM harness_command_receipts WHERE id = ?1")
            .bind(&send.receipt)
            .fetch_optional(&mut *tx)
            .await
            .map_err(err)?;
    if let Some(run) = taken {
        return Err(format!("send {} already started run {run}", send.receipt));
    }
    if let Some(card) = &send.continuation {
        if !super::super::app::continuation::start(&mut tx, send.thread_id, card).await? {
            return Err(NOT_CLAIMED.into());
        }
    }
    let bound: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM harness_bindings
          WHERE thread_id = ?1 AND provider = ?2 AND native_session IS ?3
          ORDER BY id DESC LIMIT 1",
    )
    .bind(send.thread_id)
    .bind(send.provider.as_str())
    .bind(&send.native_session)
    .fetch_optional(&mut *tx)
    .await
    .map_err(err)?;
    let binding_id = match bound {
        Some(id) => id,
        None => sqlx::query(
            "INSERT INTO harness_bindings (thread_id, provider, native_session) VALUES (?1, ?2, ?3)",
        )
        .bind(send.thread_id)
        .bind(send.provider.as_str())
        .bind(&send.native_session)
        .execute(&mut *tx)
        .await
        .map_err(err)?
        .last_insert_rowid(),
    };
    sqlx::query(
        "UPDATE harness_threads SET status = 'running', updated_at = datetime('now') WHERE id = ?1",
    )
    .bind(send.thread_id)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    // `content` is only what the student typed; the playhead second is meta.
    // A continuation's row is Oculus's line about its card instead.
    let (kind, origin, command, meta) = match &send.continuation {
        Some(card) => (
            "continuation",
            "continuation",
            "continuation",
            Some(serde_json::json!({ "proposal": card }).to_string()),
        ),
        None => (
            "user",
            "student",
            "send",
            send.at
                .map(|at| serde_json::json!({ "at": at }).to_string()),
        ),
    };
    let item_id = sqlx::query(
        "INSERT INTO harness_items (thread_id, kind, content, meta) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(send.thread_id)
    .bind(kind)
    .bind(&send.text)
    .bind(meta)
    .execute(&mut *tx)
    .await
    .map_err(err)?
    .last_insert_rowid();
    let run_id = sqlx::query(
        "INSERT INTO harness_runs
           (thread_id, ordinal, origin, initiating_item_id, binding_id, model, execution_status)
         VALUES (?1, (SELECT COALESCE(MAX(ordinal), 0) + 1 FROM harness_runs WHERE thread_id = ?1),
                 ?5, ?2, ?3, ?4, 'preparing')",
    )
    .bind(send.thread_id)
    .bind(item_id)
    .bind(binding_id)
    .bind(&send.model)
    .bind(origin)
    .execute(&mut *tx)
    .await
    .map_err(err)?
    .last_insert_rowid();
    if let Some(card) = &send.continuation {
        super::super::app::continuation::started(&mut tx, card, run_id).await?;
    }
    let attempt_id = sqlx::query(
        "INSERT INTO harness_attempts (run_id, ordinal, reason, status) VALUES (?1, 1, 'initial', 'preparing')",
    )
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(err)?
    .last_insert_rowid();
    sqlx::query("UPDATE harness_runs SET active_attempt_id = ?2 WHERE id = ?1")
        .bind(run_id)
        .bind(attempt_id)
        .execute(&mut *tx)
        .await
        .map_err(err)?;
    sqlx::query("UPDATE harness_items SET run_id = ?2 WHERE id = ?1")
        .bind(item_id)
        .bind(run_id)
        .execute(&mut *tx)
        .await
        .map_err(err)?;
    sqlx::query(
        "INSERT INTO harness_command_receipts (id, thread_id, command, run_id) VALUES (?1, ?2, ?4, ?3)",
    )
    .bind(&send.receipt)
    .bind(send.thread_id)
    .bind(run_id)
    .bind(command)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    tx.commit().await.map_err(err)?;
    Ok(RunStart {
        run_id,
        attempt_id,
        binding_id,
        item_id,
    })
}

/// The provider accepted the send. Conditional, since the turn may already
/// have finished on the consumer by the time the write returns.
pub async fn mark_sent(pool: &SqlitePool, run: &RunStart) -> Result<(), String> {
    for (sql, id) in [
        (
            "UPDATE harness_runs SET execution_status = 'running', started_at = datetime('now')
              WHERE id = ?1 AND execution_status = 'preparing'",
            run.run_id,
        ),
        (
            "UPDATE harness_attempts SET status = 'running', started_at = datetime('now')
              WHERE id = ?1 AND status = 'preparing'",
            run.attempt_id,
        ),
    ] {
        sqlx::query(sql)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// What [`settle`] decided for a run, for the webview.
pub struct Settlement {
    pub answer_item_id: Option<i64>,
    /// Earlier replies of the attempt, now work.
    pub work_item_ids: Vec<i64>,
    pub delivery: &'static str,
    /// The notice row and its text, when delivery is incomplete with no other outcome row.
    pub notice: Option<(i64, String)>,
    /// `completed`, `interrupted` or `failed`.
    pub status: &'static str,
    /// The run's stored wall-clock interval (SQLite UTC), the timeline's "Worked for".
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

pub const NO_ANSWER: &str = "The agent finished, but its answer did not reach Oculus.";
pub const PART_MISSING: &str =
    "Some earlier reply text did not reach Oculus. This reply may be incomplete.";

/// Close the attempt and run on their `TurnFinished`, once: a repeat returns
/// `None` and changes nothing. In the same transaction, set the thread's
/// status, select the answer, mark the other replies as work, judge delivery
/// and write the notice row.
pub async fn settle(
    pool: &SqlitePool,
    run_id: i64,
    attempt_id: i64,
    status: &str,
) -> Result<Option<Settlement>, String> {
    let err = |e: sqlx::Error| e.to_string();
    let status: &'static str = match status {
        "completed" => "completed",
        "interrupted" => "interrupted",
        _ => "failed",
    };
    let mut tx = pool.begin().await.map_err(err)?;
    let closed = sqlx::query(
        "UPDATE harness_runs SET execution_status = ?2, ended_at = datetime('now')
          WHERE id = ?1 AND execution_status IN ('preparing', 'running')",
    )
    .bind(run_id)
    .bind(status)
    .execute(&mut *tx)
    .await
    .map_err(err)?
    .rows_affected();
    if closed == 0 {
        return Ok(None);
    }
    sqlx::query(
        "UPDATE harness_attempts SET status = ?2, ended_at = datetime('now')
          WHERE id = ?1 AND status IN ('preparing', 'running')",
    )
    .bind(attempt_id)
    .bind(status)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    sqlx::query(
        "UPDATE harness_threads SET status = ?2, updated_at = datetime('now')
          WHERE id = (SELECT thread_id FROM harness_runs WHERE id = ?1)",
    )
    .bind(run_id)
    .bind(if status == "failed" { "error" } else { "idle" })
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    let rows = sqlx::query(
        "SELECT id, kind, content, native_phase FROM harness_items WHERE attempt_id = ?1 ORDER BY id",
    )
    .bind(attempt_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?;
    let replies: Vec<_> = rows
        .iter()
        .filter(|r| r.get::<String, _>("kind") == "assistant")
        .map(|r| {
            let id: i64 = r.get("id");
            let phase: Option<String> = r.get("native_phase");
            let empty = r
                .get::<Option<String>, _>("content")
                .is_none_or(|c| c.trim().is_empty());
            (id, phase, empty)
        })
        .collect();
    let has = |kind: &str| rows.iter().any(|r| r.get::<String, _>("kind") == kind);
    let gaps: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM harness_delivery_gaps WHERE run_id = ?1 AND state = 'unresolved'",
    )
    .bind(run_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(err)?;
    let candidates: Vec<Candidate> = replies
        .iter()
        .map(|(_, phase, empty)| Candidate {
            phase: phase.as_deref(),
            empty: *empty,
        })
        .collect();
    let chosen = select(&candidates).map(|(i, why)| (replies[i].0, why));
    let answer = chosen.map(|(id, _)| id);
    let work: Vec<i64> = replies
        .iter()
        .map(|(id, ..)| *id)
        .filter(|id| Some(*id) != answer)
        .collect();
    let delivery = if replies.is_empty() && has("compaction") {
        "not_applicable"
    } else if status == "completed" && answer.is_some() && gaps == 0 {
        "delivered"
    } else {
        "incomplete"
    };
    for (id, role) in work
        .iter()
        .map(|id| (*id, "commentary"))
        .chain(answer.map(|id| (id, "answer")))
    {
        sqlx::query("UPDATE harness_items SET display_role = ?2 WHERE id = ?1")
            .bind(id)
            .bind(role)
            .execute(&mut *tx)
            .await
            .map_err(err)?;
    }
    sqlx::query(
        "UPDATE harness_runs SET answer_item_id = ?2, answer_reason = ?3, delivery_state = ?4 WHERE id = ?1",
    )
    .bind(run_id)
    .bind(answer)
    .bind(chosen.map(|(_, why)| why))
    .bind(delivery)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    // A stop, a failure or an error row already says what happened.
    let notice = if status == "completed" && delivery == "incomplete" && !has("error") {
        let text = if answer.is_some() {
            PART_MISSING
        } else {
            NO_ANSWER
        };
        let id = sqlx::query(
            "INSERT INTO harness_items (thread_id, kind, content, run_id, attempt_id)
             SELECT thread_id, 'delivery_notice', ?2, id, ?3 FROM harness_runs WHERE id = ?1",
        )
        .bind(run_id)
        .bind(text)
        .bind(attempt_id)
        .execute(&mut *tx)
        .await
        .map_err(err)?
        .last_insert_rowid();
        Some((id, text.to_string()))
    } else {
        None
    };
    sqlx::query("DELETE FROM harness_stream_drafts WHERE run_id = ?1")
        .bind(run_id)
        .execute(&mut *tx)
        .await
        .map_err(err)?;
    // A run that never left `preparing` has no start; its request time stands in.
    let (started_at, ended_at): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT COALESCE(started_at, requested_at), ended_at FROM harness_runs WHERE id = ?1",
    )
    .bind(run_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(err)?;
    tx.commit().await.map_err(err)?;
    Ok(Some(Settlement {
        answer_item_id: answer,
        work_item_ids: work,
        delivery,
        notice,
        status,
        started_at,
        ended_at,
    }))
}

/// The answer text a run has streamed and not yet committed as a row, whole
/// (`app/drafts.rs` gathers it): what a restart keeps.
pub async fn set_draft(pool: &SqlitePool, run_id: i64, text: &str) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO harness_stream_drafts (run_id, text) VALUES (?1, ?2)
         ON CONFLICT(run_id) DO UPDATE SET text = excluded.text",
    )
    .bind(run_id)
    .bind(text)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// The streamed text became a row: nothing is left to recover.
pub async fn clear_draft(pool: &SqlitePool, run_id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM harness_stream_drafts WHERE run_id = ?1")
        .bind(run_id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub const APP_CLOSED: &str = "Oculus closed before this reply finished. What arrived is kept.";

/// Startup, inside `reconcile`'s transaction: every run a dead process left
/// open keeps its streamed text as a partial answer, gets one notice row, and
/// is interrupted with delivery incomplete. Returns how many runs.
pub(super) async fn recover(tx: &mut sqlx::SqliteConnection) -> Result<u64, String> {
    let err = |e: sqlx::Error| e.to_string();
    let open = sqlx::query(
        "SELECT r.id, r.thread_id, r.active_attempt_id, d.text FROM harness_runs r
           LEFT JOIN harness_stream_drafts d ON d.run_id = r.id
          WHERE r.execution_status IN ('preparing', 'running')",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?;
    for row in &open {
        let (run, thread): (i64, i64) = (row.get("id"), row.get("thread_id"));
        let attempt: Option<i64> = row.get("active_attempt_id");
        let mut answer = None;
        if let Some(text) = row
            .get::<Option<String>, _>("text")
            .filter(|t| !t.trim().is_empty())
        {
            answer = Some(
                sqlx::query(
                    "INSERT INTO harness_items
                       (thread_id, kind, content, run_id, attempt_id, lifecycle, display_role)
                     VALUES (?1, 'assistant', ?2, ?3, ?4, 'partial', 'answer')",
                )
                .bind(thread)
                .bind(text.trim())
                .bind(run)
                .bind(attempt)
                .execute(&mut *tx)
                .await
                .map_err(err)?
                .last_insert_rowid(),
            );
        }
        sqlx::query(
            "INSERT INTO harness_items (thread_id, kind, content, run_id, attempt_id)
             VALUES (?1, 'delivery_notice', ?2, ?3, ?4)",
        )
        .bind(thread)
        .bind(APP_CLOSED)
        .bind(run)
        .bind(attempt)
        .execute(&mut *tx)
        .await
        .map_err(err)?;
        sqlx::query(
            "UPDATE harness_runs SET answer_item_id = ?2, delivery_state = 'incomplete' WHERE id = ?1",
        )
        .bind(run)
        .bind(answer)
        .execute(&mut *tx)
        .await
        .map_err(err)?;
    }
    sqlx::query("DELETE FROM harness_stream_drafts")
        .execute(&mut *tx)
        .await
        .map_err(err)?;
    Ok(open.len() as u64)
}

/// The provider named its session. A binding learns its first one; a
/// different one is a new conversation, so the run moves to a new binding.
pub async fn bind_session(pool: &SqlitePool, run_id: i64, session: &str) -> Result<(), String> {
    let err = |e: sqlx::Error| e.to_string();
    let mut tx = pool.begin().await.map_err(err)?;
    let row = sqlx::query(
        "SELECT b.id, b.thread_id, b.provider, b.native_session
           FROM harness_runs r JOIN harness_bindings b ON b.id = r.binding_id WHERE r.id = ?1",
    )
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(err)?;
    let Some(row) = row else {
        return Ok(());
    };
    match row.get::<Option<String>, _>("native_session") {
        Some(known) if known == session => {}
        None => {
            sqlx::query("UPDATE harness_bindings SET native_session = ?2 WHERE id = ?1")
                .bind(row.get::<i64, _>("id"))
                .bind(session)
                .execute(&mut *tx)
                .await
                .map_err(err)?;
        }
        Some(_) => {
            let id = sqlx::query(
                "INSERT INTO harness_bindings (thread_id, provider, native_session) VALUES (?1, ?2, ?3)",
            )
            .bind(row.get::<i64, _>("thread_id"))
            .bind(row.get::<String, _>("provider"))
            .bind(session)
            .execute(&mut *tx)
            .await
            .map_err(err)?
            .last_insert_rowid();
            sqlx::query("UPDATE harness_runs SET binding_id = ?2 WHERE id = ?1")
                .bind(run_id)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(err)?;
        }
    }
    tx.commit().await.map_err(err)
}
