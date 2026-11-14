//! Messages that wait their turn. Each is a `harness_runs` row in `queued`
//! from the moment it is accepted, so it has one identity through the
//! queue, a restart and its answer (docs/harness.md, "One turn at a time").
//! It has no timeline row and no real ordinal until it starts: both are
//! taken then (`runs::begin`), since rows and ordinals are the order turns
//! ran in, not the order messages were typed.

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection, SqliteExecutor};

use crate::harness::event::Provider;
use crate::harness::SendOptions;

use super::runs::RunStart;

/// A student's waiting message, as the composer lists it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueuedMessage {
    /// Its run's id; a string, as the webview keys its list.
    pub id: String,
    pub text: String,
    /// The queue is held: nothing goes out until the student resumes it.
    pub held: bool,
}

/// A waiting run with what it will be sent as.
pub struct Waiting {
    pub run_id: i64,
    pub text: String,
    pub options: SendOptions,
}

/// What a run keeps of its send: the options as the webview sent them,
/// whole, the card a continuation belongs to (which the app sets and the
/// options never serialize), and whether the send changed the chat's model.
#[derive(Serialize, Deserialize, Default)]
struct Stored {
    send: SendOptions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    continuation: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    respawn: bool,
}

/// `options` as a run stores them. `respawn` is set once the run starts:
/// its send changed the chat's model, which a CLI that fixes its model at
/// spawn can only take as a new session.
pub fn stored(options: &SendOptions, respawn: bool) -> Result<String, String> {
    serde_json::to_string(&Stored {
        send: options.clone(),
        continuation: options.continuation.clone(),
        respawn,
    })
    .map_err(|e| e.to_string())
}

fn restored(json: Option<String>) -> (SendOptions, bool) {
    let stored: Stored = json
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    let options = SendOptions {
        continuation: stored.continuation,
        ..stored.send
    };
    (options, stored.respawn)
}

/// Add a message to the back of the thread's queue. Its ordinal is a
/// negative placeholder, unique in the thread, until the run starts.
pub async fn enqueue(
    tx: &mut SqliteConnection,
    thread_id: i64,
    provider: Provider,
    text: &str,
    options: &SendOptions,
) -> Result<i64, String> {
    let err = |e: sqlx::Error| e.to_string();
    let binding_id = super::runs::binding(&mut *tx, thread_id, provider).await?;
    let origin = match options.continuation {
        Some(_) => "continuation",
        None => "student",
    };
    let run_id = sqlx::query(
        "INSERT INTO harness_runs
           (thread_id, ordinal, origin, binding_id, execution_status, queue_position, queued_text, queued_options)
         VALUES (?1,
                 (SELECT MIN(0, COALESCE(MIN(ordinal), 0)) - 1 FROM harness_runs WHERE thread_id = ?1),
                 ?2, ?3, 'queued',
                 (SELECT COALESCE(MAX(queue_position), 0) + 1 FROM harness_runs
                   WHERE thread_id = ?1 AND execution_status = 'queued'),
                 ?4, ?5)",
    )
    .bind(thread_id)
    .bind(origin)
    .bind(binding_id)
    .bind(text)
    .bind(stored(options, false)?)
    .execute(&mut *tx)
    .await
    .map_err(err)?
    .last_insert_rowid();
    Ok(run_id)
}

/// The student's waiting messages in queue order. A card's continuation
/// waits in the same queue and is never listed: it is not theirs to edit.
pub async fn list(
    db: impl SqliteExecutor<'_>,
    thread_id: i64,
) -> Result<Vec<QueuedMessage>, String> {
    let rows = sqlx::query(
        "SELECT id, queued_text, queue_held FROM harness_runs
          WHERE thread_id = ?1 AND execution_status = 'queued' AND origin = 'student'
          ORDER BY queue_position, id",
    )
    .bind(thread_id)
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .map(|r| QueuedMessage {
            id: r.get::<i64, _>("id").to_string(),
            text: r
                .get::<Option<String>, _>("queued_text")
                .unwrap_or_default(),
            held: r.get::<i64, _>("queue_held") != 0,
        })
        .collect())
}

