//! `harness_requests` (migration 58). Every write raises the thread's
//! revision in its own transaction, as a row change does, so a reload that
//! reads a request reads the revision that covers it.

use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::harness::store::{raise_revision, Owner};
use crate::harness::{Generation, Provider};

use super::{reasons, Delivery, NewRequest, RequestView, Status};

const COLUMNS: &str = "id, thread_id, run_id, item_id, provider, kind, action, payload, options,
    status, responder, reason, response, delivery, created_at, resolved_at";

fn err(e: sqlx::Error) -> String {
    e.to_string()
}

fn view(row: &SqliteRow) -> Result<RequestView, String> {
    let json = |column: &str| -> Result<serde_json::Value, String> {
        serde_json::from_str(&row.get::<String, _>(column)).map_err(|e| e.to_string())
    };
    let provider: String = row.get("provider");
    let status: String = row.get("status");
    let delivery: String = row.get("delivery");
    Ok(RequestView {
        id: row.get("id"),
        thread_id: row.get("thread_id"),
        run_id: row.get("run_id"),
        item_id: row.get("item_id"),
        provider: Provider::parse(&provider).ok_or(format!("unknown provider {provider}"))?,
        kind: row.get("kind"),
        action: serde_json::from_value(serde_json::Value::String(row.get("action")))
            .map_err(|e| e.to_string())?,
        payload: json("payload")?,
        options: serde_json::from_value(json("options")?).map_err(|e| e.to_string())?,
        status: Status::parse(&status).ok_or(format!("unknown request status {status}"))?,
        responder: row.get("responder"),
        reason: row.get("reason"),
        response: row.get("response"),
        delivery: Delivery::parse(&delivery).ok_or(format!("unknown delivery {delivery}"))?,
        created_at: row.get("created_at"),
        resolved_at: row.get("resolved_at"),
    })
}

/// One request, as it is saved.
pub async fn get(conn: &mut SqliteConnection, id: i64) -> Result<Option<RequestView>, String> {
    let sql = format!("SELECT {COLUMNS} FROM harness_requests WHERE id = ?1");
    let row = sqlx::query(&sql)
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(err)?;
    row.as_ref().map(view).transpose()
}

/// The thread's requests, oldest first: what a reload shows.
pub async fn list(conn: &mut SqliteConnection, thread_id: i64) -> Result<Vec<RequestView>, String> {
    let sql = format!("SELECT {COLUMNS} FROM harness_requests WHERE thread_id = ?1 ORDER BY id");
    let rows = sqlx::query(&sql)
        .bind(thread_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(err)?;
    rows.iter().map(view).collect()
}

/// How many of the thread's requests wait for the student.
pub async fn pending(pool: &SqlitePool, thread_id: i64) -> Result<i64, String> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM harness_requests WHERE thread_id = ?1 AND status = 'pending'",
    )
    .bind(thread_id)
    .fetch_one(pool)
    .await
    .map_err(err)
}

/// A request saved, and the row that shows it, under the open run (`owner`,
/// none in a turn the CLI started itself). The row holds only its id.
pub struct Opened {
    pub request: RequestView,
    pub item_id: i64,
    pub revision: i64,
}

pub async fn open(
    pool: &SqlitePool,
    thread_id: i64,
    owner: Option<&Owner>,
    (provider, generation): (Provider, Generation),
    new: &NewRequest,
) -> Result<Opened, String> {
    let mut tx = pool.begin().await.map_err(err)?;
    let payload = new.payload.to_string();
    let options = serde_json::to_string(&new.options).map_err(|e| e.to_string())?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO harness_requests
             (thread_id, run_id, attempt_id, binding_id, provider, kind, action, payload,
              options, responder, generation, native_ref)
         VALUES (?1, ?2, ?3, (SELECT binding_id FROM harness_runs WHERE id = ?2), ?4,
                 'approval', ?5, ?6, ?7, 'live', ?8, ?9)
         RETURNING id",
    )
    .bind(thread_id)
    .bind(owner.map(|o| o.run_id))
    .bind(owner.map(|o| o.attempt_id))
    .bind(provider.as_str())
    .bind(new.action.as_str())
    .bind(&payload)
    .bind(&options)
    .bind(generation as i64)
    .bind(&new.native_ref)
    .fetch_one(&mut *tx)
    .await
    .map_err(err)?;
    let item_id = sqlx::query(
        "INSERT INTO harness_items (thread_id, kind, ref_id, run_id, attempt_id, entity_ref)
         VALUES (?1, 'request', ?2, ?3, ?4, ?5)",
    )
    .bind(thread_id)
    .bind(id.to_string())
    .bind(owner.map(|o| o.run_id))
    .bind(owner.map(|o| o.attempt_id))
    .bind(owner.map(|o| o.entity.as_str()))
    .execute(&mut *tx)
    .await
    .map_err(err)?
    .last_insert_rowid();
    sqlx::query("UPDATE harness_requests SET item_id = ?2 WHERE id = ?1")
        .bind(id)
        .bind(item_id)
        .execute(&mut *tx)
        .await
        .map_err(err)?;
    let revision = raise_revision(&mut tx, thread_id).await?;
    let request = get(&mut tx, id)
        .await?
        .ok_or("the request just saved is gone")?;
    tx.commit().await.map_err(err)?;
    Ok(Opened {
        request,
        item_id,
        revision,
    })
}

