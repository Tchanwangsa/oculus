//! Thread and timeline rows, written here and read by the frontend. Rust owns
//! the writes because events arrive here in order, so a crash mid-tool still
//! leaves a row that says so.

use sqlx::{Row, SqlitePool};

use super::event::{HarnessEvent, Provider};

/// A tool row's `meta`: everything the expanded row shows but the title.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ToolMeta {
    kind: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
    ok: Option<bool>,
    output: Option<String>,
}

/// Start a thread. `lecture_id` is the recording a dock conversation is about.
/// A lecture thread's subject is read off the `lectures` row, never trusted
/// from the payload: a stale one would point at the wrong course folder.
pub async fn create_thread(
    pool: &SqlitePool,
    provider: Provider,
    model: Option<&str>,
    subject_id: Option<i64>,
    lecture_id: Option<&str>,
    first_message: &str,
) -> Result<i64, String> {
    let subject_id = match lecture_id {
        Some(id) => sqlx::query_scalar::<_, Option<i64>>("SELECT subject_id FROM lectures WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("no lecture {id}"))?,
        None => subject_id,
    };
    let title = title_from(first_message);
    let res = sqlx::query(
        "INSERT INTO harness_threads (provider, model, subject_id, lecture_id, title, status)
         VALUES (?1, ?2, ?3, ?4, ?5, 'idle')",
    )
    .bind(provider.as_str())
    .bind(model)
    .bind(subject_id)
    .bind(lecture_id)
    .bind(title)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(res.last_insert_rowid())
}

/// The first line of the first message, clipped: the name until the naming
/// turn replaces it ([`claim_naming`]).
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
    /// The subject's Canvas code, i.e. its `courses/` folder. Joined, not stored,
    /// so a rename cannot strand the thread; None is the general thread.
    pub subject_code: Option<String>,
    /// The dock conversation's recording, joined likewise. Deleting the
    /// recording clears the column, not the conversation.
    pub lecture: Option<LectureRef>,
}

/// What a lecture thread's instructions name: the recording and its transcript.
pub struct LectureRef {
    pub id: String,
    pub title: String,
    /// `YYYY-MM-DD`. The title is the timetable's (`… TU L105`), so the date is
    /// what says which lecture this is, and which week's slides go with it.
    pub date: String,
    /// Whether `transcript.vtt` is on disk: clearing transcripts leaves the
    /// column's path behind, and the agent must not be sent to a missing file.
    pub has_transcript: bool,
}

