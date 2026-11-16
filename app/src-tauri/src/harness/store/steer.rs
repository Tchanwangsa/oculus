//! Messages written into a running run (docs/harness.md, "Steering the
//! running turn"). A steer's identity is a `harness_runs` row in `steered`:
//! a message that never runs, with a negative placeholder ordinal like a
//! waiting one, the send's text and options, and its `steer` row as
//! `initiating_item_id`. That row belongs to the run it was written into,
//! so the timeline shows it inside that run. A steer that misses its turn
//! flips the same run row back to `queued` at the head of the queue and
//! loses its row: one identity, one status change.
//!
//! The row's `meta` holds the message (`message`), the id the provider
//! knows it by (`client`) and how far it got (`delivery`): `sending` until
//! the effect takes it, `written` once taken, `delivered` once echoed,
//! `unknown` after a broken write, or `restart` for a correction that
//! waits for its turn to stop.

use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::harness::SendOptions;

/// The run a steer goes into, as the consumer has it open.
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub run_id: i64,
    pub attempt_id: i64,
    pub binding_id: i64,
}

/// A steer as written: its message and its row.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Written {
    pub message: i64,
    pub item_id: i64,
}

/// A steer's progress, as its row keeps it.
pub mod delivery {
    pub const SENDING: &str = "sending";
    pub const WRITTEN: &str = "written";
    pub const DELIVERED: &str = "delivered";
    pub const UNKNOWN: &str = "unknown";
    pub const RESTART: &str = "restart";
    /// Never stored: the row is gone and the message waits in the queue.
    pub const REQUEUED: &str = "requeued";
}

