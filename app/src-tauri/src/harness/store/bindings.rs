//! A chat's provider sessions. Each provider that has answered in a chat
//! has one current binding there: the native session its next send resumes
//! and how much of the chat that session has seen (`covered_through`, a run
//! ordinal). A binding that had to be given up stays as a row its runs
//! still point at, no longer current.

use sqlx::{Row, SqliteConnection, SqliteExecutor};

use crate::harness::event::Provider;

#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub id: i64,
    pub provider: Provider,
    /// The provider's session id; `None` until it names one.
    pub native_session: Option<String>,
    /// The last run ordinal the session has seen, by running it or by
    /// receiving it in a handoff.
    pub covered_through: i64,
    /// What the session last reported of its context.
    pub context_tokens: Option<i64>,
    pub context_window: Option<i64>,
}

const COLUMNS: &str =
    "id, provider, native_session, covered_through, context_tokens, context_window";

fn read(row: &sqlx::sqlite::SqliteRow) -> Result<Binding, String> {
    let provider: String = row.get("provider");
    Ok(Binding {
        id: row.get("id"),
        provider: Provider::parse(&provider)
            .ok_or_else(|| format!("unknown provider {provider}"))?,
        native_session: row.get("native_session"),
        covered_through: row.get("covered_through"),
        context_tokens: row.get("context_tokens"),
        context_window: row.get("context_window"),
    })
}

/// The chat's current binding for `provider`, if it has one.
pub async fn find(
    db: impl SqliteExecutor<'_>,
    thread_id: i64,
    provider: Provider,
) -> Result<Option<Binding>, String> {
    sqlx::query(&format!(
        "SELECT {COLUMNS} FROM harness_bindings
          WHERE thread_id = ?1 AND provider = ?2 AND current = 1"
    ))
    .bind(thread_id)
    .bind(provider.as_str())
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?
    .as_ref()
    .map(read)
    .transpose()
}

/// The chat's current binding for `provider`, made when it has none: a
/// session that has seen nothing.
pub async fn current(
    tx: &mut SqliteConnection,
    thread_id: i64,
    provider: Provider,
) -> Result<Binding, String> {
    if let Some(binding) = find(&mut *tx, thread_id, provider).await? {
        return Ok(binding);
    }
    insert(tx, thread_id, provider, None, 0).await
}

/// Give up the provider's current session and start over: the old binding
/// stays for the runs it answered, and a new one with no session and no
/// coverage becomes current, so its first prompt carries the whole chat.
pub async fn fresh(
    tx: &mut SqliteConnection,
    thread_id: i64,
    provider: Provider,
) -> Result<Binding, String> {
    retire(tx, thread_id, provider).await?;
    insert(tx, thread_id, provider, None, 0).await
}

async fn retire(
    tx: &mut SqliteConnection,
    thread_id: i64,
    provider: Provider,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE harness_bindings SET current = 0
          WHERE thread_id = ?1 AND provider = ?2 AND current = 1",
    )
    .bind(thread_id)
    .bind(provider.as_str())
    .execute(&mut *tx)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

async fn insert(
    tx: &mut SqliteConnection,
    thread_id: i64,
    provider: Provider,
    native_session: Option<&str>,
    covered_through: i64,
) -> Result<Binding, String> {
    let id = sqlx::query(
        "INSERT INTO harness_bindings (thread_id, provider, native_session, current, covered_through)
         VALUES (?1, ?2, ?3, 1, ?4)",
    )
    .bind(thread_id)
    .bind(provider.as_str())
    .bind(native_session)
    .bind(covered_through)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .last_insert_rowid();
    Ok(Binding {
        id,
        provider,
        native_session: native_session.map(String::from),
        covered_through,
        context_tokens: None,
        context_window: None,
    })
}