/// The run that goes out next: the first in the queue that is not held.
pub async fn next(db: impl SqliteExecutor<'_>, thread_id: i64) -> Result<Option<Waiting>, String> {
    let row = sqlx::query(
        "SELECT id, queued_text, queued_options FROM harness_runs
          WHERE thread_id = ?1 AND execution_status = 'queued' AND queue_held = 0
          ORDER BY queue_position, id LIMIT 1",
    )
    .bind(thread_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(row.map(|r| Waiting {
        run_id: r.get("id"),
        text: r
            .get::<Option<String>, _>("queued_text")
            .unwrap_or_default(),
        options: restored(r.get("queued_options")).0,
    }))
}

/// The thread's open run: the one going out or being answered.
pub struct Open {
    pub run: RunStart,
    /// `starting` until the start step has taken it for its provider call.
    pub starting: bool,
    /// The student asked it to stop (`stop_requested_at`).
    pub stop_requested: bool,
    /// The row's text: the student's words, or a continuation's line.
    pub text: String,
    pub options: SendOptions,
    pub respawn: bool,
}

/// A thread runs one turn at a time, so at most one run is open.
pub async fn open(db: impl SqliteExecutor<'_>, thread_id: i64) -> Result<Option<Open>, String> {
    let row = sqlx::query(
        "SELECT id, active_attempt_id, binding_id, initiating_item_id, execution_status,
                queued_text, queued_options, stop_requested_at IS NOT NULL AS stop_requested
           FROM harness_runs
          WHERE thread_id = ?1 AND execution_status IN ('preparing', 'starting', 'running')
          ORDER BY id DESC LIMIT 1",
    )
    .bind(thread_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(row.map(|r| {
        let (options, respawn) = restored(r.get("queued_options"));
        Open {
            run: RunStart {
                run_id: r.get("id"),
                attempt_id: r
                    .get::<Option<i64>, _>("active_attempt_id")
                    .unwrap_or_default(),
                binding_id: r.get("binding_id"),
                item_id: r
                    .get::<Option<i64>, _>("initiating_item_id")
                    .unwrap_or_default(),
            },
            starting: r.get::<String, _>("execution_status") == "starting",
            stop_requested: r.get::<i64, _>("stop_requested") != 0,
            text: r
                .get::<Option<String>, _>("queued_text")
                .unwrap_or_default(),
            options,
            respawn,
        }
    }))
}

/// How many runs wait in the thread's queue, and how many of them are held.
pub async fn waiting(db: impl SqliteExecutor<'_>, thread_id: i64) -> Result<(i64, i64), String> {
    sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(queue_held != 0), 0) FROM harness_runs
          WHERE thread_id = ?1 AND execution_status = 'queued'",
    )
    .bind(thread_id)
    .fetch_one(db)
    .await
    .map_err(|e| e.to_string())
}

/// Rewrite a student's waiting message. False when it no longer waits.
pub async fn edit(
    tx: &mut SqliteConnection,
    thread_id: i64,
    run_id: i64,
    text: &str,
) -> Result<bool, String> {
    sqlx::query(
        "UPDATE harness_runs SET queued_text = ?3
          WHERE id = ?1 AND thread_id = ?2 AND execution_status = 'queued' AND origin = 'student'",
    )
    .bind(run_id)
    .bind(thread_id)
    .bind(text)
    .execute(&mut *tx)
    .await
    .map(|r| r.rows_affected() == 1)
    .map_err(|e| e.to_string())
}

/// A waiting run leaves the queue as `status` (`cancelled`, or `failed`
/// for one that could not be started). It never ran, so it gets no end
/// time: the thread did not settle. False when it no longer waits.
pub async fn close(
    tx: &mut SqliteConnection,
    thread_id: i64,
    run_id: i64,
    status: &str,
) -> Result<bool, String> {
    sqlx::query(
        "UPDATE harness_runs SET execution_status = ?3, queue_position = NULL, queue_held = 0
          WHERE id = ?1 AND thread_id = ?2 AND execution_status = 'queued'",
    )
    .bind(run_id)
    .bind(thread_id)
    .bind(status)
    .execute(&mut *tx)
    .await
    .map(|r| r.rows_affected() == 1)
    .map_err(|e| e.to_string())
}

/// Put the student's waiting messages in `order`, first to go out first.
/// Ids that no longer wait are ignored; messages `order` leaves out keep
/// their relative order behind the ones it names.
pub async fn reorder(
    tx: &mut SqliteConnection,
    thread_id: i64,
    order: &[i64],
) -> Result<(), String> {
    let err = |e: sqlx::Error| e.to_string();
    let queued: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM harness_runs WHERE thread_id = ?1 AND execution_status = 'queued'
          ORDER BY queue_position, id",
    )
    .bind(thread_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?;
    let named = order.iter().filter(|id| queued.contains(id));
    let rest = queued.iter().filter(|id| !order.contains(id));
    for (position, run_id) in named.chain(rest).enumerate() {
        sqlx::query("UPDATE harness_runs SET queue_position = ?2 WHERE id = ?1")
            .bind(run_id)
            .bind(position as i64 + 1)
            .execute(&mut *tx)
            .await
            .map_err(err)?;
    }
    Ok(())
}