/// A fresh id for a steer, as a version 4 UUID: Claude takes the message's
/// `uuid` in that form, and Codex's `clientUserMessageId` takes any string.
pub fn client_id() -> String {
    use ring::rand::SecureRandom;
    let mut b = [0u8; 16];
    // The system's random source does not fail on the platforms Oculus runs.
    let _ = ring::rand::SystemRandom::new().fill(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn err(e: sqlx::Error) -> String {
    e.to_string()
}

/// Write `text` into `target`: a new message, or, with `from`, a student's
/// waiting message taken out of the queue ("Send now"). `None` when `from`
/// no longer waits, which writes nothing.
pub async fn write(
    tx: &mut SqliteConnection,
    thread_id: i64,
    target: Target,
    (text, options): (&str, &SendOptions),
    client: &str,
    from: Option<i64>,
    state: &str,
) -> Result<Option<Written>, String> {
    let message = match from {
        Some(run_id) => {
            let taken = sqlx::query(
                "UPDATE harness_runs SET execution_status = 'steered', queue_position = NULL,
                        queue_held = 0
                  WHERE id = ?1 AND thread_id = ?2 AND execution_status = 'queued'
                    AND origin = 'student'",
            )
            .bind(run_id)
            .bind(thread_id)
            .execute(&mut *tx)
            .await
            .map_err(err)?
            .rows_affected();
            if taken == 0 {
                return Ok(None);
            }
            run_id
        }
        None => sqlx::query(
            "INSERT INTO harness_runs
               (thread_id, ordinal, origin, binding_id, execution_status, queued_text,
                queued_options, delivery_state)
             VALUES (?1,
                     (SELECT MIN(0, COALESCE(MIN(ordinal), 0)) - 1 FROM harness_runs WHERE thread_id = ?1),
                     'student', ?2, 'steered', ?3, ?4, 'not_applicable')",
        )
        .bind(thread_id)
        .bind(target.binding_id)
        .bind(text)
        .bind(super::queue::stored(options)?)
        .execute(&mut *tx)
        .await
        .map_err(err)?
        .last_insert_rowid(),
    };
    let meta = serde_json::json!({ "message": message, "client": client, "delivery": state });
    let item_id = sqlx::query(
        "INSERT INTO harness_items (thread_id, kind, content, meta, run_id, attempt_id)
         VALUES (?1, 'steer', ?2, ?3, ?4, ?5)",
    )
    .bind(thread_id)
    .bind(text)
    .bind(meta.to_string())
    .bind(target.run_id)
    .bind(target.attempt_id)
    .execute(&mut *tx)
    .await
    .map_err(err)?
    .last_insert_rowid();
    sqlx::query(
        "UPDATE harness_runs SET initiating_item_id = ?2, queued_text = ?3,
                delivery_state = 'not_applicable'
          WHERE id = ?1",
    )
    .bind(message)
    .bind(item_id)
    .bind(text)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    Ok(Some(Written { message, item_id }))
}

/// Move a steer's row from one state to the next. False when it is not in
/// `from` (any state, for `None`), or no longer there.
pub async fn set(
    tx: &mut SqliteConnection,
    item_id: i64,
    from: Option<&str>,
    to: &str,
) -> Result<bool, String> {
    sqlx::query(
        "UPDATE harness_items SET meta = json_set(meta, '$.delivery', ?2)
          WHERE id = ?1 AND kind = 'steer'
            AND (?3 IS NULL OR json_extract(meta, '$.delivery') = ?3)",
    )
    .bind(item_id)
    .bind(to)
    .bind(from)
    .execute(&mut *tx)
    .await
    .map(|r| r.rows_affected() == 1)
    .map_err(err)
}

/// The steer's effect takes it for its write: `sending` becomes `written`,
/// once, and only while its run is still running and nobody asked it to
/// stop, with the thread's raised revision. False means the write must not
/// be made. Saved before the write, so a restart never sends it again.
pub async fn claim(pool: &SqlitePool, thread_id: i64, item_id: i64) -> Result<bool, String> {
    let mut tx = pool.begin().await.map_err(err)?;
    let claimed = sqlx::query(
        "UPDATE harness_items SET meta = json_set(meta, '$.delivery', 'written')
          WHERE id = ?1 AND kind = 'steer' AND json_extract(meta, '$.delivery') = 'sending'
            AND EXISTS (SELECT 1 FROM harness_runs r WHERE r.id = harness_items.run_id
                          AND r.execution_status = 'running' AND r.stop_requested_at IS NULL)",
    )
    .bind(item_id)
    .execute(&mut *tx)
    .await
    .map_err(err)?
    .rows_affected()
        == 1;
    if claimed {
        super::threads::raise_revision(&mut tx, thread_id).await?;
    }
    tx.commit().await.map_err(err)?;
    Ok(claimed)
}

/// The steer the provider knows as `client` in this thread: its row and
/// its message.
pub async fn by_client(
    pool: &SqlitePool,
    thread_id: i64,
    client: &str,
) -> Result<Option<Written>, String> {
    let row = sqlx::query(
        "SELECT id, json_extract(meta, '$.message') AS message FROM harness_items
          WHERE thread_id = ?1 AND kind = 'steer' AND json_extract(meta, '$.client') = ?2",
    )
    .bind(thread_id)
    .bind(client)
    .fetch_optional(pool)
    .await
    .map_err(err)?;
    Ok(row.map(|r| Written {
        message: r.get("message"),
        item_id: r.get("id"),
    }))
}

/// How far a steer got, as its row says.
pub async fn delivery_of(pool: &SqlitePool, item_id: i64) -> Result<Option<String>, String> {
    sqlx::query_scalar(
        "SELECT json_extract(meta, '$.delivery') FROM harness_items WHERE id = ?1 AND kind = 'steer'",
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await
    .map_err(err)
}

/// The message a steer row stands for.
pub async fn message_of(pool: &SqlitePool, item_id: i64) -> Result<Option<i64>, String> {
    sqlx::query_scalar(
        "SELECT json_extract(meta, '$.message') FROM harness_items WHERE id = ?1 AND kind = 'steer'",
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await
    .map_err(err)
}

/// A steer that missed its turn goes back to being a message of its own:
/// its run row waits first in the queue, and its row leaves the run it
/// never reached. It is held when `held` says so, or, with `None`, when
/// the run it was aimed at was stopped or failed, as Stop and a failure
/// hold what waits. The message's id, or `None` when the steer was
/// already delivered or is gone.
pub async fn requeue(
    tx: &mut SqliteConnection,
    thread_id: i64,
    item_id: i64,
    held: Option<bool>,
) -> Result<Option<i64>, String> {
    let found = sqlx::query(
        "SELECT r.id, t.stop_requested_at IS NOT NULL OR t.execution_status = 'failed' AS held
           FROM harness_runs r JOIN harness_items i ON i.id = r.initiating_item_id
           LEFT JOIN harness_runs t ON t.id = i.run_id
          WHERE i.id = ?1 AND i.thread_id = ?2 AND r.execution_status = 'steered'
            AND json_extract(i.meta, '$.delivery') IN ('sending', 'written', 'restart')",
    )
    .bind(item_id)
    .bind(thread_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(err)?;
    let Some(found) = found else {
        return Ok(None);
    };
    let message: i64 = found.get("id");
    let held = held.unwrap_or(found.get::<Option<i64>, _>("held").unwrap_or(0) != 0);
    sqlx::query(
        "UPDATE harness_runs
            SET execution_status = 'queued', initiating_item_id = NULL, queue_held = ?3,
                queue_position = (SELECT COALESCE(MIN(q.queue_position), 1) - 1 FROM harness_runs q
                                   WHERE q.thread_id = ?2 AND q.execution_status = 'queued')
          WHERE id = ?1",
    )
    .bind(message)
    .bind(thread_id)
    .bind(held)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    sqlx::query("DELETE FROM harness_items WHERE id = ?1")
        .bind(item_id)
        .execute(&mut *tx)
        .await
        .map_err(err)?;
    Ok(Some(message))
}

/// [`requeue`] as a transaction of its own, with the thread's raised
/// revision: what the consumer does when a steer's provider says it came
/// too late or would not take it.
pub async fn missed(
    pool: &SqlitePool,
    thread_id: i64,
    item_id: i64,
) -> Result<Option<(i64, i64)>, String> {
    let mut tx = pool.begin().await.map_err(err)?;
    let Some(message) = requeue(&mut tx, thread_id, item_id, None).await? else {
        return Ok(None);
    };
    let revision = super::threads::raise_revision(&mut tx, thread_id).await?;
    tx.commit().await.map_err(err)?;
    Ok(Some((message, revision)))
}

/// Move a steer to `to` from any state but `delivered`, with the thread's
/// raised revision: the provider's echo, or a write that broke partway.
/// The steer and the revision, or `None` when it was already delivered or
/// is gone.
pub async fn update(
    pool: &SqlitePool,
    thread_id: i64,
    item_id: i64,
    to: &str,
) -> Result<Option<(Written, i64)>, String> {
    let mut tx = pool.begin().await.map_err(err)?;
    let message: Option<i64> = sqlx::query_scalar(
        "UPDATE harness_items SET meta = json_set(meta, '$.delivery', ?3)
          WHERE id = ?1 AND thread_id = ?2 AND kind = 'steer'
            AND json_extract(meta, '$.delivery') NOT IN ('delivered', ?3)
          RETURNING json_extract(meta, '$.message')",
    )
    .bind(item_id)
    .bind(thread_id)
    .bind(to)
    .fetch_optional(&mut *tx)
    .await
    .map_err(err)?;
    let Some(message) = message else {
        return Ok(None);
    };
    let revision = super::threads::raise_revision(&mut tx, thread_id).await?;
    tx.commit().await.map_err(err)?;
    Ok(Some((Written { message, item_id }, revision)))
}

/// The corrections a restart steer left for `run_id`: rows and text, in
/// the order they were sent.
pub async fn restarts(
    conn: &mut SqliteConnection,
    run_id: i64,
) -> Result<Vec<(i64, String)>, String> {
    let rows = sqlx::query(
        "SELECT id, content FROM harness_items
          WHERE run_id = ?1 AND kind = 'steer' AND json_extract(meta, '$.delivery') = 'restart'
          ORDER BY id",
    )
    .bind(run_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(err)?;
    Ok(rows
        .iter()
        .map(|r| {
            let text: Option<String> = r.get("content");
            (r.get("id"), text.unwrap_or_default())
        })
        .collect())
}

/// What a run's terminal did with the restart steers waiting on it.
#[derive(Debug, PartialEq)]
pub enum Restart {
    /// None waited: the terminal settles the run.
    None,
    /// The run goes on as `attempt`; `work` are the superseded attempt's
    /// replies, now work, and `delivered` the corrections it carries.
    Restarted {
        attempt: i64,
        revision: i64,
        work: Vec<i64>,
        delivered: Vec<i64>,
    },
    /// The student stopped the run meanwhile: the corrections wait first in
    /// the queue, held, and the terminal settles the run.
    Requeued { revision: i64 },
}

/// A run's turn ended with restart steers waiting on it: its attempt is
/// superseded, what it said stays as work and never as the answer, and a
/// new attempt (`steering_restart`) takes the run back to `starting` with
/// the corrections as what its start step sends. One transaction with the
/// thread's raised revision.
pub async fn restart(
    pool: &SqlitePool,
    thread_id: i64,
    run_id: i64,
    superseded: i64,
    status: &str,
) -> Result<Restart, String> {
    let mut tx = pool.begin().await.map_err(err)?;
    let corrections = restarts(&mut tx, run_id).await?;
    if corrections.is_empty() {
        return Ok(Restart::None);
    }
    let stopped: Option<bool> = sqlx::query_scalar(
        "SELECT stop_requested_at IS NOT NULL FROM harness_runs
          WHERE id = ?1 AND execution_status = 'running'",
    )
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(err)?;
    match stopped {
        None => return Ok(Restart::None),
        Some(true) => {
            for (item_id, _) in corrections.iter().rev() {
                requeue(&mut tx, thread_id, *item_id, Some(true)).await?;
            }
            let revision = super::threads::raise_revision(&mut tx, thread_id).await?;
            tx.commit().await.map_err(err)?;
            return Ok(Restart::Requeued { revision });
        }
        Some(false) => {}
    }
    let text = corrections
        .iter()
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    sqlx::query(
        "UPDATE harness_attempts SET status = 'superseded', ended_at = datetime('now')
          WHERE id = ?1",
    )
    .bind(superseded)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    let attempt = sqlx::query(
        "INSERT INTO harness_attempts (run_id, ordinal, reason, status)
         VALUES (?1, (SELECT COALESCE(MAX(ordinal), 0) + 1 FROM harness_attempts WHERE run_id = ?1),
                 'steering_restart', 'starting')",
    )
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(err)?
    .last_insert_rowid();
    sqlx::query(
        "UPDATE harness_runs SET active_attempt_id = ?2, execution_status = 'starting',
                queued_text = ?3
          WHERE id = ?1",
    )
    .bind(run_id)
    .bind(attempt)
    .bind(&text)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    // What the superseded attempt said is kept, folded as work: never the
    // answer, however the provider ended it (`status`).
    let work: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM harness_items WHERE attempt_id = ?1 AND kind = 'assistant' ORDER BY id",
    )
    .bind(superseded)
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?;
    sqlx::query(
        "UPDATE harness_items SET display_role = 'commentary',
                lifecycle = CASE WHEN ?2 = 'completed' THEN lifecycle ELSE 'partial' END
          WHERE attempt_id = ?1 AND kind = 'assistant'",
    )
    .bind(superseded)
    .bind(status)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    for (item_id, _) in &corrections {
        set(
            &mut tx,
            *item_id,
            Some(delivery::RESTART),
            delivery::DELIVERED,
        )
        .await?;
    }
    sqlx::query("DELETE FROM harness_stream_drafts WHERE run_id = ?1")
        .bind(run_id)
        .execute(&mut *tx)
        .await
        .map_err(err)?;
    let revision = super::threads::raise_revision(&mut tx, thread_id).await?;
    tx.commit().await.map_err(err)?;
    let delivered = corrections.iter().map(|(id, _)| *id).collect();
    Ok(Restart::Restarted {
        attempt,
        revision,
        work,
        delivered,
    })
}

/// Startup, inside `reconcile`'s transaction: a steer saved and never
/// handed to its provider goes back to the head of its queue, held, as an
/// unsent start does; one whose write was made but never echoed stays where
/// it is as `unknown`: it may have reached the turn, so it is never resent. A run a
/// restart steer had taken back to `starting` is running for the repair
/// that follows, which ends it interrupted with what it said.
pub(super) async fn reconcile(tx: &mut SqliteConnection) -> Result<(), String> {
    let unsent = sqlx::query(
        "SELECT id, thread_id FROM harness_items
          WHERE kind = 'steer' AND json_extract(meta, '$.delivery') IN ('sending', 'restart')
          ORDER BY id DESC",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?;
    for row in unsent {
        requeue(&mut *tx, row.get("thread_id"), row.get("id"), Some(true)).await?;
    }
    for sql in [
        "UPDATE harness_items SET meta = json_set(meta, '$.delivery', 'unknown')
          WHERE kind = 'steer' AND json_extract(meta, '$.delivery') = 'written'",
        "UPDATE harness_attempts SET status = 'running'
          WHERE status = 'starting' AND ordinal > 1",
        "UPDATE harness_runs SET execution_status = 'running'
          WHERE execution_status = 'starting'
            AND EXISTS (SELECT 1 FROM harness_attempts a
                         WHERE a.id = harness_runs.active_attempt_id AND a.ordinal > 1)",
    ] {
        sqlx::query(sql).execute(&mut *tx).await.map_err(err)?;
    }
    Ok(())
}
