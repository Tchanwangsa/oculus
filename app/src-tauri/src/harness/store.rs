//! Thread and timeline rows, written here and read by the frontend.
//!
//! Same split as the chat agent before it: Rust owns the writes because the
//! events that make a row arrive on Rust's side, in order, and a crash
//! between "tool started" and "tool finished" must leave a row that says
//! so — not a webview that never heard the second half.

use sqlx::{Row, SqlitePool};

use super::event::{HarnessEvent, Provider};

/// A `harness_items` row's `meta` for a tool call. Everything the expanded
/// row shows that is not the title.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ToolMeta {
    kind: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
    ok: Option<bool>,
    output: Option<String>,
}

pub async fn create_thread(
    pool: &SqlitePool,
    provider: Provider,
    model: Option<&str>,
    subject_id: Option<i64>,
    first_message: &str,
) -> Result<i64, String> {
    let title = title_from(first_message);
    let res = sqlx::query(
        "INSERT INTO harness_threads (provider, model, subject_id, title, status)
         VALUES (?1, ?2, ?3, ?4, 'idle')",
    )
    .bind(provider.as_str())
    .bind(model)
    .bind(subject_id)
    .bind(title)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(res.last_insert_rowid())
}

/// The first line of the first message, clipped. bb derives titles the same
/// way until the provider names the thread; there is no naming step here.
fn title_from(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    let mut t: String = line.chars().take(72).collect();
    if line.chars().count() > 72 {
        t.push('…');
    }
    if t.is_empty() {
        "New thread".into()
    } else {
        t
    }
}

pub struct ThreadRow {
    pub id: i64,
    pub provider: Provider,
    pub provider_session_id: Option<String>,
    pub model: Option<String>,
    /// The scoped subject's Canvas code, which is also its folder name under
    /// `courses/`. Joined rather than stored so a renamed subject cannot
    /// leave a thread pointing at a folder that no longer exists; None is
    /// the general thread, or a subject that has since been removed.
    pub subject_code: Option<String>,
}

pub async fn thread(pool: &SqlitePool, id: i64) -> Result<ThreadRow, String> {
    let r = sqlx::query(
        "SELECT t.id, t.provider, t.provider_session_id, t.model, s.code AS subject_code
         FROM harness_threads t
         LEFT JOIN subjects s ON s.id = t.subject_id
         WHERE t.id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("no thread {id}"))?;
    let provider: String = r.get("provider");
    Ok(ThreadRow {
        id: r.get("id"),
        provider: Provider::parse(&provider).ok_or_else(|| format!("unknown provider {provider}"))?,
        provider_session_id: r.get("provider_session_id"),
        model: r.get("model"),
        subject_code: r.get("subject_code"),
    })
}