/// The student's answer, inside a command's commit: the one conditional
/// write that claims the request. False when it was no longer pending.
pub async fn respond(
    conn: &mut SqliteConnection,
    thread_id: i64,
    id: i64,
    option: &str,
) -> Result<Option<RequestView>, String> {
    let claimed = sqlx::query(
        "UPDATE harness_requests
            SET status = 'resolved', response = ?3, delivery = 'sending',
                resolved_at = datetime('now')
          WHERE id = ?1 AND thread_id = ?2 AND status = 'pending'",
    )
    .bind(id)
    .bind(thread_id)
    .bind(option)
    .execute(&mut *conn)
    .await
    .map_err(err)?
    .rows_affected();
    match claimed {
        0 => Ok(None),
        _ => get(conn, id).await,
    }
}

/// What a request's reply proved. `Failed` puts it back to `pending` so the
/// student can answer again; the others only move a delivery that is still
/// `sending`. `None` when nothing changed.
pub async fn delivered(
    pool: &SqlitePool,
    id: i64,
    delivery: Delivery,
) -> Result<Option<(RequestView, i64)>, String> {
    let sql = match delivery {
        Delivery::Failed => {
            "UPDATE harness_requests
                SET status = 'pending', response = NULL, resolved_at = NULL, delivery = 'failed'
              WHERE id = ?1 AND status = 'resolved' AND delivery = 'sending'
              RETURNING thread_id"
        }
        _ => {
            "UPDATE harness_requests SET delivery = ?2
              WHERE id = ?1 AND status = 'resolved' AND delivery = 'sending'
              RETURNING thread_id"
        }
    };
    let mut tx = pool.begin().await.map_err(err)?;
    let thread: Option<i64> = sqlx::query_scalar(sql)
        .bind(id)
        .bind(delivery.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(err)?;
    let Some(thread_id) = thread else {
        return Ok(None);
    };
    let revision = raise_revision(&mut tx, thread_id).await?;
    let request = get(&mut tx, id).await?.ok_or("the request is gone")?;
    tx.commit().await.map_err(err)?;
    Ok(Some((request, revision)))
}

/// Requests that can no longer be answered: each still `pending` becomes
/// `status` with `reason`, and an answer still `sending` becomes
/// `uncertain`, since its session is gone before it confirmed. Returns
/// those that changed and the thread's revision.
pub async fn end(
    pool: &SqlitePool,
    thread_id: i64,
    ids: &[i64],
    status: Status,
    reason: &str,
) -> Result<(Vec<RequestView>, i64), String> {
    let mut tx = pool.begin().await.map_err(err)?;
    let mut changed = Vec::new();
    for id in ids {
        let moved = sqlx::query(
            "UPDATE harness_requests
                SET status = CASE WHEN status = 'pending' THEN ?3 ELSE status END,
                    reason = CASE WHEN status = 'pending' THEN ?4 ELSE reason END,
                    responder = CASE WHEN status = 'pending' THEN 'not_resumable'
                                     ELSE responder END,
                    resolved_at = CASE WHEN status = 'pending' THEN datetime('now')
                                       ELSE resolved_at END,
                    delivery = CASE WHEN delivery = 'sending' THEN 'uncertain' ELSE delivery END
              WHERE id = ?1 AND thread_id = ?2
                AND (status = 'pending' OR delivery = 'sending')",
        )
        .bind(id)
        .bind(thread_id)
        .bind(status.as_str())
        .bind(reason)
        .execute(&mut *tx)
        .await
        .map_err(err)?
        .rows_affected();
        if moved > 0 {
            if let Some(request) = get(&mut tx, *id).await? {
                changed.push(request);
            }
        }
    }
    let revision = match changed.is_empty() {
        true => 0,
        false => raise_revision(&mut tx, thread_id).await?,
    };
    tx.commit().await.map_err(err)?;
    Ok((changed, revision))
}

/// The pending requests a turn left behind as it finished: its run's, or
/// for a turn the CLI started itself (`run_id` `None`), those with no run.
pub async fn left_by(
    pool: &SqlitePool,
    thread_id: i64,
    run_id: Option<i64>,
) -> Result<Vec<i64>, String> {
    sqlx::query_scalar(
        "SELECT id FROM harness_requests
          WHERE thread_id = ?1 AND run_id IS ?2 AND status = 'pending' ORDER BY id",
    )
    .bind(thread_id)
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(err)
}

/// The request a provider's retired callback was, among those its session
/// `generation` raised on this thread that still wait for an answer or for
/// its confirmation.
pub async fn by_native(
    pool: &SqlitePool,
    thread_id: i64,
    generation: Generation,
    native_ref: &str,
) -> Result<Option<RequestView>, String> {
    let id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM harness_requests
          WHERE thread_id = ?1 AND generation = ?2 AND native_ref = ?3
            AND (status = 'pending' OR delivery = 'sending')
          ORDER BY id DESC LIMIT 1",
    )
    .bind(thread_id)
    .bind(generation as i64)
    .bind(native_ref)
    .fetch_optional(pool)
    .await
    .map_err(err)?;
    match id {
        Some(id) => get(&mut *pool.acquire().await.map_err(err)?, id).await,
        None => Ok(None),
    }
}