/// Lift the hold on the thread's queue.
pub async fn unhold(tx: &mut SqliteConnection, thread_id: i64) -> Result<(), String> {
    sqlx::query(
        "UPDATE harness_runs SET queue_held = 0
          WHERE thread_id = ?1 AND execution_status = 'queued' AND queue_held != 0",
    )
    .bind(thread_id)
    .execute(&mut *tx)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// How many other threads of `provider` have a run going out or being
/// answered: the chats a force stop of a shared server would cut too.
pub async fn others_open(
    db: impl SqliteExecutor<'_>,
    provider: Provider,
    thread_id: i64,
) -> Result<i64, String> {
    sqlx::query_scalar(
        "SELECT COUNT(DISTINCT r.thread_id) FROM harness_runs r
           JOIN harness_threads t ON t.id = r.thread_id
          WHERE t.provider = ?1 AND r.thread_id != ?2
            AND r.execution_status IN ('preparing', 'starting', 'running')",
    )
    .bind(provider.as_str())
    .bind(thread_id)
    .fetch_one(db)
    .await
    .map_err(|e| e.to_string())
}

/// The student asked this run to stop: a saved fact every later decision
/// reads. False when the run is not open any more, which changes nothing.
pub async fn request_stop(
    tx: &mut SqliteConnection,
    thread_id: i64,
    run_id: i64,
) -> Result<bool, String> {
    sqlx::query(
        "UPDATE harness_runs SET stop_requested_at = COALESCE(stop_requested_at, datetime('now'))
          WHERE id = ?1 AND thread_id = ?2
            AND execution_status IN ('preparing', 'starting', 'running')",
    )
    .bind(run_id)
    .bind(thread_id)
    .execute(&mut *tx)
    .await
    .map(|r| r.rows_affected() == 1)
    .map_err(|e| e.to_string())
}

/// Hold the thread's queue: the student's waiting messages stay, in order,
/// and none goes out until the hold is lifted. A card's continuation is
/// never held, since the student could not see it wait: its run is
/// cancelled, and the cards whose claim was still open are returned for
/// `reason` to be written on them (`continuation::cancel`).
pub async fn hold(tx: &mut SqliteConnection, thread_id: i64) -> Result<Vec<String>, String> {
    let err = |e: sqlx::Error| e.to_string();
    let rows = sqlx::query(
        "SELECT queued_options FROM harness_runs
          WHERE thread_id = ?1 AND execution_status = 'queued' AND origin = 'continuation'
          ORDER BY queue_position, id",
    )
    .bind(thread_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?;
    let cards = rows
        .iter()
        .filter_map(|row| restored(row.get("queued_options")).0.continuation)
        .collect();
    for sql in [
        "UPDATE harness_runs SET execution_status = 'cancelled', queue_position = NULL, queue_held = 0
          WHERE thread_id = ?1 AND execution_status = 'queued' AND origin = 'continuation'",
        "UPDATE harness_runs SET queue_held = 1
          WHERE thread_id = ?1 AND execution_status = 'queued'",
    ] {
        sqlx::query(sql)
            .bind(thread_id)
            .execute(&mut *tx)
            .await
            .map_err(err)?;
    }
    Ok(cards)
}

/// Startup: a run still `starting` was written and never handed to its
/// provider, so nothing of it was sent. A student's message goes back to
/// the head of the queue, held, without its question row (it takes a new
/// one when it starts); a continuation's click belongs to the session that
/// is over, so its run is cancelled and its card says the app closed.
pub(super) async fn requeue_unsent(tx: &mut SqliteConnection, skipped: &str) -> Result<(), String> {
    let err = |e: sqlx::Error| e.to_string();
    const UNSENT: &str = "SELECT id FROM harness_runs WHERE execution_status = 'starting'";
    sqlx::query(&format!(
        "UPDATE chat_proposals
            SET state = json_set(state, '$.continuation', json_object('phase', 'skipped', 'reason', ?1))
          WHERE json_extract(state, '$.continuation.phase') = 'started'
            AND json_extract(state, '$.continuation.run_id') IN ({UNSENT} AND origin = 'continuation')"
    ))
    .bind(skipped)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    for sql in [
        "DELETE FROM harness_items WHERE id IN
           (SELECT initiating_item_id FROM harness_runs WHERE execution_status = 'starting')",
        "DELETE FROM harness_attempts WHERE run_id IN
           (SELECT id FROM harness_runs WHERE execution_status = 'starting')",
        "DELETE FROM harness_command_receipts WHERE run_id IN
           (SELECT id FROM harness_runs WHERE execution_status = 'starting')",
        "UPDATE harness_runs SET execution_status = 'cancelled', initiating_item_id = NULL, active_attempt_id = NULL
          WHERE execution_status = 'starting' AND origin = 'continuation'",
    ] {
        sqlx::query(sql).execute(&mut *tx).await.map_err(err)?;
    }
    let unsent: Vec<i64> = sqlx::query_scalar(&format!("{UNSENT} ORDER BY id DESC"))
        .fetch_all(&mut *tx)
        .await
        .map_err(err)?;
    // One at a time: each takes the placeholder ordinal and the head of the
    // queue as the rows stand after the one before it.
    for run_id in unsent {
        sqlx::query(
            "UPDATE harness_runs
                SET execution_status = 'queued', queue_held = 1, initiating_item_id = NULL,
                    active_attempt_id = NULL,
                    ordinal = (SELECT MIN(0, COALESCE(MIN(o.ordinal), 0)) - 1 FROM harness_runs o
                                WHERE o.thread_id = harness_runs.thread_id),
                    queue_position = (SELECT COALESCE(MIN(q.queue_position), 1) - 1 FROM harness_runs q
                                       WHERE q.thread_id = harness_runs.thread_id
                                         AND q.execution_status = 'queued')
              WHERE id = ?1",
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await
        .map_err(err)?;
    }
    Ok(())
}
