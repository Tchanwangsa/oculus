//! The Claude Code bridge: one long-lived `claude -p` process per thread.
//!
//! Stream-json both ways: a user turn is one JSON line on stdin; stdout
//! carries stream events for live text, an `assistant` message per block, a
//! `user` message per tool result, and a `result` closing each turn. The
//! process stays up between turns; a thread whose process has gone resumes
//! with `--resume <session id>`.
//!
//! `--permission-prompts none` auto-denies, because under `-p` an unanswered
//! prompt hangs the turn — so containment is settled up front in
//! `settings_json`. See docs/harness.md.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use super::child::{self, str_of, ChildProc, ThreadSpawn};
use super::event::{cap_output, classify, HarnessEvent, Provider, RateWindow};
use super::protected::{LIBRARY_DIRS, ROOT_FILE_GLOBS, WORKSPACE_DIRS};
use super::Sink;

pub struct ClaudeSpawn {
    pub base: ThreadSpawn,
    /// The discovered `oculus` binary, allowed by absolute path as well as name.
    pub oculus: Option<PathBuf>,
    /// `default`, `acceptEdits`, `plan`, `bypassPermissions`.
    pub permission_mode: String,
    /// Appended to the CLI's own system prompt.
    pub system_append: String,
    /// A turn outside any thread (`Harness::one_off`): no tools, skills or MCP
    /// servers, and no session kept in the student's `claude --resume` list.
    pub one_off: bool,
}

pub struct ClaudeSession {
    proc: ChildProc,
    request_ids: AtomicU64,
    /// Set between asking the CLI to stop and the `result` that answers; the
    /// `result` line does not say plainly that it was interrupted.
    interrupting: Arc<AtomicBool>,
    /// Control requests (only `rewind`) waiting on their `control_response`.
    pending: Mutex<HashMap<String, mpsc::Sender<Value>>>,
    /// Set between a message going in and the `result` that closes its turn,
    /// so a process that dies before its first stream event (a rejected model
    /// name) still emits the `TurnFinished` the manager's queue waits on.
    expecting: Arc<AtomicBool>,
}

impl ClaudeSession {
    pub fn spawn(cfg: ClaudeSpawn, sink: Sink) -> Result<Arc<Self>, String> {
        let base = cfg.base;
        let mut cmd = Command::new(&base.bin);
        cmd.arg("-p")
            .args(["--input-format", "stream-json"])
            .args(["--output-format", "stream-json"])
            .arg("--verbose")
            .arg("--include-partial-messages")
            .args(["--permission-mode", &cfg.permission_mode])
            .args(["--permission-prompts", "none"]);
        if let Some(m) = &base.model {
            cmd.args(["--model", m]);
        }
        if let Some(e) = &base.effort {
            cmd.args(["--effort", e]);
        }
        if let Some(id) = &base.resume {
            cmd.args(["--resume", id]);
        }
        if !cfg.system_append.trim().is_empty() {
            cmd.args(["--append-system-prompt", &cfg.system_append]);
        }
        if cfg.one_off {
            cmd.args(["--tools", ""])
                .arg("--disable-slash-commands")
                .arg("--strict-mcp-config")
                .arg("--no-session-persistence");
        }
        cmd.args(["--add-dir", &base.library.display().to_string()]);
        cmd.args([
            "--settings",
            &settings_json(&base.library, &base.cwd, cfg.oculus.as_deref()),
        ]);
        cmd.current_dir(&base.cwd)
            .env_clear()
            .envs(base.env.iter().map(|(k, v)| (k, v)))
            // The CLI gates some behaviour on its entrypoint.
            .env("CLAUDE_CODE_ENTRYPOINT", "cli");
        let (proc, stdout) = ChildProc::spawn("claude", &mut cmd, true)?;

        let interrupting = Arc::new(AtomicBool::new(false));
        let expecting = Arc::new(AtomicBool::new(false));
        let session = Arc::new(ClaudeSession {
            proc,
            request_ids: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            interrupting: interrupting.clone(),
            expecting: expecting.clone(),
        });

        let reader_session = session.clone();
        let raw_log = base.raw_log;
        std::thread::spawn(move || {
            let mut state = Translator {
                interrupting,
                expecting: expecting.clone(),
                ..Default::default()
            };
            child::read_json_lines(stdout, raw_log.as_ref(), |v| {
                if v.get("type").and_then(|t| t.as_str()) == Some("control_response") {
                    reader_session.settle(&v);
                    return;
                }
                for ev in state.translate(&v) {
                    sink(ev);
                }
            });
            reader_session.proc.finish(&sink, Provider::Claude, || {
                expecting.swap(false, Ordering::SeqCst) || state.turn_open
            });
        });

        Ok(session)
    }

    pub fn is_alive(&self) -> bool {
        self.proc.is_alive()
    }

