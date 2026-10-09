use sqlx::{Row, SqlitePool};

use crate::harness::event::HarnessEvent;

use super::items::{insert_item, set_status};

/// A tool row's `meta`: everything the expanded row shows but the title.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ToolMeta {
    kind: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
    ok: Option<bool>,
    output: Option<String>,
}

/// Fold one event into the tables. Returns the id of the row it inserted,
/// when it inserted one, so the frontend can key the timeline on it.
pub async fn apply(
    pool: &SqlitePool,
    thread_id: i64,
    ev: &HarnessEvent,
) -> Result<Option<i64>, String> {
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
            insert_item(pool, thread_id, "user", None, Some(text), meta)
                .await
                .map(Some)
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
            insert_item(pool, thread_id, "assistant", None, Some(text), None)
                .await
                .map(Some)
        }
        HarnessEvent::Thinking { text } => {
            insert_item(pool, thread_id, "thinking", None, Some(text), None)
                .await
                .map(Some)
        }
        HarnessEvent::ToolStarted {
            id,
            kind,
            name,
            title,
            input,
        } => {
            let meta = ToolMeta {
                kind: Some(
                    serde_json::to_value(kind)
                        .ok()
                        .and_then(|v| v.as_str().map(String::from))
                        .unwrap_or_default(),
                ),
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
        HarnessEvent::ToolFinished {
            id,
            ok,
            output,
            title,
        } => {
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
                Some(t) => {
                    sqlx::query("UPDATE harness_items SET meta = ?2, content = ?3 WHERE id = ?1")
                        .bind(item_id)
                        .bind(serde_json::to_string(&meta).ok())
                        .bind(t)
                }
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
            insert_item(pool, thread_id, "error", None, Some(message), meta)
                .await
                .map(Some)
        }
        // Its own row, not an error: the turn ended normally, and a reload must
        // offer the rule again. `content` is the refused target.
        HarnessEvent::PermissionNeeded {
            tool,
            action,
            target,
            rule,
        } => {
            let meta = serde_json::json!({
                "tool": tool,
                "action": action,
                "target": target,
                "rule": rule,
            });
            insert_item(
                pool,
                thread_id,
                "permission",
                None,
                target.as_deref(),
                Some(meta.to_string()),
            )
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