pub async fn set_model(pool: &SqlitePool, id: i64, model: Option<&str>) -> Result<(), String> {
    sqlx::query("UPDATE harness_threads SET model = ?2 WHERE id = ?1")
        .bind(id)
        .bind(model)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub async fn delete_thread(pool: &SqlitePool, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM harness_threads WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

async fn insert_item(
    pool: &SqlitePool,
    thread_id: i64,
    kind: &str,
    ref_id: Option<&str>,
    content: Option<&str>,
    meta: Option<String>,
) -> Result<i64, String> {
    let res = sqlx::query(
        "INSERT INTO harness_items (thread_id, kind, ref_id, content, meta) VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(thread_id)
    .bind(kind)
    .bind(ref_id)
    .bind(content)
    .bind(meta)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(res.last_insert_rowid())
}

async fn set_status(pool: &SqlitePool, thread_id: i64, status: &str) -> Result<(), String> {
    sqlx::query(
        "UPDATE harness_threads SET status = ?2, updated_at = datetime('now') WHERE id = ?1",
    )
    .bind(thread_id)
    .bind(status)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Fold one event into the tables. Returns the id of the row it inserted,
/// when it inserted one, so the frontend can key the timeline on it.
pub async fn apply(pool: &SqlitePool, thread_id: i64, ev: &HarnessEvent) -> Result<Option<i64>, String> {
    match ev {
        HarnessEvent::SessionStarted {
            provider_session_id,
            model,
            ..
        } => {
            sqlx::query(
                "UPDATE harness_threads SET provider_session_id = ?2, model = COALESCE(?3, model) WHERE id = ?1",
            )
            .bind(thread_id)
            .bind(provider_session_id)
            .bind(model)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
            Ok(None)
        }
        HarnessEvent::UserMessage { text } => {
            set_status(pool, thread_id, "running").await?;
            insert_item(pool, thread_id, "user", None, Some(text), None).await.map(Some)
        }
        HarnessEvent::TurnStarted => set_status(pool, thread_id, "running").await.map(|_| None),
        HarnessEvent::AssistantMessage { text } => {
            insert_item(pool, thread_id, "assistant", None, Some(text), None).await.map(Some)
        }
        HarnessEvent::Thinking { text } => {
            insert_item(pool, thread_id, "thinking", None, Some(text), None).await.map(Some)
        }
        HarnessEvent::ToolStarted {
            id,
            kind,
            name,
            title,
            input,
        } => {
            let meta = ToolMeta {
                kind: Some(serde_json::to_value(kind).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()),
                name: Some(name.clone()),
                input: Some(input.clone()),
                ok: None,
                output: None,
            };
            insert_item(
                pool,
                thread_id,
                "tool",
                Some(id),
                Some(title),
                serde_json::to_string(&meta).ok(),
            )
            .await
            .map(Some)
        }
        HarnessEvent::ToolFinished { id, ok, output } => {
            // Read-modify-write on the JSON: SQLite's json_set is there, but
            // a string round trip is one query fewer to get wrong.
            let row = sqlx::query(
                "SELECT id, meta FROM harness_items WHERE thread_id = ?1 AND ref_id = ?2 ORDER BY id DESC LIMIT 1",
            )
            .bind(thread_id)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
            let Some(row) = row else {
                return Ok(None);
            };
            let item_id: i64 = row.get("id");
            let mut meta: ToolMeta = row
                .get::<Option<String>, _>("meta")
                .and_then(|m| serde_json::from_str(&m).ok())
                .unwrap_or_default();
            meta.ok = Some(*ok);
            meta.output = Some(output.clone());
            sqlx::query("UPDATE harness_items SET meta = ?2 WHERE id = ?1")
                .bind(item_id)
                .bind(serde_json::to_string(&meta).ok())
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            Ok(None)
        }
        HarnessEvent::Error { message } => {
            insert_item(pool, thread_id, "error", None, Some(message), None).await.map(Some)
        }
        HarnessEvent::Usage {
            input_tokens,
            output_tokens,
            context_tokens,
            context_window,
            cost_usd,
        } => {
            let usage = serde_json::json!({
                "inputTokens": input_tokens,
                "outputTokens": output_tokens,
                "contextTokens": context_tokens,
                "contextWindow": context_window,
                "costUsd": cost_usd,
            });
            sqlx::query("UPDATE harness_threads SET usage = ?2 WHERE id = ?1")
                .bind(thread_id)
                .bind(usage.to_string())
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            Ok(None)
        }
        HarnessEvent::TurnFinished { status } => {
            let s = if status == "failed" { "error" } else { "idle" };
            set_status(pool, thread_id, s).await.map(|_| None)
        }
        HarnessEvent::Exited { .. } => {
            // A process gone mid-turn already produced a failed TurnFinished;
            // an idle one leaving changes nothing the reader can see.
            Ok(None)
        }
        HarnessEvent::AssistantDelta { .. }
        | HarnessEvent::ThinkingDelta { .. }
        | HarnessEvent::ToolOutputDelta { .. }
        | HarnessEvent::RateLimits { .. } => Ok(None),
    }
}

/// Rate limits are per provider account, not per thread, so they live in
/// `settings` under the provider's key and the page reads them on load.
pub async fn save_rate_limits(pool: &SqlitePool, provider: Provider, ev: &HarnessEvent) -> Result<(), String> {
    let HarnessEvent::RateLimits { windows } = ev else {
        return Ok(());
    };
    let key = format!("harness_rate_limits_{}", provider.as_str());
    let value = serde_json::to_string(windows).map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Threads left `running` by a crash or a quit mid-turn. Called at startup;
/// nothing is going to finish them.
pub async fn reconcile(pool: &SqlitePool) -> Result<u64, String> {
    sqlx::query("UPDATE harness_threads SET status = 'idle' WHERE status = 'running'")
        .execute(pool)
        .await
        .map(|r| r.rows_affected())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_the_first_line_clipped() {
        assert_eq!(title_from("\n\n  Hello world  \nmore"), "Hello world");
        assert_eq!(title_from(""), "New thread");
        let long = "x".repeat(100);
        let t = title_from(&long);
        assert!(t.ends_with('…'));
        assert_eq!(t.chars().count(), 73);
    }
}