    /// One user turn. A line sent mid-turn is queued by the CLI itself, which
    /// is why the manager holds messages back until `TurnFinished`.
    pub fn send(&self, text: &str) -> Result<(), String> {
        self.expecting.store(true, Ordering::SeqCst);
        self.proc.write_line(&serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": text },
            "parent_tool_use_id": null,
            "session_id": "",
        }))
    }

    /// Stop the current turn without ending the session. The CLI emits the
    /// partial answer as an ordinary `assistant` line, then a `result` that
    /// calls itself an error — the flag tells that apart from a real failure.
    pub fn interrupt(&self) -> Result<(), String> {
        self.interrupting.store(true, Ordering::SeqCst);
        let id = self.request_ids.fetch_add(1, Ordering::SeqCst);
        self.proc.write_line(&serde_json::json!({
            "type": "control_request",
            "request_id": format!("oculus-{id}"),
            "request": { "subtype": "interrupt" },
        }))
    }

    /// Hand a `control_response` to its waiter; unawaited ones are dropped.
    fn settle(&self, v: &Value) {
        let Some(id) = v.pointer("/response/request_id").and_then(|s| s.as_str()) else {
            return;
        };
        if let Some(tx) = self.pending.lock().unwrap().remove(id) {
            let _ = tx.send(v.clone());
        }
    }

    /// Drop a question and everything after it from the CLI's own session.
    /// `target_message_uuid` never appears on stdout; it comes from the
    /// transcript ([`anchor_for`]). `last_seen` is the newest question's: with
    /// it missing the CLI refuses (`stale_target`) to cut past any later turn.
    /// Waits for the answer, because the rows are deleted on the strength of it.
    pub fn rewind(&self, target_message_uuid: &str, last_seen: Option<&str>) -> Result<(), String> {
        let id = format!("oculus-{}", self.request_ids.fetch_add(1, Ordering::SeqCst));
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id.clone(), tx);
        let sent = self.proc.write_line(&serde_json::json!({
            "type": "control_request",
            "request_id": id,
            "request": {
                "subtype": "rewind_conversation",
                "target_message_uuid": target_message_uuid,
                "last_seen_user_message_uuid": last_seen,
                // The manager only rewinds between turns.
                "interrupt_if_running": false,
            },
        }));
        let answer = sent.and_then(|()| {
            rx.recv_timeout(Duration::from_secs(30))
                .map_err(|_| "claude did not answer the rewind".to_string())
        });
        // A failed write or a timeout would otherwise leave the id behind.
        self.pending.lock().unwrap().remove(&id);
        let v = answer?;
        if v.pointer("/response/subtype").and_then(|s| s.as_str()) != Some("success") {
            let why = v
                .pointer("/response/error")
                .and_then(|e| e.as_str())
                .unwrap_or("refused");
            return Err(format!("claude would not rewind: {why}"));
        }
        if v.pointer("/response/response/rewound")
            .and_then(|b| b.as_bool())
            == Some(false)
        {
            return Err("claude found nothing to rewind to".into());
        }
        Ok(())
    }

    pub fn kill(&self) {
        self.proc.kill();
    }
}

/// One row of the CLI's `/model` catalogue, as `initialize` reports it. Raw
/// on purpose: `claudeAsModels` in `app/src/lib/harness.ts` adapts it.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// An alias (`sonnet`, `opus[1m]`, `default`) or a full name.
    pub value: String,
    /// The concrete model the alias stands for today.
    pub resolved_model: String,
    pub display_name: String,
    pub description: String,
    /// Empty for a model that takes no `--effort` (Haiku).
    pub supported_effort_levels: Vec<String>,
}

/// For a CLI wedged on a login prompt or an update, not a slow one.
const MODELS_TIMEOUT: Duration = Duration::from_secs(20);

/// Ask the installed CLI which models it offers, without starting a turn or
/// making an API call: the catalogue rides the answer to the stream-json
/// `initialize` control request. A throwaway, inert process (no hooks, no
/// MCP, no session persisted), killed afterwards since it waits for input.
pub fn list_models(
    bin: &Path,
    cwd: &Path,
    env: &[(String, String)],
) -> Result<Vec<ModelInfo>, String> {
    const REQUEST_ID: &str = "oculus-models";
    let mut cmd = Command::new(bin);
    cmd.arg("-p")
        .args(["--input-format", "stream-json"])
        .args(["--output-format", "stream-json"])
        .arg("--verbose")
        .arg("--no-session-persistence")
        .arg("--strict-mcp-config")
        .args(["--settings", r#"{"disableAllHooks":true}"#])
        .current_dir(cwd)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .env("CLAUDE_CODE_ENTRYPOINT", "cli");
    // stdin stays open until the kill: an early EOF may make the CLI leave
    // unanswered.
    let (proc, stdout) = ChildProc::spawn("claude", &mut cmd, true)?;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(answer) = models_from_response(&v, REQUEST_ID) {
                let _ = tx.send(Some(answer));
                return;
            }
        }
        let _ = tx.send(None);
    });

    let written = proc.write_line(&serde_json::json!({
        "type": "control_request",
        "request_id": REQUEST_ID,
        "request": { "subtype": "initialize" },
    }));
    let answer = written.and_then(|()| {
        rx.recv_timeout(MODELS_TIMEOUT).map_err(|_| {
            format!(
                "claude did not list its models within {}s",
                MODELS_TIMEOUT.as_secs()
            )
        })
    });

    let code = proc.kill();
    match answer? {
        Some(result) => result,
        None => Err(proc.with_tail(format!(
            "claude exited (code {code:?}) before listing its models"
        ))),
    }
}

