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

/// What a waiting run keeps of its send: the options as the webview sent
/// them, whole, and the card a continuation belongs to, which the app sets
/// and the options never serialize.
#[derive(Serialize, Deserialize, Default)]
struct Stored {
    send: SendOptions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    continuation: Option<String>,
}

fn stored(options: &SendOptions) -> Result<String, String> {
    serde_json::to_string(&Stored {
        send: options.clone(),
        continuation: options.continuation.clone(),
    })
    .map_err(|e| e.to_string())
}

fn restored(json: Option<String>) -> SendOptions {
    let stored: Stored = json
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    SendOptions {
        continuation: stored.continuation,
        ..stored.send
    }
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
    .bind(stored(options)?)
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
        options: restored(r.get("queued_options")),
    }))
}

/// How many runs wait in the thread's queue, held or not.
pub async fn waiting(db: impl SqliteExecutor<'_>, thread_id: i64) -> Result<i64, String> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM harness_runs WHERE thread_id = ?1 AND execution_status = 'queued'",
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

/// What [`clear`] took out of the queue.
#[derive(Default, Debug)]
pub struct Cleared {
    /// The student's messages, in order, for the composer.
    pub texts: Vec<String>,
    /// Cards whose continuation was waiting.
    pub cards: Vec<String>,
}

/// Empty the thread's queue: every waiting run is cancelled.
pub async fn clear(tx: &mut SqliteConnection, thread_id: i64) -> Result<Cleared, String> {
    let err = |e: sqlx::Error| e.to_string();
    let rows = sqlx::query(
        "SELECT queued_text, queued_options FROM harness_runs
          WHERE thread_id = ?1 AND execution_status = 'queued' ORDER BY queue_position, id",
    )
    .bind(thread_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?;
    let mut cleared = Cleared::default();
    for row in &rows {
        match restored(row.get("queued_options")).continuation {
            Some(card) => cleared.cards.push(card),
            None => cleared.texts.push(
                row.get::<Option<String>, _>("queued_text")
                    .unwrap_or_default(),
            ),
        }
    }
    sqlx::query(
        "UPDATE harness_runs SET execution_status = 'cancelled', queue_position = NULL, queue_held = 0
          WHERE thread_id = ?1 AND execution_status = 'queued'",
    )
    .bind(thread_id)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    Ok(cleared)
}
