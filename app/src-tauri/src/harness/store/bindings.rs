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
/// first one. A different one is a new conversation on the provider's
/// side: the run moves to a new binding, which takes the old one's place
/// as current and its coverage, since the provider carried the context
/// over itself.
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
                "UPDATE harness_runs SET binding_id = ?2 WHERE id = ?1",
                "UPDATE harness_handoffs SET to_binding_id = ?2 WHERE run_id = ?1",
            ] {
                sqlx::query(sql)
                    .bind(run_id)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(err)?;
            }
        }
    }
    Ok(())
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