/// The catalogue out of one stdout line, or None when it is not the answer to
/// `request_id`. A missing field reads as empty; a row with no name is skipped.
fn models_from_response(v: &Value, request_id: &str) -> Option<Result<Vec<ModelInfo>, String>> {
    if v.get("type").and_then(|t| t.as_str()) != Some("control_response")
        || v.pointer("/response/request_id").and_then(|s| s.as_str()) != Some(request_id)
    {
        return None;
    }
    if v.pointer("/response/subtype").and_then(|s| s.as_str()) == Some("error") {
        let why = v
            .pointer("/response/error")
            .and_then(|e| e.as_str())
            .unwrap_or("refused");
        return Some(Err(format!("claude would not initialize: {why}")));
    }
    let Some(rows) = v
        .pointer("/response/response/models")
        .and_then(|m| m.as_array())
    else {
        return Some(Err("claude's initialize answer has no models".into()));
    };
    Some(Ok(rows
        .iter()
        .map(|m| ModelInfo {
            value: str_of(m, "value"),
            resolved_model: str_of(m, "resolvedModel"),
            display_name: str_of(m, "displayName"),
            description: str_of(m, "description"),
            supported_effort_levels: m
                .get("supportedEffortLevels")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .filter(|m| !m.value.is_empty() || !m.resolved_model.is_empty())
        .collect()))
}

/// The inline `--settings` document; docs/harness.md explains each part.
/// `//` prefixes an absolute path in the CLI's rule syntax. Root files are
/// denied by suffix, because a bare `*` also denied everything under `agents/`.
fn settings_json(library: &Path, cwd: &Path, oculus: Option<&Path>) -> String {
    let root = library.display().to_string();
    let abs = root.trim_start_matches('/');
    // Never `oculus.db*`: the CLI merges `Edit(...)` denies into the
    // sandbox's `denyWrite`, which would cancel `allowWrite` below.
    let mut deny: Vec<String> = LIBRARY_DIRS
        .iter()
        .map(|d| format!("{d}/**"))
        .chain(ROOT_FILE_GLOBS.iter().map(|g| g.to_string()))
        .chain(WORKSPACE_DIRS.iter().map(|d| format!("agents/{d}/**")))
        .map(|p| format!("Edit(//{abs}/{p})"))
        .collect();
    // The database is OS-writable, so the CLI must stay the only door to it.
    deny.push("Bash(sqlite3:*)".to_string());

    // `autoAllowBashIfSandboxed` misses commands its analyser cannot vouch
    // for (multi-line, loops), and the resulting denial sticks for the session.
    let mut allow = vec!["Bash(oculus:*)".to_string()];
    // The rule matches the command name, so the full path needs its own.
    if let Some(cli) = oculus {
        allow.push(format!("Bash({}:*)", cli.display()));
    }

    // `agents/` plus the database's three files (`paths::db_write_paths`).
    let write: Vec<String> = std::iter::once(cwd.display().to_string())
        .chain(
            crate::paths::db_write_paths(library)
                .iter()
                .map(|p| p.display().to_string()),
        )
        .collect();
    // `oculus` reaches its keys only through keyd's socket; without this the
    // sandbox refuses the connect (EPERM).
    let keyd = crate::paths::keyd_socket_path(library)
        .display()
        .to_string();
    serde_json::json!({
        "permissions": { "allow": allow, "deny": deny },
        "sandbox": {
            "enabled": true,
            "failIfUnavailable": false,
            "autoAllowBashIfSandboxed": true,
            "allowUnsandboxedCommands": false,
            "network": { "allowLocalBinding": true, "allowUnixSockets": [keyd] },
            "filesystem": { "allowWrite": write },
        },
        "autoMemoryEnabled": false,
    })
    .to_string()
}

// ── Translation ──────────────────────────────────────────────────────────────

/// Per-process translation state. `started_tools` dedupes: a tool_use block
/// can appear in more than one `assistant` line of the same message.
#[derive(Default)]
struct Translator {
    started_tools: std::collections::HashSet<String>,
    /// Between the first stream event of a turn and its `result`.
    turn_open: bool,
    saw_result: bool,
    /// What the last request occupied; `result.usage` sums the turn (spend).
    last_context_tokens: Option<u64>,
    interrupting: Arc<AtomicBool>,
    expecting: Arc<AtomicBool>,
    /// From the `init` line; together they locate the transcript.
    session_id: String,
    cwd: String,
    /// The open turn's first `assistant` uuid — see [`anchor_for`].
    turn_first_assistant: Option<String>,
}

impl Translator {
    fn open_turn(&mut self, out: &mut Vec<HarnessEvent>) {
        if !self.turn_open {
            self.turn_open = true;
            out.push(HarnessEvent::TurnStarted);
        }
    }

    fn translate(&mut self, v: &Value) -> Vec<HarnessEvent> {
        let mut out = Vec::new();
        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match ty {
            "system" => {
                if v.get("subtype").and_then(|s| s.as_str()) == Some("init") {
                    self.session_id = str_of(v, "session_id");
                    self.cwd = str_of(v, "cwd");
                    out.push(HarnessEvent::SessionStarted {
                        provider_session_id: str_of(v, "session_id"),
                        model: v.get("model").and_then(|m| m.as_str()).map(String::from),
                        cwd: str_of(v, "cwd"),
                    });
                }
            }
            "stream_event" => {
                let Some(ev) = v.get("event") else { return out };
                let et = ev.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match et {
                    "message_start" => self.open_turn(&mut out),
                    "content_block_start" => {
                        // A start can carry text the deltas do not repeat.
                        if let Some(cb) = ev.get("content_block") {
                            match cb.get("type").and_then(|t| t.as_str()) {
                                Some("text") => {
                                    let t = str_of(cb, "text");
                                    if !t.is_empty() {
                                        self.open_turn(&mut out);
                                        out.push(HarnessEvent::AssistantDelta { text: t });
                                    }
                                }
                                Some("thinking") => {
                                    let t = str_of(cb, "thinking");
                                    if !t.is_empty() {
                                        self.open_turn(&mut out);
                                        out.push(HarnessEvent::ThinkingDelta { text: t });
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "content_block_delta" => {
                        if let Some(d) = ev.get("delta") {
                            match d.get("type").and_then(|t| t.as_str()) {
                                Some("text_delta") => {
                                    let t = str_of(d, "text");
                                    if !t.is_empty() {
                                        self.open_turn(&mut out);
                                        out.push(HarnessEvent::AssistantDelta { text: t });
                                    }
                                }
                                Some("thinking_delta") => {
                                    let t = str_of(d, "thinking");
                                    if !t.is_empty() {
                                        self.open_turn(&mut out);
                                        out.push(HarnessEvent::ThinkingDelta { text: t });
                                    }
                                }
                                // Tool inputs arrive whole on the `assistant` line.
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            "assistant" => {
                let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) else {
                    return out;
                };
                if self.turn_first_assistant.is_none() {
                    self.turn_first_assistant =
                        v.get("uuid").and_then(|u| u.as_str()).map(String::from);
                }
                self.open_turn(&mut out);
                if let Some(u) = v.pointer("/message/usage") {
                    let n = |k: &str| u.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
                    let ctx = n("input_tokens")
                        + n("cache_read_input_tokens")
                        + n("cache_creation_input_tokens");
                    if ctx > 0 {
                        self.last_context_tokens = Some(ctx);
                    }
                }
                let mut text = String::new();
                for block in content {
                    match block.get("type").and_then(|t| t.as_str()) {
                        Some("text") => {
                            let t = str_of(block, "text");
                            if !t.is_empty() {
                                if !text.is_empty() {
                                    text.push('\n');
                                }
                                text.push_str(&t);
                            }
                        }
                        Some("thinking") => {
                            let t = str_of(block, "thinking");
                            if !t.trim().is_empty() {
                                out.push(HarnessEvent::Thinking { text: t });
                            }
                        }
                        Some("tool_use") => {
                            let id = str_of(block, "id");
                            if id.is_empty() || !self.started_tools.insert(id.clone()) {
                                continue;
                            }
                            let name = str_of(block, "name");
                            let input = block.get("input").cloned().unwrap_or(Value::Null);
                            let (kind, title) = classify(&name, &input);
                            out.push(HarnessEvent::ToolStarted {
                                id,
                                kind,
                                name,
                                title,
                                input,
                            });
                        }
                        _ => {}
                    }
                }
                let text = text.trim().to_string();
                if !text.is_empty() {
                    out.push(HarnessEvent::AssistantMessage { text });
                }
            }
            "user" => {
                // Only tool results; the user's own text is what we sent.
                let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) else {
                    return out;
                };
                for block in content {
                    if block.get("type").and_then(|t| t.as_str()) != Some("tool_result") {
                        continue;
                    }
                    let id = str_of(block, "tool_use_id");
                    let is_error = block
                        .get("is_error")
                        .and_then(|b| b.as_bool())
                        .unwrap_or(false);
                    let output = tool_result_text(block, v.get("tool_use_result"));
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        ok: !is_error,
                        output: cap_output(&output),
                        title: None,
                    });
                }
            }
            "result" => {
                self.saw_result = true;
                let is_error = v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false);
                let subtype = v.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
                // A stopped turn reports `is_error: true` with an
                // `[ede_diagnostic]` in `errors`; only `terminal_reason:
                // "aborted_streaming"` (or our flag) names it. The rest is fallback.
                let interrupted = self.interrupting.swap(false, Ordering::SeqCst)
                    || v.get("terminal_reason")
                        .and_then(|t| t.as_str())
                        .is_some_and(|t| t.starts_with("aborted"))
                    || matches!(
                        v.get("stop_reason").and_then(|s| s.as_str()),
                        Some("interrupted") | Some("interrupt")
                    )
                    || subtype.contains("interrupt");
                // An interrupted result reports zero usage; don't fold it in.
                if let Some(u) = v.get("usage").filter(|_| !interrupted) {
                    let n = |k: &str| u.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
                    let input = n("input_tokens");
                    let cached = n("cache_read_input_tokens") + n("cache_creation_input_tokens");
                    let context_window =
                        v.get("modelUsage")
                            .and_then(|m| m.as_object())
                            .and_then(|m| {
                                m.values()
                                    .filter_map(|x| x.get("contextWindow")?.as_u64())
                                    .max()
                            });
                    out.push(HarnessEvent::Usage {
                        input_tokens: input + cached,
                        output_tokens: n("output_tokens"),
                        context_tokens: self.last_context_tokens,
                        context_window,
                        cost_usd: v.get("total_cost_usd").and_then(|c| c.as_f64()),
                    });
                }
                if is_error && !interrupted {
                    let msg = v
                        .get("result")
                        .and_then(|r| r.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .or_else(|| {
                            v.get("errors").and_then(|e| e.as_array()).map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                        })
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| format!("claude: {subtype}"));
                    out.push(HarnessEvent::error_for(Provider::Claude, msg));
                }
                let status = if interrupted {
                    "interrupted"
                } else if is_error {
                    "failed"
                } else {
                    "completed"
                };
                // The turn's rows are on disk now, and the next turn would
                // move the landmark `anchor_for` walks back from.
                let first = self.turn_first_assistant.take();
                if let Some(path) = transcript_path(&self.cwd, &self.session_id) {
                    if let Some(anchor) = anchor_for(&path, first.as_deref()) {
                        out.push(HarnessEvent::TurnAnchor { anchor });
                    }
                }
                self.turn_open = false;
                self.expecting.store(false, Ordering::SeqCst);
                out.push(HarnessEvent::TurnFinished {
                    status: status.into(),
                });
            }
            "rate_limit_event" => {
                if let Some(w) = v
                    .pointer("/rate_limit_info/unifiedWindows")
                    .and_then(|w| w.as_object())
                {
                    let mut windows = Vec::new();
                    for (k, win) in w {
                        let label = match k.as_str() {
                            "five_hour" => "5-hour",
                            "seven_day" => "Weekly",
                            "seven_day_opus" => "Weekly Opus",
                            "seven_day_sonnet" => "Weekly Sonnet",
                            other => other,
                        };
                        windows.push(RateWindow {
                            label: label.to_string(),
                            used_percent: win
                                .get("utilization")
                                .and_then(|u| u.as_f64())
                                .unwrap_or(0.0)
                                * 100.0,
                            resets_at: win.get("resetsAt").and_then(|r| r.as_i64()),
                        });
                    }
                    windows.sort_by(|a, b| a.label.cmp(&b.label));
                    out.push(HarnessEvent::RateLimits { windows });
                }
            }
            _ => {}
        }
        out
    }
}

/// Where the CLI keeps a session's transcript: `projects/<cwd slug>/<id>.jsonl`,
/// the slug being the cwd with every non-alphanumeric char turned to `-`
/// (the CLI does not announce it with auto-memory off). Falls back to a
/// search by file name, since session ids are unique.
fn transcript_path(cwd: &str, session_id: &str) -> Option<PathBuf> {
    if cwd.is_empty() || session_id.is_empty() {
        return None;
    }
    let root = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude")))?;
    let projects = root.join("projects");
    let slug: String = cwd
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let name = format!("{session_id}.jsonl");
    let direct = projects.join(&slug).join(&name);
    if direct.exists() {
        return Some(direct);
    }
    std::fs::read_dir(&projects).ok()?.flatten().find_map(|e| {
        let p = e.path().join(&name);
        p.exists().then_some(p)
    })
}

/// The uuid of the question a turn answered. The transcript is a tree by
/// `parentUuid`, so walk up from the turn's first answer to a `user` row that
/// is not a tool result, skipping attachments. With no answer (an interrupted
/// turn) take the newest question, which is this turn's.
fn anchor_for(path: &Path, first_assistant: Option<&str>) -> Option<String> {
    struct Row {
        parent: Option<String>,
        question: bool,
    }
    let file = std::fs::File::open(path).ok()?;
    let mut by_uuid: HashMap<String, Row> = HashMap::new();
    let mut newest_question: Option<String> = None;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(uuid) = v.get("uuid").and_then(|u| u.as_str()) else {
            continue;
        };
        let question = v.get("type").and_then(|t| t.as_str()) == Some("user")
            && v.get("tool_use_result").is_none();
        if question {
            newest_question = Some(uuid.to_string());
        }
        by_uuid.insert(
            uuid.to_string(),
            Row {
                parent: v
                    .get("parentUuid")
                    .and_then(|p| p.as_str())
                    .map(String::from),
                question,
            },
        );
    }
    let Some(start) = first_assistant else {
        return newest_question;
    };
    let mut at = start.to_string();
    // Bounded: a malformed parent chain must not loop forever.
    for _ in 0..by_uuid.len() {
        let row = by_uuid.get(&at)?;
        if row.question {
            return Some(at);
        }
        at = row.parent.clone()?;
    }
    None
}

/// A tool result's text: the structured `tool_use_result` (stdout + stderr
/// for Bash) when the CLI attached one, else the content blocks.
fn tool_result_text(block: &Value, structured: Option<&Value>) -> String {
    if let Some(s) = structured {
        if let Some(text) = s.as_str() {
            return text.to_string();
        }
        let stdout = s.get("stdout").and_then(|x| x.as_str());
        let stderr = s.get("stderr").and_then(|x| x.as_str());
        if stdout.is_some() || stderr.is_some() {
            let mut out = stdout.unwrap_or("").to_string();
            if let Some(e) = stderr.filter(|e| !e.is_empty()) {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(e);
            }
            return out;
        }
    }
    match block.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::event::ToolKind;

    #[test]
    fn the_settings_document_opens_the_database_and_nothing_else() {
        let library = Path::new("/Users/x/Library/Application Support/com.tchan.oculus");
        let cwd = library.join("agents");
        let v: serde_json::Value =
            serde_json::from_str(&settings_json(library, &cwd, None)).expect("valid settings JSON");

        let write = v.pointer("/sandbox/filesystem/allowWrite").unwrap();
        assert_eq!(
            write,
            &serde_json::json!([
                "/Users/x/Library/Application Support/com.tchan.oculus/agents",
                "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db",
                "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db-wal",
                "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db-shm",
            ]),
            "the cwd, then the database's three files — nothing else in the library"
        );
        assert_eq!(v["sandbox"]["enabled"], true);
        assert_eq!(v["autoMemoryEnabled"], false);
        assert_eq!(
            v.pointer("/sandbox/network/allowUnixSockets").unwrap(),
            &serde_json::json!(["/Users/x/Library/Application Support/com.tchan.oculus/keyd.sock"]),
            "keyd's socket, and no other"
        );
        assert_eq!(
            v.pointer("/sandbox/network/allowUnixSockets/0").unwrap(),
            keyd_core::paths::socket(library).to_str().unwrap(),
            "the path keyd's endpoint is bound at, not a copy of it"
        );
        assert!(
            v.pointer("/sandbox/network/allowAllUnixSockets").is_none(),
            "one socket is granted, not all of them"
        );

        let deny: Vec<&str> = v
            .pointer("/permissions/deny")
            .and_then(|d| d.as_array())
            .unwrap()
            .iter()
            .filter_map(|r| r.as_str())
            .collect();
        for rule in [
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/courses/**)",
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/agents/skills/**)",
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/agents/.claude/**)",
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/*.cookie)",
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/vault.bin*)",
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/.vault.bin.*)",
            "Bash(sqlite3:*)",
        ] {
            assert!(deny.contains(&rule), "missing {rule} in {deny:?}");
        }
        let writable = v
            .pointer("/sandbox/filesystem/allowWrite")
            .unwrap()
            .to_string();
        assert!(
            !writable.contains("vault"),
            "the vault is never writable: {writable}"
        );

        // An `Edit(...)` deny merges into `denyWrite` and cancels `allowWrite`.
        assert!(
            !deny.iter().any(|r| r.contains("oculus.db")),
            "oculus.db must stay out of deny — it cancels allowWrite: {deny:?}"
        );

        let allow: Vec<&str> = v
            .pointer("/permissions/allow")
            .and_then(|a| a.as_array())
            .unwrap()
            .iter()
            .filter_map(|r| r.as_str())
            .collect();
        assert_eq!(
            allow,
            ["Bash(oculus:*)"],
            "the board's door, and nothing else, is allowed by name"
        );

        let v2: serde_json::Value = serde_json::from_str(&settings_json(
            library,
            &cwd,
            Some(Path::new("/opt/oculus/bin/oculus")),
        ))
        .expect("valid settings JSON");
        let allow2: Vec<&str> = v2
            .pointer("/permissions/allow")
            .and_then(|a| a.as_array())
            .unwrap()
            .iter()
            .filter_map(|r| r.as_str())
            .collect();
        assert_eq!(allow2, ["Bash(oculus:*)", "Bash(/opt/oculus/bin/oculus:*)"]);
    }

    /// The sandbox grant follows the library the thread runs over, and no
    /// `Edit` deny (merged into `denyWrite`) covers the socket it grants.
    #[test]
    fn the_socket_grant_follows_the_library_and_no_deny_covers_it() {
        for lib in [
            "/Users/x/Library/Application Support/com.tchan.oculus",
            "/d",
        ] {
            let library = Path::new(lib);
            let v: serde_json::Value =
                serde_json::from_str(&settings_json(library, &library.join("agents"), None))
                    .unwrap();
            let socket = keyd_core::paths::socket(library);
            assert_eq!(
                v.pointer("/sandbox/network/allowUnixSockets").unwrap(),
                &serde_json::json!([socket.to_str().unwrap()])
            );
            for rule in v
                .pointer("/permissions/deny")
                .and_then(|d| d.as_array())
                .unwrap()
                .iter()
                .filter_map(|r| r.as_str())
                .filter_map(|r| r.strip_prefix("Edit(/").and_then(|r| r.strip_suffix(')')))
            {
                assert!(
                    !crate::harness::protected::glob_covers(rule, socket.to_str().unwrap()),
                    "Edit deny {rule} covers {socket:?}"
                );
            }
        }
    }

    /// A question, its attachments, the answer, then a tool result that is
    /// also a `user` row and must not be mistaken for a question.
    fn transcript(dir: &Path) -> PathBuf {
        let rows = [
            r#"{"type":"user","uuid":"q1","parentUuid":null,"message":{"role":"user","content":"first"}}"#,
            r#"{"type":"assistant","uuid":"a1","parentUuid":"q1"}"#,
            r#"{"type":"user","uuid":"q2","parentUuid":"a1","message":{"role":"user","content":"second"}}"#,
            r#"{"type":"attachment","uuid":"at1","parentUuid":"q2"}"#,
            r#"{"type":"attachment","uuid":"at2","parentUuid":"at1"}"#,
            r#"{"type":"assistant","uuid":"a2","parentUuid":"at2"}"#,
            r#"{"type":"user","uuid":"tr1","parentUuid":"a2","tool_use_result":{"ok":true}}"#,
            r#"{"type":"assistant","uuid":"a3","parentUuid":"tr1"}"#,
        ];
        let path = dir.join("session.jsonl");
        std::fs::write(&path, rows.join("\n")).unwrap();
        path
    }

    #[test]
    fn the_anchor_is_the_question_the_answer_hangs_off() {
        let dir = crate::test_support::Scratch::new("anchor");
        let path = transcript(&dir);

        assert_eq!(anchor_for(&path, Some("a2")).as_deref(), Some("q2"));
        assert_eq!(anchor_for(&path, Some("a1")).as_deref(), Some("q1"));
        assert_eq!(anchor_for(&path, Some("a3")).as_deref(), Some("q2"));
        assert_eq!(anchor_for(&path, None).as_deref(), Some("q2"));
        assert_eq!(anchor_for(&path, Some("nope")), None);
    }

    /// A dotfile in the cwd produces a double dash.
    #[test]
    fn the_transcript_slug_flattens_everything_but_letters_and_digits() {
        let dir = crate::test_support::Scratch::new("slug");
        let projects = dir.join("projects").join("-tmp-a-b--claude-c-d");
        std::fs::create_dir_all(&projects).unwrap();
        std::fs::write(projects.join("sess.jsonl"), "").unwrap();
        std::env::set_var("CLAUDE_CONFIG_DIR", &*dir);

        let found = transcript_path("/tmp/a b/.claude/c_d", "sess");
        assert_eq!(
            found.as_deref(),
            Some(projects.join("sess.jsonl").as_path())
        );
        assert_eq!(transcript_path("/tmp/a b/.claude/c_d", "gone"), None);

        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }

    /// Recorded from `claude 2.1.267` asked to `ls` the library.
    #[test]
    fn folds_a_recorded_session() {
        let raw = include_str!("../../fixtures/harness/claude-ls.ndjson");
        let mut t = Translator::default();
        let events: Vec<HarnessEvent> = raw
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .flat_map(|v| t.translate(&v))
            .collect();

        let session = events.iter().find_map(|e| match e {
            HarnessEvent::SessionStarted {
                provider_session_id,
                ..
            } => Some(provider_session_id.clone()),
            _ => None,
        });
        assert!(session.is_some(), "session id from system/init");

        let tools: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::ToolStarted { kind, title, .. } => Some((*kind, title.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(tools, vec![(ToolKind::Bash, "ls -a".to_string())]);

        let finished = events
            .iter()
            .filter(|e| matches!(e, HarnessEvent::ToolFinished { ok: true, .. }))
            .count();
        assert_eq!(finished, 1);

        let deltas: String = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::AssistantDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        let message = events.iter().find_map(|e| match e {
            HarnessEvent::AssistantMessage { text } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(
            message.as_deref(),
            Some(deltas.as_str()),
            "deltas add up to the message"
        );

        assert!(
            matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "completed")
        );
        assert!(events.iter().any(|e| matches!(
            e,
            HarnessEvent::Usage {
                cost_usd: Some(_),
                ..
            }
        )));
        assert!(events
            .iter()
            .any(|e| matches!(e, HarnessEvent::RateLimits { windows } if windows.len() == 2)));
    }

    /// Recorded: the stop's `result` calls itself an error, with zeroed usage.
    #[test]
    fn a_stopped_turn_is_not_an_error() {
        let raw = include_str!("../../fixtures/harness/claude-interrupt.ndjson");
        let mut t = Translator::default();
        let events: Vec<HarnessEvent> = raw
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .flat_map(|v| t.translate(&v))
            .collect();

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, HarnessEvent::Error { .. })),
            "the CLI's own diagnostic is not something the student did"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, HarnessEvent::Usage { .. })),
            "an interrupted result reports zeros; folding them in blanks the meter"
        );
        let deltas: String = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::AssistantDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        let message = events.iter().find_map(|e| match e {
            HarnessEvent::AssistantMessage { text } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(message.as_deref(), Some(deltas.trim()));
        assert!(
            matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "interrupted")
        );
    }

    /// `initialize` from CLI 2.1.281, cut to its models. Haiku declares no
    /// effort levels and must still arrive.
    #[test]
    fn the_initialize_answer_lists_the_models() {
        let line = r#"{"type":"control_response","response":{"subtype":"success","request_id":"oculus-models","response":{"models":[
            {"value":"default","resolvedModel":"claude-opus-5-5[1m]","displayName":"Default (recommended)","description":"Opus 5.5 with 1M context · Best for everyday, complex tasks","supportsEffort":true,"supportedEffortLevels":["low","medium","high","xhigh","max"]},
            {"value":"claude-fable-5-1[1m]","resolvedModel":"claude-fable-5-1","displayName":"Fable","description":"Fable 5.1 · Most capable for your hardest and longest-running tasks","supportedEffortLevels":["low","medium","high","xhigh","max"]},
            {"value":"haiku","resolvedModel":"claude-haiku-4-5-20251001","displayName":"Haiku","description":"Haiku 4.5 · Fastest for quick answers"},
            {"displayName":"nameless"}
        ]}}}"#;
        let v: Value = serde_json::from_str(line).unwrap();

        assert!(
            models_from_response(&v, "someone-else").is_none(),
            "another request's answer"
        );
        let models = models_from_response(&v, "oculus-models").unwrap().unwrap();
        assert_eq!(
            models.len(),
            3,
            "a row with neither name is dropped, not fatal"
        );
        assert_eq!(models[0].value, "default");
        assert_eq!(models[0].resolved_model, "claude-opus-5-5[1m]");
        assert_eq!(models[0].supported_effort_levels.len(), 5);
        assert_eq!(models[1].value, "claude-fable-5-1[1m]");
        assert_eq!(models[2].resolved_model, "claude-haiku-4-5-20251001");
        assert!(models[2].supported_effort_levels.is_empty());

        let refused: Value = serde_json::from_str(
            r#"{"type":"control_response","response":{"subtype":"error","request_id":"oculus-models","error":"not logged in"}}"#,
        )
        .unwrap();
        assert!(models_from_response(&refused, "oculus-models")
            .unwrap()
            .is_err());
    }

    /// Needs an installed CLI: `cargo test --lib list_models_from_the_real_cli -- --ignored`.
    #[test]
    #[ignore]
    fn list_models_from_the_real_cli() {
        let bin = crate::harness::discover::binary(Provider::Claude).expect("claude on PATH");
        let cwd = std::env::temp_dir();
        let started = std::time::Instant::now();
        let models = list_models(&bin, &cwd, &crate::harness::discover::child_env()).unwrap();
        eprintln!(
            "{} models in {:?}: {models:#?}",
            models.len(),
            started.elapsed()
        );
        assert!(!models.is_empty());
    }
}