/// The binding a run was written on: the session its start step resumes.
pub async fn of_run(db: impl SqliteExecutor<'_>, run_id: i64) -> Result<Option<Binding>, String> {
    sqlx::query(&format!(
        "SELECT {COLUMNS} FROM harness_bindings
          WHERE id = (SELECT binding_id FROM harness_runs WHERE id = ?1)"
    ))
    .bind(run_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?
    .as_ref()
    .map(read)
    .transpose()
}

/// The provider named the session answering `run_id`. A binding learns its
/// first one. A different one is the same conversation under a new id on
/// the provider's side: a new binding takes the old one's place as
/// current, with its coverage and every run it answered, so a rewind to
/// one of them still finds the session that holds it.
pub async fn bind_session(
    tx: &mut SqliteConnection,
    run_id: i64,
    session: &str,
) -> Result<(), String> {
    let err = |e: sqlx::Error| e.to_string();
    let row = sqlx::query(
        "SELECT b.id, b.thread_id, b.provider, b.native_session, b.current, b.covered_through,
                b.context_tokens, b.context_window
           FROM harness_runs r JOIN harness_bindings b ON b.id = r.binding_id WHERE r.id = ?1",
    )
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(err)?;
    let Some(row) = row else {
        return Ok(());
    };
    let old: i64 = row.get("id");
    match row.get::<Option<String>, _>("native_session") {
        Some(known) if known == session => {}
        None => {
            sqlx::query("UPDATE harness_bindings SET native_session = ?2 WHERE id = ?1")
                .bind(old)
                .bind(session)
                .execute(&mut *tx)
                .await
                .map_err(err)?;
        }
        Some(_) => {
            let current: i64 = row.get("current");
            // The old row gives up `current` first: one per chat and provider.
            sqlx::query("UPDATE harness_bindings SET current = 0 WHERE id = ?1")
                .bind(old)
                .execute(&mut *tx)
                .await
                .map_err(err)?;
            let id = sqlx::query(
                "INSERT INTO harness_bindings
                   (thread_id, provider, native_session, current, covered_through,
                    context_tokens, context_window)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(row.get::<i64, _>("thread_id"))
            .bind(row.get::<String, _>("provider"))
            .bind(session)
            .bind(current)
            .bind(row.get::<i64, _>("covered_through"))
            .bind(row.get::<Option<i64>, _>("context_tokens"))
            .bind(row.get::<Option<i64>, _>("context_window"))
            .execute(&mut *tx)
            .await
            .map_err(err)?
            .last_insert_rowid();
            for sql in [
                "UPDATE harness_runs SET binding_id = ?2 WHERE binding_id = ?1",
                "UPDATE harness_handoffs SET to_binding_id = ?2 WHERE to_binding_id = ?1",
                "UPDATE harness_handoffs SET from_binding_id = ?2 WHERE from_binding_id = ?1",
            ] {
                sqlx::query(sql)
                    .bind(old)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(err)?;
            }
        }
    }
    Ok(())
}

/// The binding whose session holds the question `item_id`: its run's, or,
/// for a question from before runs, the chat's first. With the model and
/// reasoning level that run was sent under.
pub struct Holder {
    pub provider: Provider,
    /// The native session to rewind; `None` when the binding has none, or
    /// has since been given up for a fresh one.
    pub session: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
}

pub async fn holder(
    pool: &sqlx::SqlitePool,
    thread_id: i64,
    item_id: i64,
) -> Result<Option<Holder>, String> {
    let err = |e: sqlx::Error| e.to_string();
    let own = sqlx::query(
        "SELECT b.provider, b.native_session, b.current, r.model, r.effort
           FROM harness_items i
           JOIN harness_runs r ON r.id = i.run_id
           JOIN harness_bindings b ON b.id = r.binding_id
          WHERE i.id = ?1 AND i.thread_id = ?2",
    )
    .bind(item_id)
    .bind(thread_id)
    .fetch_optional(pool)
    .await
    .map_err(err)?;
    let row = match own {
        Some(row) => Some(row),
        None => sqlx::query(
            "SELECT b.provider, b.native_session, b.current, t.model, t.effort
               FROM harness_bindings b JOIN harness_threads t ON t.id = b.thread_id
              WHERE b.thread_id = ?1 ORDER BY b.id LIMIT 1",
        )
        .bind(thread_id)
        .fetch_optional(pool)
        .await
        .map_err(err)?,
    };
    Ok(row.and_then(|row| {
        let provider = Provider::parse(&row.get::<String, _>("provider"))?;
        let current = row.get::<i64, _>("current") != 0;
        Some(Holder {
            provider,
            session: row
                .get::<Option<String>, _>("native_session")
                .filter(|_| current),
            model: row.get("model"),
            effort: row.get("effort"),
        })
    }))
}

/// The rows from `item_id` on are about to leave the chat. The binding
/// that holds that question keeps its session, which the provider's own
/// rewind takes back, and is marked as having seen only what came before
/// it. Every other binding that has seen anything from that question on
/// holds context that no longer exists: its session is given up and it
/// covers nothing, so its next use starts fresh with the whole chat.
/// Returns the providers whose current session was given up.
pub async fn rewound(
    tx: &mut SqliteConnection,
    thread_id: i64,
    item_id: i64,
) -> Result<Vec<Provider>, String> {
    let err = |e: sqlx::Error| e.to_string();
    // The question's own run, the binding that holds it and what that
    // binding had seen before the run: what its handoff started from, or
    // every earlier run when it carried none.
    let own: Option<(i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT r.ordinal, r.binding_id,
                (SELECT h.covered_from FROM harness_handoffs h
                  WHERE h.run_id = r.id AND h.status = 'inline')
           FROM harness_items i JOIN harness_runs r ON r.id = i.run_id
          WHERE i.id = ?1 AND i.thread_id = ?2 AND r.ordinal > 0",
    )
    .bind(item_id)
    .bind(thread_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(err)?;
    // A question from before runs: everything any session has seen goes.
    let (ordinal, holder, before) = match own {
        Some((ordinal, binding, from)) => (ordinal, Some(binding), from.unwrap_or(ordinal) - 1),
        None => {
            let first: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM harness_bindings WHERE thread_id = ?1 ORDER BY id LIMIT 1",
            )
            .bind(thread_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(err)?;
            (0, first, 0)
        }
    };
    // Only a current binding is kept: one given up earlier is not used again.
    let kept: i64 = sqlx::query_scalar(
        "UPDATE harness_bindings SET covered_through = MIN(covered_through, ?2)
          WHERE id = ?1 AND current = 1 RETURNING id",
    )
    .bind(holder)
    .bind(before.max(0))
    .fetch_optional(&mut *tx)
    .await
    .map_err(err)?
    .unwrap_or(-1);
    let stale = "thread_id = ?1 AND id != ?2 AND covered_through >= ?3
                 AND (covered_through > 0 OR native_session IS NOT NULL)";
    let gone: Vec<String> = sqlx::query_scalar(&format!(
        "SELECT provider FROM harness_bindings WHERE {stale} AND current = 1"
    ))
    .bind(thread_id)
    .bind(kept)
    .bind(ordinal)
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?;
    sqlx::query(&format!(
        "UPDATE harness_bindings
            SET native_session = NULL, covered_through = 0, context_tokens = NULL
          WHERE {stale}"
    ))
    .bind(thread_id)
    .bind(kept)
    .bind(ordinal)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    Ok(gone
        .iter()
        .filter_map(|provider| Provider::parse(provider))
        .collect())
}

/// The provider's live session in this chat is gone. One that never named
/// itself cannot be resumed, so whatever it had seen is lost with it: its
/// binding covers nothing, and the next send hands a new session the whole
/// chat. `None` for `thread_id` is every chat, at startup: no session
/// survives a restart.
pub async fn forget_unnamed(
    db: impl SqliteExecutor<'_>,
    thread: Option<(i64, Provider)>,
) -> Result<(), String> {
    let lost = "UPDATE harness_bindings SET covered_through = 0, context_tokens = NULL
                 WHERE native_session IS NULL AND covered_through != 0";
    let done = match thread {
        Some((thread_id, provider)) => {
            sqlx::query(&format!("{lost} AND thread_id = ?1 AND provider = ?2"))
                .bind(thread_id)
                .bind(provider.as_str())
                .execute(db)
                .await
        }
        None => sqlx::query(lost).execute(db).await,
    };
    done.map(|_| ()).map_err(|e| e.to_string())
}

/// The session answering `run_id` reported its context: how much it holds
/// and the window it has. A handoff into that session budgets against it.
pub async fn report_usage(
    tx: &mut SqliteConnection,
    run_id: i64,
    context_tokens: Option<u64>,
    context_window: Option<u64>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE harness_bindings
            SET context_tokens = COALESCE(?2, context_tokens),
                context_window = COALESCE(?3, context_window)
          WHERE id = (SELECT binding_id FROM harness_runs WHERE id = ?1)",
    )
    .bind(run_id)
    .bind(context_tokens.map(|n| n as i64))
    .bind(context_window.map(|n| n as i64))
    .execute(&mut *tx)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// The session has seen the chat through run `ordinal`.
pub async fn cover(tx: &mut SqliteConnection, binding_id: i64, ordinal: i64) -> Result<(), String> {
    sqlx::query(
        "UPDATE harness_bindings SET covered_through = MAX(covered_through, ?2) WHERE id = ?1",
    )
    .bind(binding_id)
    .bind(ordinal)
    .execute(&mut *tx)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}
