//! Runs, attempts, provider bindings and send receipts: the app's own identity
//! for each accepted send, written before the provider hears of it
//! (docs/harness.md, "Runs").

use std::sync::atomic::{AtomicU64, Ordering};

use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::harness::event::answer::{select, Candidate};
use crate::harness::event::Provider;

/// What [`begin`] wrote: the run, its first attempt, the binding it runs on
/// and the question's row.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// The reasoning level it is sent under; `None` where the model has none.
    pub effort: Option<String>,
    pub receipt: String,
    pub text: String,
    pub at: Option<i64>,
    /// The card whose follow-up this run is (`app/continuation.rs`): its row is
    /// a `continuation` row holding `text`, never a question, and the run's
    /// origin is `continuation`. `None` for a student send.
    pub continuation: Option<String>,
    /// The waiting run this send starts (`queue.rs`): its row becomes the
    /// run, instead of a new one. `None` for a send that never waited.
    pub queued: Option<i64>,
    /// The send's options as the run keeps them (`queue::stored`), which is
    /// what its start step reads; `None` keeps what a waiting run holds.
    pub options: Option<String>,
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
pub async fn begin(tx: &mut SqliteConnection, send: &SendIntent) -> Result<RunStart, String> {
    let err = |e: sqlx::Error| e.to_string();
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
        if !super::super::app::continuation::start(&mut *tx, send.thread_id, card).await? {
            return Err(NOT_CLAIMED.into());
        }
    }
    // The provider's one current session in this chat is what it resumes.
    let binding_id = super::bindings::current(&mut *tx, send.thread_id, send.provider)
        .await?
        .id;
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
    // The ordinal is the order turns ran in: a run that waited takes the
    // next one now, not when it was queued.
    const NEXT_ORDINAL: &str =
        "(SELECT COALESCE(MAX(ordinal), 0) + 1 FROM harness_runs WHERE thread_id = ?1 AND ordinal > 0)";
    let run_id = match send.queued {
        Some(run_id) => {
            let started = sqlx::query(&format!(
                "UPDATE harness_runs
                    SET ordinal = {NEXT_ORDINAL}, initiating_item_id = ?2, binding_id = ?3, model = ?4,
                        effort = ?8, execution_status = 'starting', queue_position = NULL,
                        queue_held = 0, queued_text = ?6,
                        queued_options = COALESCE(?7, queued_options)
                  WHERE id = ?5 AND thread_id = ?1 AND execution_status = 'queued'"
            ))
            .bind(send.thread_id)
            .bind(item_id)
            .bind(binding_id)
            .bind(&send.model)
            .bind(run_id)
            .bind(&send.text)
            .bind(&send.options)
            .bind(&send.effort)
            .execute(&mut *tx)
            .await
            .map_err(err)?
            .rows_affected();
            if started == 0 {
                return Err(format!("run {run_id} is no longer waiting"));
            }
            run_id
        }
        None => sqlx::query(&format!(
            "INSERT INTO harness_runs
               (thread_id, ordinal, origin, initiating_item_id, binding_id, model, execution_status,
                queued_text, queued_options, effort)
             VALUES (?1, {NEXT_ORDINAL}, ?5, ?2, ?3, ?4, 'starting', ?6, ?7, ?8)"
        ))
        .bind(send.thread_id)
        .bind(item_id)
        .bind(binding_id)
        .bind(&send.model)
        .bind(origin)
        .bind(&send.text)
        .bind(&send.options)
        .bind(&send.effort)
        .execute(&mut *tx)
        .await
        .map_err(err)?
        .last_insert_rowid(),
    };
    if let Some(card) = &send.continuation {
        super::super::app::continuation::started(&mut *tx, card, run_id).await?;
    }
    let attempt_id = sqlx::query(
        "INSERT INTO harness_attempts (run_id, ordinal, reason, status) VALUES (?1, 1, 'initial', 'starting')",
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
    Ok(RunStart {
        run_id,
        attempt_id,
        binding_id,
        item_id,
    })
}

/// [`begin`] as a transaction of its own.
#[cfg(test)]
pub async fn begin_run(pool: &SqlitePool, send: &SendIntent) -> Result<RunStart, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let run = begin(&mut tx, send).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(run)
}

