//! The chat agent: a tool loop over the library.
//!
//! The model is given tools that reach the same data the UI does — semantic
//! search over embedded pages, a file's markdown, a subject's file list — and
//! loops until it answers. Everything runs here rather than in the frontend
//! because the API key, the spending ledger, and the retrieval functions are
//! all already on this side; a webview round-trip per turn would buy nothing
//! and lose the conversation on a reload mid-stream.
//!
//! Message rows are written here too, for the same reason: the loop's own
//! tool-call and tool-result turns are re-read on the next turn, so the
//! history has to be authoritative where the loop runs. The frontend reads
//! chats for display and listens to events for the live stream.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Emitter};

use crate::llm::{self, LlmConfig, ModelRef, ProviderConfig};

/// Beyond this the loop stops calling tools and demands an answer. Reached
/// only when the model keeps searching without converging.
const MAX_TOOL_ROUNDS: usize = 8;
/// Tool results are text going straight into the next prompt; without a cap a
/// single long reading pack would blow the context window.
const MAX_TOOL_CHARS: usize = 20_000;

const SYSTEM_PROMPT: &str = "\
You are Oculus, a study assistant with access to the student's own course \
library (Canvas pages, PDFs, lecture slides, Ed Discussion threads).

Answer from the library, not from memory. Use `search_library` first for any \
question about course content; use `read_file` when you need the full text of \
something search surfaced. If the library does not contain the answer, say so \
plainly rather than guessing.

Cite what you used. Link to a source inline with markdown of the form \
[filename](oculus-file://SUBJECT_ID/RELATIVE_PATH?page=N) — the relative path \
and subject id are given in every search result. Keep answers concise and \
concrete; prefer the course's own wording and notation.";

/// Cancel flags per in-flight chat, set by `chat_cancel` and read between
/// stream reads and tool rounds.
#[derive(Default)]
pub struct ChatCancel(pub Arc<Mutex<HashMap<i64, Arc<AtomicBool>>>>);

#[derive(Serialize, Clone)]
struct Citation {
    subject_id: i64,
    relative_path: String,
    filename: String,
    page_no: Option<i64>,
}

// ── Persistence ──────────────────────────────────────────────────────────────

async fn insert_message(
    pool: &SqlitePool,
    chat_id: i64,
    role: &str,
    content: Option<&str>,
    tool_calls: Option<String>,
    tool_call_id: Option<&str>,
    citations: Option<String>,
    model: Option<&str>,
) -> Result<i64, String> {
    let res = sqlx::query(
        "INSERT INTO chat_messages (chat_id, role, content, tool_calls, tool_call_id, citations, model)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(chat_id)
    .bind(role)
    .bind(content)
    .bind(tool_calls)
    .bind(tool_call_id)
    .bind(citations)
    .bind(model)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query("UPDATE chats SET updated_at = datetime('now') WHERE id = ?1")
        .bind(chat_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(res.last_insert_rowid())
}

/// Rebuild the OpenAI-shape history for the next model turn.
async fn history(pool: &SqlitePool, chat_id: i64) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query(
        "SELECT role, content, tool_calls, tool_call_id FROM chat_messages
         WHERE chat_id = ?1 ORDER BY id ASC",
    )
    .bind(chat_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut out = vec![serde_json::json!({ "role": "system", "content": SYSTEM_PROMPT })];
    for r in rows {
        let role: String = r.get("role");
        let content: Option<String> = r.get("content");
        let mut m = serde_json::json!({ "role": role, "content": content.unwrap_or_default() });
        if let Some(tc) = r.get::<Option<String>, _>("tool_calls") {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&tc) {
                m["tool_calls"] = v;
            }
        }
        if let Some(id) = r.get::<Option<String>, _>("tool_call_id") {
            m["tool_call_id"] = id.into();
        }
        out.push(m);
    }
    Ok(out)
}

// ── Tools ────────────────────────────────────────────────────────────────────

fn tool_schemas() -> serde_json::Value {
    serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "search_library",
                "description": "Semantic search over every indexed page of the student's course library. Returns ranked pages with their markdown, filename, subject id and relative path.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Natural-language query." },
                        "subject_id": { "type": "integer", "description": "Restrict to one subject (Canvas course id). Omit to search everything." },
                        "limit": { "type": "integer", "description": "Max results, default 6." }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read the full parsed markdown of one library file, given its relative path from a search result.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "relative_path": { "type": "string" },
                        "offset": { "type": "integer", "description": "Character offset for paging through a long file." }
                    },
                    "required": ["relative_path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_subjects",
                "description": "List the student's subjects with their Canvas course ids and codes.",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_files",
                "description": "List the files held for one subject, with category and parse state.",
                "parameters": {
                    "type": "object",
                    "properties": { "subject_id": { "type": "integer" } },
                    "required": ["subject_id"]
                }
            }
        }
    ])
}

