use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::harness::event::{HarnessEvent, Source};

use super::items::{insert_item, set_status};
use super::Owner;

/// A tool row's `meta`: everything the expanded row shows but the title.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ToolMeta {
    kind: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
    ok: Option<bool>,
    output: Option<String>,
}

/// The row an event wrote or updated, and its weak id within the run.
#[derive(Default)]
pub struct Stored {
    pub item_id: Option<i64>,
    /// A new row, which took the owner's next ordinal.
    pub inserted: bool,
    pub entity: Option<String>,
    /// The thread's revision once the event was written; `None` for an
    /// event that writes nothing.
    pub revision: Option<i64>,
}

impl Stored {
    fn row(id: i64) -> Self {
        Stored {
            item_id: Some(id),
            inserted: true,
            ..Stored::default()
        }
    }
}

/// Fold one event into the tables. Returns the id of the row it inserted,
/// when it inserted one, so the frontend can key the timeline on it.
pub async fn apply(
    pool: &SqlitePool,
    thread_id: i64,
    ev: &HarnessEvent,
    owner: Option<&Owner>,
) -> Result<Option<i64>, String> {
    apply_event(pool, thread_id, ev, owner)
        .await
        .map(|s| s.item_id)
}

/// [`apply`], saying whether the row is new. A new row belongs to `owner`,
/// the run the consumer has open on this thread. One transaction: what the
/// event writes and the thread's raised revision land together, so a reload
/// that reads the rows reads the revision that covers them.
pub async fn apply_event(
    pool: &SqlitePool,
    thread_id: i64,
    ev: &HarnessEvent,
    owner: Option<&Owner>,
) -> Result<Stored, String> {
    apply_with_draft(pool, thread_id, ev, owner, None).await
}