/// What the thread's newest run that started was sent with: the provider,
/// model and reasoning level a card's continuation carries on under.
pub async fn last_selection(
    tx: &mut SqliteConnection,
    thread_id: i64,
) -> Result<Option<(Provider, Option<String>, Option<String>)>, String> {
    let row = sqlx::query(
        "SELECT b.provider, r.model, r.effort FROM harness_runs r
           JOIN harness_bindings b ON b.id = r.binding_id
          WHERE r.thread_id = ?1 AND r.ordinal > 0 ORDER BY r.ordinal DESC LIMIT 1",
    )
    .bind(thread_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(row.and_then(|row| {
        let provider = Provider::parse(&row.get::<String, _>("provider"))?;
        Some((provider, row.get("model"), row.get("effort")))
    }))
}

/// The start step takes the run for its provider call: `starting` becomes
/// `running`, once, and only while nobody asked it to stop. False means
/// the call must not be made: the run was stopped, settled or deleted in
/// the meantime, or another start already has it.
pub async fn claim_start(pool: &SqlitePool, run: &RunStart) -> Result<bool, String> {
    let claimed = sqlx::query(
        "UPDATE harness_runs SET execution_status = 'running'
          WHERE id = ?1 AND execution_status = 'starting' AND stop_requested_at IS NULL",
    )
    .bind(run.run_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();
    Ok(claimed == 1)
}

/// The provider accepted the send: the run's clock starts. Conditional,
/// since the turn may already have finished on the consumer by the time
/// the write returns.
pub async fn mark_sent(pool: &SqlitePool, run: &RunStart) -> Result<(), String> {
    for (sql, id) in [
        (
            "UPDATE harness_runs SET started_at = datetime('now')
              WHERE id = ?1 AND execution_status = 'running' AND started_at IS NULL",
            run.run_id,
        ),
        (
            "UPDATE harness_attempts SET status = 'running', started_at = datetime('now')
              WHERE id = ?1 AND status IN ('preparing', 'starting')",
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
    /// The thread's revision with the settlement written.
    pub revision: i64,
    /// The run failed, which held the thread's queue; the cards whose
    /// waiting continuation that cancelled.
    pub held: Option<Vec<String>>,
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
          WHERE id = ?1 AND execution_status IN ('preparing', 'starting', 'running')",
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
          WHERE id = ?1 AND status IN ('preparing', 'starting', 'running')",
    )
    .bind(attempt_id)
    .bind(status)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    let thread_id: i64 = sqlx::query_scalar("SELECT thread_id FROM harness_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    sqlx::query(
        "UPDATE harness_threads SET status = ?2, updated_at = datetime('now') WHERE id = ?1",
    )
    .bind(thread_id)
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
    // A run the provider never accepted has no start; its request time stands in.
    let (started_at, ended_at): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT COALESCE(started_at, requested_at), ended_at FROM harness_runs WHERE id = ?1",
    )
    .bind(run_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(err)?;
    // A failing provider does not burn through what waits behind it.
    let held = match status {
        "failed" => {
            let reason = crate::harness::app::continuation::FAILED_BEFORE;
            Some(crate::harness::app::continuation::hold_queue(&mut tx, thread_id, reason).await?)
        }
        _ => None,
    };
    let revision = super::threads::raise_revision(&mut tx, thread_id).await?;
    tx.commit().await.map_err(err)?;
    Ok(Some(Settlement {
        revision,
        held,
        answer_item_id: answer,
        work_item_ids: work,
        delivery,
        notice,
        status,
        started_at,
        ended_at,
    }))
}

/// Write the answer text a run has streamed and not yet committed as a row,
/// whole (`app/drafts.rs` gathers it): what a restart keeps and a reload
/// shows. Empty text removes the draft: everything streamed became rows.
/// The thread's revision is raised with it, so an envelope told before the
/// write is known to be inside the draft a later snapshot reads.
pub(super) async fn write_draft(
    tx: &mut SqliteConnection,
    run_id: i64,
    text: &str,
) -> Result<(), String> {
    let written = match text.is_empty() {
        true => sqlx::query("DELETE FROM harness_stream_drafts WHERE run_id = ?1").bind(run_id),
        false => sqlx::query(
            "INSERT INTO harness_stream_drafts (run_id, text) VALUES (?1, ?2)
             ON CONFLICT(run_id) DO UPDATE SET text = excluded.text",
        )
        .bind(run_id)
        .bind(text),
    };
    written
        .execute(&mut *tx)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// [`write_draft`] as a transaction of its own. Returns the run's thread
/// and its raised revision; `None` when the run is gone.
pub async fn set_draft(
    pool: &SqlitePool,
    run_id: i64,
    text: &str,
) -> Result<Option<(i64, i64)>, String> {
    let err = |e: sqlx::Error| e.to_string();
    let mut tx = pool.begin().await.map_err(err)?;
    let thread: Option<i64> =
        sqlx::query_scalar("SELECT thread_id FROM harness_runs WHERE id = ?1")
            .bind(run_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(err)?;
    let Some(thread_id) = thread else {
        return Ok(None);
    };
    write_draft(&mut tx, run_id, text).await?;
    let revision = super::threads::raise_revision(&mut tx, thread_id).await?;
    tx.commit().await.map_err(err)?;
    Ok(Some((thread_id, revision)))
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

/// [`bindings::bind_session`](super::bindings::bind_session) as a
/// transaction of its own.
pub async fn bind_session(pool: &SqlitePool, run_id: i64, session: &str) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    super::bindings::bind_session(&mut tx, run_id, session).await?;
    tx.commit().await.map_err(|e| e.to_string())
}