pub async fn thread(pool: &SqlitePool, id: i64) -> Result<ThreadRow, String> {
    let r = sqlx::query(
        "SELECT t.id, t.provider, t.provider_session_id, t.model, s.code AS subject_code,
                l.id AS lecture_id, l.title AS lecture_title, l.date AS lecture_date,
                l.transcript_path
         FROM harness_threads t
         LEFT JOIN subjects s ON s.id = t.subject_id
         LEFT JOIN lectures l ON l.id = t.lecture_id
         WHERE t.id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("no thread {id}"))?;
    let provider: String = r.get("provider");
    let lecture = r.get::<Option<String>, _>("lecture_id").map(|lid| LectureRef {
        id: lid,
        title: r.get("lecture_title"),
        date: r.get::<String, _>("lecture_date").chars().take(10).collect(),
        has_transcript: r
            .get::<Option<String>, _>("transcript_path")
            .is_some_and(|p| std::path::Path::new(&p).exists()),
    });
    Ok(ThreadRow {
        id: r.get("id"),
        provider: Provider::parse(&provider).ok_or_else(|| format!("unknown provider {provider}"))?,
        provider_session_id: r.get("provider_session_id"),
        model: r.get("model"),
        subject_code: r.get("subject_code"),
        lecture,
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
        HarnessEvent::UserMessage { text, at } => {
            set_status(pool, thread_id, "running").await?;
            // `content` is only what the student typed; the playhead second is
            // a fact about the message, so it goes in `meta` ("at 3:40").
            let meta = at.map(|at| serde_json::json!({ "at": at }).to_string());
            insert_item(pool, thread_id, "user", None, Some(text), meta).await.map(Some)
        }
        HarnessEvent::TurnStarted => set_status(pool, thread_id, "running").await.map(|_| None),
        HarnessEvent::TurnAnchor { anchor } => {
            // The newest question is this turn's: one turn per thread runs at a time.
            sqlx::query(
                "UPDATE harness_items SET anchor = ?2 WHERE id =
                   (SELECT id FROM harness_items
                     WHERE thread_id = ?1 AND kind = 'user' ORDER BY id DESC LIMIT 1)",
            )
            .bind(thread_id)
            .bind(anchor)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
            Ok(None)
        }
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
        HarnessEvent::ToolFinished { id, ok, output, title } => {
            // Read-modify-write on the JSON rather than json_set.
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
            // A title learned on completion replaces the row's; absent or empty keeps it.
            let retitle = title.as_deref().filter(|t| !t.trim().is_empty());
            match retitle {
                Some(t) => sqlx::query("UPDATE harness_items SET meta = ?2, content = ?3 WHERE id = ?1")
                    .bind(item_id)
                    .bind(serde_json::to_string(&meta).ok())
                    .bind(t),
                None => sqlx::query("UPDATE harness_items SET meta = ?2 WHERE id = ?1")
                    .bind(item_id)
                    .bind(serde_json::to_string(&meta).ok()),
            }
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
            Ok(None)
        }
        // `meta` records which provider's credentials failed, so a reload can
        // redraw the sign-in card.
        HarnessEvent::Error { message, auth } => {
            let meta = auth.map(|p| serde_json::json!({ "auth": p.as_str() }).to_string());
            insert_item(pool, thread_id, "error", None, Some(message), meta).await.map(Some)
        }
        // Its own row, not an error: the turn ended normally, and a reload must
        // offer the rule again. `content` is the refused target.
        HarnessEvent::PermissionNeeded { tool, action, target, rule } => {
            let meta = serde_json::json!({
                "tool": tool,
                "action": action,
                "target": target,
                "rule": rule,
            });
            insert_item(pool, thread_id, "permission", None, target.as_deref(), Some(meta.to_string()))
                .await
                .map(Some)
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
        HarnessEvent::ThreadTitled { title } => {
            sqlx::query("UPDATE harness_threads SET title = ?2 WHERE id = ?1")
                .bind(thread_id)
                .bind(title)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            Ok(None)
        }
        HarnessEvent::TurnFinished { status } => {
            let s = if status == "failed" { "error" } else { "idle" };
            set_status(pool, thread_id, s).await?;
            // A stopped turn leaves a mark, so a cut-off answer reads as stopped.
            if status == "interrupted" {
                return insert_item(pool, thread_id, "interrupted", None, None, None)
                    .await
                    .map(Some);
            }
            Ok(None)
        }
        HarnessEvent::Exited { .. } => {
            // A mid-turn exit already produced a failed TurnFinished.
            Ok(None)
        }
        // No rows: a queued message has none until sent, and a rewind already deleted its.
        HarnessEvent::AssistantDelta { .. }
        | HarnessEvent::ThinkingDelta { .. }
        | HarnessEvent::ToolOutputDelta { .. }
        | HarnessEvent::Queued { .. }
        | HarnessEvent::Unqueued { .. }
        | HarnessEvent::Rewound { .. }
        | HarnessEvent::RateLimits { .. } => Ok(None),
    }
}

/// A question in this thread and its turn's anchor, checked against the row
/// before an edit deletes everything after it. `anchor` is None for a question
/// predating migration 28 or whose turn never started.
pub struct Question {
    pub text: String,
    pub anchor: Option<String>,
}

pub async fn user_item(pool: &SqlitePool, thread_id: i64, item_id: i64) -> Result<Question, String> {
    let row = sqlx::query("SELECT kind, content, anchor FROM harness_items WHERE id = ?1 AND thread_id = ?2")
        .bind(item_id)
        .bind(thread_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no item {item_id} in thread {thread_id}"))?;
    let kind: String = row.get("kind");
    if kind != "user" {
        return Err(format!("item {item_id} is a {kind} row, not a question"));
    }
    Ok(Question {
        text: row.get::<Option<String>, _>("content").unwrap_or_default(),
        anchor: row.get::<Option<String>, _>("anchor"),
    })
}

/// Delete this row and everything after it — the local half of a rewind
/// (`Harness::rewind` rewinds the provider's session).
pub async fn truncate_from(pool: &SqlitePool, thread_id: i64, item_id: i64) -> Result<u64, String> {
    sqlx::query("DELETE FROM harness_items WHERE thread_id = ?1 AND id >= ?2")
        .bind(thread_id)
        .bind(item_id)
        .execute(pool)
        .await
        .map(|r| r.rows_affected())
        .map_err(|e| e.to_string())
}

/// A naming turn's input: the first question and the last answer.
pub struct NamingSeed {
    pub first_message: String,
    pub reply: String,
}

/// The first or last non-empty row of one kind. `order` is a literal, never
/// user input.
async fn one_item(
    pool: &SqlitePool,
    thread_id: i64,
    kind: &str,
    order: &'static str,
) -> Result<Option<String>, String> {
    sqlx::query(&format!(
        "SELECT content FROM harness_items
         WHERE thread_id = ?1 AND kind = ?2 AND content IS NOT NULL AND content != ''
         ORDER BY id {order} LIMIT 1"
    ))
    .bind(thread_id)
    .bind(kind)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())
    .map(|r| r.and_then(|r| r.get::<Option<String>, _>("content")))
}

/// Claim the right to name this thread and return what to name it from. The
/// claim flips `title_generated` 0→1 atomically, so a thread is named once; a
/// failed naming turn is not retried, since each costs a real turn.
pub async fn claim_naming(pool: &SqlitePool, thread_id: i64) -> Result<Option<NamingSeed>, String> {
    let first_message = one_item(pool, thread_id, "user", "ASC").await?;
    let reply = one_item(pool, thread_id, "assistant", "DESC").await?;
    let (Some(first_message), Some(reply)) = (first_message, reply) else {
        // Nothing was said back — a failed first turn. Leave the claim open.
        return Ok(None);
    };
    let claimed = sqlx::query(
        "UPDATE harness_threads SET title_generated = 1 WHERE id = ?1 AND title_generated = 0",
    )
    .bind(thread_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();
    if claimed == 0 {
        return Ok(None);
    }
    Ok(Some(NamingSeed { first_message, reply }))
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