/// [`apply_event`], writing a run's stream draft in the same transaction:
/// a message that becomes a row leaves the draft as the row lands, so no
/// snapshot holds its text twice, or not at all. `draft` is the run and
/// what is left of its draft.
pub async fn apply_with_draft(
    pool: &SqlitePool,
    thread_id: i64,
    ev: &HarnessEvent,
    owner: Option<&Owner>,
    draft: Option<(i64, &str)>,
) -> Result<Stored, String> {
    if !writes(ev) {
        return Ok(Stored::default());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let mut stored = apply_rows(&mut tx, thread_id, ev, owner).await?;
    if stored.inserted {
        stored.entity = owner.map(|o| o.entity.clone());
    }
    if let Some((run_id, text)) = draft {
        super::runs::write_draft(&mut tx, run_id, text).await?;
    }
    stored.revision = Some(super::threads::raise_revision(&mut tx, thread_id).await?);
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(stored)
}

/// Whether an event changes a table. A streamed delta is only gathered
/// into its run's draft (`app/drafts.rs`), and the rest are the consumer's
/// own announcements of rows already written.
pub fn writes(ev: &HarnessEvent) -> bool {
    !matches!(
        ev,
        HarnessEvent::AssistantDelta { .. }
            | HarnessEvent::ThinkingDelta { .. }
            | HarnessEvent::ToolOutputDelta { .. }
            | HarnessEvent::Queue { .. }
            | HarnessEvent::StopRequested { .. }
            | HarnessEvent::StopUnresponsive { .. }
            | HarnessEvent::Proposal { .. }
            | HarnessEvent::CardSettled { .. }
            | HarnessEvent::Rewound { .. }
            | HarnessEvent::RunSettled { .. }
            | HarnessEvent::RunSelected { .. }
            | HarnessEvent::Selected { .. }
            | HarnessEvent::Handoff { .. }
            | HarnessEvent::DeliveryNotice { .. }
            | HarnessEvent::RateLimits { .. }
            | HarnessEvent::TurnStarted { .. }
            | HarnessEvent::RequestAsked { .. }
            | HarnessEvent::RequestOpened { .. }
            | HarnessEvent::RequestUpdated { .. }
            | HarnessEvent::RequestRetired { .. }
            | HarnessEvent::PermissionMode { .. }
            | HarnessEvent::Exited { .. }
    )
}

async fn apply_rows(
    conn: &mut SqliteConnection,
    thread_id: i64,
    ev: &HarnessEvent,
    owner: Option<&Owner>,
) -> Result<Stored, String> {
    match ev {
        HarnessEvent::SessionStarted {
            provider_session_id,
            model,
            ..
        } => {
            // The thread's model is the student's choice for the next send;
            // a chat that never named one learns it here. What the CLI says
            // it is running belongs to the run it answers.
            sqlx::query(
                "UPDATE harness_threads SET provider_session_id = ?2, model = COALESCE(model, ?3) WHERE id = ?1",
            )
            .bind(thread_id)
            .bind(provider_session_id)
            .bind(model)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
            if let (Some(owner), Some(model)) = (owner, model) {
                sqlx::query(
                    "UPDATE harness_runs SET resolved_model = ?2 WHERE id = ?1 AND model IS NOT ?2",
                )
                .bind(owner.run_id)
                .bind(model)
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
            }
            Ok(Stored::default())
        }
        // `store::begin` wrote this question with its run, and set the
        // thread running; the consumer makes no question without one.
        HarnessEvent::UserMessage { run, .. } => run
            .map(|run| Stored {
                item_id: Some(run.item_id),
                ..Stored::default()
            })
            .ok_or_else(|| "a question with no run".to_string()),
        HarnessEvent::Continued { run, .. } => run
            .map(|run| Stored {
                item_id: Some(run.item_id),
                ..Stored::default()
            })
            .ok_or_else(|| "a continuation with no run".to_string()),
        // `store::begin` or `UnpromptedTurn` already set the thread running.
        HarnessEvent::TurnStarted { .. } => Ok(Stored::default()),
        // A row, so a reload still shows why the agent went on unasked.
        HarnessEvent::UnpromptedTurn { after } => {
            set_status(conn, thread_id, "running").await?;
            let meta = serde_json::json!({ "after": after }).to_string();
            insert_item(conn, thread_id, "unprompted", None, None, Some(meta), owner)
                .await
                .map(Stored::row)
        }
        HarnessEvent::TurnAnchor { anchor } => {
            // The newest question (or continuation) is this turn's: one turn
            // per thread runs at a time.
            sqlx::query(
                "UPDATE harness_items SET anchor = ?2 WHERE id =
                   (SELECT id FROM harness_items
                     WHERE thread_id = ?1 AND kind IN ('user', 'continuation') ORDER BY id DESC LIMIT 1)",
            )
            .bind(thread_id)
            .bind(anchor)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
            Ok(Stored::default())
        }
        HarnessEvent::AssistantMessage { text, source } => {
            text_row(
                conn,
                thread_id,
                "assistant",
                text,
                None,
                source.as_ref(),
                owner,
            )
            .await
        }
        HarnessEvent::Thinking { text, ms, source } => {
            let meta = ms.map(|ms| serde_json::json!({ "ms": ms }).to_string());
            text_row(
                conn,
                thread_id,
                "thinking",
                text,
                meta,
                source.as_ref(),
                owner,
            )
            .await
        }
        HarnessEvent::DeliveryGap { source, cause } => {
            sqlx::query(
                "INSERT INTO harness_delivery_gaps (thread_id, run_id, attempt_id, source_ref, cause)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(thread_id)
            .bind(owner.map(|o| o.run_id))
            .bind(owner.map(|o| o.attempt_id))
            .bind(source)
            .bind(cause.as_str())
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
            Ok(Stored::default())
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
                conn,
                thread_id,
                "tool",
                Some(id),
                Some(title),
                serde_json::to_string(&meta).ok(),
                owner,
            )
            .await
            .map(Stored::row)
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
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
            let Some(row) = row else {
                return Ok(Stored::default());
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
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
            Ok(Stored::default())
        }
        // `meta` records which provider's credentials failed, or that its usage
        // limit is spent, so a reload can redraw the sign-in card or the notice.
        HarnessEvent::Error {
            message,
            auth,
            limit,
        } => {
            let meta = match (auth, limit) {
                (None, false) => None,
                (auth, limit) => {
                    let mut meta = serde_json::Map::new();
                    if let Some(p) = auth {
                        meta.insert("auth".into(), p.as_str().into());
                    }
                    if *limit {
                        meta.insert("limit".into(), true.into());
                    }
                    Some(serde_json::Value::Object(meta).to_string())
                }
            };
            insert_item(conn, thread_id, "error", None, Some(message), meta, owner)
                .await
                .map(Stored::row)
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
                conn,
                thread_id,
                "permission",
                None,
                target.as_deref(),
                Some(meta.to_string()),
                owner,
            )
            .await
            .map(Stored::row)
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
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
            // The session's own occupancy: what a handoff into it has to
            // fit beside (`handoff/budget.rs`).
            if let Some(owner) = owner {
                super::bindings::report_usage(conn, owner.run_id, *context_tokens, *context_window)
                    .await?;
            }
            Ok(Stored::default())
        }
        HarnessEvent::ThreadTitled { title } => {
            sqlx::query("UPDATE harness_threads SET title = ?2 WHERE id = ?1")
                .bind(thread_id)
                .bind(title)
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Stored::default())
        }
        HarnessEvent::TurnFinished { status, .. } => {
            // A run's settlement sets the thread's status with its own; only
            // a turn with no run (one the provider started) sets it here.
            if owner.is_none() {
                let s = if status == "failed" { "error" } else { "idle" };
                set_status(conn, thread_id, s).await?;
            }
            // A stopped turn leaves a mark, so a cut-off answer reads as stopped.
            if status == "interrupted" {
                return insert_item(conn, thread_id, "interrupted", None, None, None, owner)
                    .await
                    .map(Stored::row);
            }
            Ok(Stored::default())
        }
        HarnessEvent::Exited { .. } => {
            // A mid-turn exit already produced a failed TurnFinished.
            Ok(Stored::default())
        }
        // One row per compaction: a re-sent snapshot of it adds none.
        HarnessEvent::Compaction { source } => {
            if let Some(source) = source {
                let known: Option<i64> = sqlx::query_scalar(
                    "SELECT id FROM harness_items WHERE thread_id = ?1 AND kind = 'compaction' AND ref_id = ?2",
                )
                .bind(thread_id)
                .bind(source)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
                if let Some(id) = known {
                    return Ok(Stored {
                        item_id: Some(id),
                        ..Stored::default()
                    });
                }
            }
            insert_item(conn, thread_id, "compaction", source.as_deref(), None, None, owner)
                .await
                .map(Stored::row)
        }
        // No rows: a queued message has none until sent, and a rewind already deleted its.
        HarnessEvent::AssistantDelta { .. }
        | HarnessEvent::ThinkingDelta { .. }
        | HarnessEvent::ToolOutputDelta { .. }
        | HarnessEvent::Queue { .. }
        | HarnessEvent::StopRequested { .. }
        | HarnessEvent::StopUnresponsive { .. }
        | HarnessEvent::Proposal { .. }
        | HarnessEvent::CardSettled { .. }
        | HarnessEvent::Rewound { .. }
        // The consumer writes and emits these itself, at settlement.
        | HarnessEvent::RunSettled { .. }
        | HarnessEvent::RunSelected { .. }
        | HarnessEvent::Selected { .. }
        | HarnessEvent::Handoff { .. }
        | HarnessEvent::DeliveryNotice { .. }
        | HarnessEvent::RateLimits { .. }
        // Saved by the consumer through `requests::store`, which owns the table.
        | HarnessEvent::RequestAsked { .. }
        | HarnessEvent::RequestOpened { .. }
        | HarnessEvent::RequestUpdated { .. }
        | HarnessEvent::RequestRetired { .. }
        | HarnessEvent::PermissionMode { .. } => Ok(Stored::default()),
    }
}