/// Startup: no route survives a restart. A request still pending expired
/// with the app, and an answer still `sending` may or may not have arrived.
pub async fn reconcile(conn: &mut SqliteConnection) -> Result<u64, String> {
    let expired = sqlx::query(
        "UPDATE harness_requests
            SET status = 'expired', responder = 'not_resumable', reason = ?1,
                resolved_at = datetime('now')
          WHERE status = 'pending'",
    )
    .bind(reasons::APP_CLOSED)
    .execute(&mut *conn)
    .await
    .map_err(err)?
    .rows_affected();
    sqlx::query("UPDATE harness_requests SET delivery = 'uncertain' WHERE delivery = 'sending'")
        .execute(&mut *conn)
        .await
        .map_err(err)?;
    Ok(expired)
}

/// How many of the thread's requests wait on the student: pending, with a
/// session that can still take the answer. The thread list counts the same
/// (`app/src/lib/harness/reads.ts`).
pub async fn waiting(pool: &SqlitePool, thread_id: i64) -> Result<u32, String> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM harness_requests
          WHERE thread_id = ?1 AND status = 'pending' AND responder = 'live'",
    )
    .bind(thread_id)
    .fetch_one(pool)
    .await
    .map_err(err)
}

/// The chat's permission mode, `ask` for a chat that is gone.
pub async fn mode(pool: &SqlitePool, thread_id: i64) -> Result<super::PermissionMode, String> {
    let mode: Option<String> =
        sqlx::query_scalar("SELECT permission_mode FROM harness_threads WHERE id = ?1")
            .bind(thread_id)
            .fetch_optional(pool)
            .await
            .map_err(err)?;
    Ok(mode
        .as_deref()
        .and_then(super::PermissionMode::parse)
        .unwrap_or_default())
}

/// Set the chat's permission mode, inside a command's commit.
pub async fn set_mode(
    conn: &mut SqliteConnection,
    thread_id: i64,
    mode: super::PermissionMode,
) -> Result<(), String> {
    sqlx::query("UPDATE harness_threads SET permission_mode = ?2 WHERE id = ?1")
        .bind(thread_id)
        .bind(mode.as_str())
        .execute(&mut *conn)
        .await
        .map_err(err)?;
    Ok(())
}