fn truncate(mut s: String) -> String {
    if s.len() > MAX_TOOL_CHARS {
        s.truncate(MAX_TOOL_CHARS);
        s.push_str("\n…[truncated — call read_file with a larger offset for more]");
    }
    s
}

/// Run one tool call. Returns the text handed back to the model, plus any
/// files it surfaced (which become the answer's citation fallback).
async fn run_tool(
    pool: &SqlitePool,
    name: &str,
    args: &serde_json::Value,
) -> (String, Vec<Citation>) {
    let mut cites = Vec::new();
    let text = match name {
        "search_library" => {
            let query = args["query"].as_str().unwrap_or_default().to_string();
            let limit = args["limit"].as_i64().unwrap_or(6);
            let subject_id = args["subject_id"].as_i64();
            let db = crate::paths::db_path(&crate::paths::data_dir());
            match crate::retrieval::search(&db, query, limit, subject_id).await {
                Ok(hits) if hits.is_empty() => "No matching pages in the library.".to_string(),
                Ok(hits) => {
                    let mut out = String::new();
                    for h in &hits {
                        cites.push(Citation {
                            subject_id: h.subject_id,
                            relative_path: h.relative_path.clone(),
                            filename: h.filename.clone(),
                            page_no: Some(h.page_no),
                        });
                        out.push_str(&format!(
                            "## {} — page {} (subject_id {}, relative_path {})\nscore {:.3}\n\n{}\n\n",
                            h.filename, h.page_no, h.subject_id, h.relative_path, h.score, h.markdown
                        ));
                    }
                    truncate(out)
                }
                Err(e) => format!("search failed: {e}"),
            }
        }
        "read_file" => {
            let rel = args["relative_path"].as_str().unwrap_or_default();
            let offset = args["offset"].as_u64().unwrap_or(0) as usize;
            match crate::files::read_parsed_markdown(rel) {
                Ok(text) => {
                    let slice = text.chars().skip(offset).collect::<String>();
                    truncate(slice)
                }
                Err(e) => format!("could not read {rel}: {e}"),
            }
        }
        "list_subjects" => {
            match sqlx::query("SELECT id, code, name FROM subjects WHERE selected = 1 ORDER BY code")
                .fetch_all(pool)
                .await
            {
                Ok(rows) => rows
                    .iter()
                    .map(|r| {
                        format!(
                            "{} — {} (subject_id {})",
                            r.get::<String, _>("code"),
                            r.get::<String, _>("name"),
                            r.get::<i64, _>("id")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                Err(e) => format!("query failed: {e}"),
            }
        }
        "list_files" => {
            let sid = args["subject_id"].as_i64().unwrap_or(-1);
            match sqlx::query(
                "SELECT filename, relative_path, category, parse_status FROM files
                 WHERE subject_id = ?1 ORDER BY category, filename",
            )
            .bind(sid)
            .fetch_all(pool)
            .await
            {
                Ok(rows) => truncate(
                    rows.iter()
                        .map(|r| {
                            format!(
                                "{} [{}] {} ({})",
                                r.get::<String, _>("filename"),
                                r.get::<Option<String>, _>("category").unwrap_or_default(),
                                r.get::<String, _>("relative_path"),
                                r.get::<Option<String>, _>("parse_status")
                                    .unwrap_or_else(|| "unparsed".into())
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                Err(e) => format!("query failed: {e}"),
            }
        }
        other => format!("unknown tool: {other}"),
    };
    (text, cites)
}

// ── The loop ─────────────────────────────────────────────────────────────────

async fn run_loop(
    app: AppHandle,
    pool: SqlitePool,
    cfg: LlmConfig,
    provider: ProviderConfig,
    model: String,
    chat_id: i64,
    cancel: Arc<AtomicBool>,
) -> Result<(), String> {
    let tools = tool_schemas();
    let mut collected: Vec<Citation> = Vec::new();

    for round in 0..=MAX_TOOL_ROUNDS {
        if cancel.load(Ordering::SeqCst) {
            return Ok(());
        }
        // Checked every turn, not once per message: a long tool loop must stop
        // at the cap rather than run past it.
        llm::check_budget(&pool, &cfg).await?;

        let msgs = serde_json::Value::Array(history(&pool, chat_id).await?);
        let last_round = round == MAX_TOOL_ROUNDS;

        let app2 = app.clone();
        let provider2 = provider.clone();
        let model2 = model.clone();
        let tools2 = tools.clone();
        let cancel2 = Arc::clone(&cancel);
        let outcome = tauri::async_runtime::spawn_blocking(move || {
            // On the last round the tools are withheld, forcing prose.
            let t = if last_round { None } else { Some(&tools2) };
            llm::chat_completion_stream(&provider2, &model2, &msgs, t, |delta| {
                if !cancel2.load(Ordering::SeqCst) {
                    app2.emit(
                        "chat-delta",
                        serde_json::json!({ "chatId": chat_id, "delta": delta }),
                    )
                    .ok();
                }
            })
        })
        .await
        .map_err(|e| e.to_string())??;

        llm::record_usage(&pool, &provider.id, &model, "chat", &outcome.usage, Some(chat_id))
            .await?;

        let calls = outcome.message["tool_calls"].as_array().cloned().unwrap_or_default();

        if calls.is_empty() {
            let cites = if collected.is_empty() {
                None
            } else {
                serde_json::to_string(&collected).ok()
            };
            let id = insert_message(
                &pool,
                chat_id,
                "assistant",
                Some(&outcome.content),
                None,
                None,
                cites.clone(),
                Some(&model),
            )
            .await?;
            app.emit(
                "chat-message",
                serde_json::json!({
                    "chatId": chat_id,
                    "message": {
                        "id": id,
                        "role": "assistant",
                        "content": outcome.content,
                        "citations": cites,
                        "model": model,
                    }
                }),
            )
            .ok();
            app.emit("chat-done", serde_json::json!({ "chatId": chat_id })).ok();
            return Ok(());
        }

        // Persist the assistant's tool-call turn before running the tools, so
        // the history stays valid if a tool panics or the app dies mid-round.
        insert_message(
            &pool,
            chat_id,
            "assistant",
            Some(&outcome.content),
            serde_json::to_string(&outcome.message["tool_calls"]).ok(),
            None,
            None,
            Some(&model),
        )
        .await?;

        for call in calls {
            if cancel.load(Ordering::SeqCst) {
                return Ok(());
            }
            let name = call["function"]["name"].as_str().unwrap_or_default().to_string();
            let raw_args = call["function"]["arguments"].as_str().unwrap_or("{}");
            let args: serde_json::Value =
                serde_json::from_str(raw_args).unwrap_or(serde_json::json!({}));
            let call_id = call["id"].as_str().unwrap_or_default().to_string();

            app.emit(
                "chat-tool",
                serde_json::json!({ "chatId": chat_id, "name": name, "args": args, "status": "start" }),
            )
            .ok();

            let (result, cites) = run_tool(&pool, &name, &args).await;
            collected.extend(cites);

            insert_message(&pool, chat_id, "tool", Some(&result), None, Some(&call_id), None, None)
                .await?;

            app.emit(
                "chat-tool",
                serde_json::json!({ "chatId": chat_id, "name": name, "args": args, "status": "done" }),
            )
            .ok();
        }
    }
    Ok(())
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// Send a message. Creates the chat when `chat_id` is absent, writes the user
/// turn, then runs the loop on its own task — the frontend follows the events.
///
/// `model` overrides the configured chat model for this turn; the composer's
/// switcher sends it, so a conversation can change model mid-thread without
/// touching settings. The fallback chain still applies to whatever is picked.
#[tauri::command]
pub async fn chat_send(
    app: AppHandle,
    chat_id: Option<i64>,
    content: String,
    model: Option<ModelRef>,
    cancels: tauri::State<'_, ChatCancel>,
) -> Result<i64, String> {
    let pool = llm::open_pool().await?;
    let cfg = llm::load_config(&pool).await?;
    let preferred = model
        .or_else(|| cfg.chat_model.clone())
        .ok_or("No chat model selected — pick one in Settings → AI")?;
    // Checked here rather than inside the loop: refusing before the user's
    // message is written keeps a doomed turn out of the history entirely.
    let resolved = llm::resolve(&cfg, &preferred)?;

    let chat_id = match chat_id {
        Some(id) => id,
        None => {
            // First line of the first message is a good enough title until
            // something better exists.
            let title: String = content.chars().take(60).collect();
            let res = sqlx::query("INSERT INTO chats (title) VALUES (?1)")
                .bind(&title)
                .execute(&pool)
                .await
                .map_err(|e| e.to_string())?;
            res.last_insert_rowid()
        }
    };

    insert_message(&pool, chat_id, "user", Some(&content), None, None, None, None).await?;

    let flag = Arc::new(AtomicBool::new(false));
    cancels.0.lock().unwrap().insert(chat_id, Arc::clone(&flag));

    tauri::async_runtime::spawn(async move {
        let loop_run = run_loop(
            app.clone(),
            pool,
            cfg,
            resolved.provider,
            resolved.model,
            chat_id,
            flag,
        );
        if let Err(e) = loop_run.await {
            app.emit("chat-error", serde_json::json!({ "chatId": chat_id, "error": e })).ok();
        }
    });

    Ok(chat_id)
}

#[tauri::command]
pub fn chat_cancel(chat_id: i64, cancels: tauri::State<ChatCancel>) -> Result<(), String> {
    if let Some(f) = cancels.0.lock().unwrap().get(&chat_id) {
        f.store(true, Ordering::SeqCst);
    }
    Ok(())
}