/// A message or reasoning row. Within a run, the same provider source again
/// (a re-sent or closing snapshot) replaces its row's text instead of adding one.
async fn text_row(
    conn: &mut SqliteConnection,
    thread_id: i64,
    kind: &'static str,
    text: &str,
    meta: Option<String>,
    source: Option<&Source>,
    owner: Option<&Owner>,
) -> Result<Stored, String> {
    let (Some(source), Some(owner)) = (source, owner) else {
        return insert_item(conn, thread_id, kind, None, Some(text), meta, owner)
            .await
            .map(Stored::row);
    };
    let lifecycle = if source.partial {
        "partial"
    } else {
        "complete"
    };
    let phase = (kind == "assistant").then(|| source.phase.as_str());
    let known = sqlx::query(
        "SELECT id, entity_ref FROM harness_items WHERE attempt_id = ?1 AND kind = ?2 AND source_ref = ?3",
    )
    .bind(owner.attempt_id)
    .bind(kind)
    .bind(&source.id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;
    if let Some(row) = known {
        let id: i64 = row.get("id");
        sqlx::query(
            "UPDATE harness_items SET content = ?2, meta = COALESCE(?3, meta), native_phase = ?4, lifecycle = ?5
              WHERE id = ?1",
        )
        .bind(id)
        .bind(text)
        .bind(meta)
        .bind(phase)
        .bind(lifecycle)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
        return Ok(Stored {
            item_id: Some(id),
            entity: row.get("entity_ref"),
            ..Stored::default()
        });
    }
    sqlx::query(
        "INSERT INTO harness_items
           (thread_id, kind, content, meta, run_id, attempt_id, entity_ref, source_ref, native_phase, lifecycle)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )
    .bind(thread_id)
    .bind(kind)
    .bind(text)
    .bind(meta)
    .bind(owner.run_id)
    .bind(owner.attempt_id)
    .bind(&owner.entity)
    .bind(&source.id)
    .bind(phase)
    .bind(lifecycle)
    .execute(&mut *conn)
    .await
    .map(|r| Stored::row(r.last_insert_rowid()))
    .map_err(|e| e.to_string())
}
