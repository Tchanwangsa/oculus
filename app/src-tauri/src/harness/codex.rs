//! The Codex bridge: JSON-RPC over stdio to `codex app-server`.
//!
//! One app-server process serves every Codex thread; each notification
//! carries a `threadId`. Framing is one JSON object per line: responses echo
//! a numeric `id` with no `method`, notifications have a `method` and no
//! `id`, and a line with both is the server asking *us* something, which must
//! be answered or the turn hangs.
//!
//! Shapes are from `codex app-server` 0.153. `thread/start` takes `sandbox`
//! (a mode string) while `turn/start` takes `sandboxPolicy` (an object), and a
//! resumed thread replays its last turn's token usage first.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

use super::child::{self, str_of as s, ChildProc};
use super::event::{cap_output, classify, HarnessEvent, Provider, RateWindow, ToolKind};
use super::{RawLog, Sink};

/// No answer in this long is a hung server, not a slow one.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub struct CodexSpawn {
    pub bin: PathBuf,
    pub env: Vec<(String, String)>,
    pub raw_log: Option<RawLog>,
    /// Takes the notifications that are about the account rather than a
    /// thread; see `translate_account`.
    pub account_sink: Option<Sink>,
}

/// How to open a thread. `cwd` is also the sandbox's writable root.
pub struct CodexThreadOpts {
    pub cwd: PathBuf,
    /// The database's three files (`paths::db_write_paths`) — files, not
    /// their folder, since the sandbox is Codex's only containment.
    pub writable_files: Vec<PathBuf>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    /// Appended to Codex's own instructions (`developerInstructions`).
    pub instructions: String,
    /// Kept out of the student's `codex resume` list: a one-off turn's thread.
    pub ephemeral: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub reasoning_efforts: Vec<String>,
    pub default_reasoning_effort: Option<String>,
    pub is_default: bool,
}

struct ThreadRoute {
    sink: Sink,
    state: Mutex<ThreadState>,
}

#[derive(Default)]
struct ThreadState {
    active_turn: Option<String>,
    /// Set on resume: the server replays the previous turn's usage first.
    ignore_usage_until_turn: bool,
    /// Items already announced, so a completion arriving first can synthesise
    /// the open.
    open_items: HashMap<String, ToolKind>,
    streamed_messages: std::collections::HashSet<String>,
    /// The answer as it streams; an interrupted turn never sends the
    /// `item/completed` that would commit it.
    partial_message: String,
}

pub struct CodexServer {
    proc: ChildProc,
    next_id: AtomicI64,
    pending: Mutex<HashMap<i64, mpsc::Sender<Result<Value, String>>>>,
    routes: Mutex<HashMap<String, Arc<ThreadRoute>>>,
    /// Where account-scoped events go; absent for headless runs.
    account_sink: Option<Sink>,
}

impl CodexServer {
    pub fn spawn(cfg: CodexSpawn) -> Result<Arc<Self>, String> {
        let mut cmd = Command::new(&cfg.bin);
        cmd.arg("app-server")
            .env_clear()
            .envs(cfg.env.iter().map(|(k, v)| (k, v)));
        // stderr is tracing and the user's MCP noise, kept only as a tail.
        let (proc, stdout) = ChildProc::spawn("codex", &mut cmd, true)?;
        let server = Arc::new(CodexServer {
            proc,
            next_id: AtomicI64::new(1),
            pending: Mutex::new(HashMap::new()),
            routes: Mutex::new(HashMap::new()),
            account_sink: cfg.account_sink,
        });

        let reader = server.clone();
        let raw_log = cfg.raw_log;
        std::thread::spawn(move || {
            child::read_json_lines(stdout, raw_log.as_ref(), |v| reader.dispatch(v));
            let code = reader.proc.reap();
            reader.pending.lock().unwrap().clear();
            let routes: Vec<Arc<ThreadRoute>> = reader
                .routes
                .lock()
                .unwrap()
                .drain()
                .map(|(_, r)| r)
                .collect();
            for r in routes {
                let mid_turn = r.state.lock().unwrap().active_turn.take().is_some();
                if mid_turn {
                    let msg = reader
                        .proc
                        .with_tail(format!("codex app-server exited (code {code:?})"));
                    child::fail_turn(&r.sink, Provider::Codex, msg);
                }
                (r.sink)(HarnessEvent::Exited { code });
            }
        });

        server.initialize()?;
        Ok(server)
    }

    pub fn is_alive(&self) -> bool {
        self.proc.is_alive()
    }

    fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        if !self.is_alive() {
            return Err("codex app-server is not running".into());
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, tx);
        if let Err(error) = self
            .proc
            .write_line(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
        {
            self.pending.lock().unwrap().remove(&id);
            return Err(error);
        }
        match rx.recv_timeout(REQUEST_TIMEOUT) {
            Ok(r) => r,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.pending.lock().unwrap().remove(&id);
                Err(format!(
                    "codex {method}: no reply in {}s",
                    REQUEST_TIMEOUT.as_secs()
                ))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(format!("codex {method}: server exited"))
            }
        }
    }

    fn notify(&self, method: &str) -> Result<(), String> {
        self.proc
            .write_line(&json!({ "jsonrpc": "2.0", "method": method }))
    }

    fn respond(&self, id: &Value, result: Value) {
        let _ = self
            .proc
            .write_line(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }

    fn respond_error(&self, id: &Value, code: i64, message: &str) {
        let _ = self.proc.write_line(&json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": code, "message": message },
        }));
    }

    fn initialize(&self) -> Result<(), String> {
        self.request(
            "initialize",
            json!({
                "clientInfo": { "name": "oculus", "title": "Oculus", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": { "experimentalApi": true },
            }),
        )?;
        self.notify("initialized")
    }

    pub fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        let r = self.request("model/list", json!({}))?;
        let data = r
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or("model/list: no data")?;
        let mut out = Vec::new();
        for m in data {
            if m.get("hidden").and_then(|h| h.as_bool()).unwrap_or(false) {
                continue;
            }
            let id = m
                .get("id")
                .or(m.get("model"))
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                continue;
            }
            let efforts: Vec<String> = m
                .get("supportedReasoningEfforts")
                .and_then(|e| e.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| {
                            x.get("reasoningEffort")
                                .and_then(|s| s.as_str())
                                .map(String::from)
                        })
                        .collect()
                })
                .unwrap_or_default();
            out.push(ModelInfo {
                display_name: m
                    .get("displayName")
                    .and_then(|s| s.as_str())
                    .unwrap_or(&id)
                    .to_string(),
                description: m
                    .get("description")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string(),
                reasoning_efforts: efforts,
                default_reasoning_effort: m
                    .get("defaultReasoningEffort")
                    .and_then(|s| s.as_str())
                    .map(String::from),
                is_default: m
                    .get("isDefault")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false),
                id,
            });
        }
        Ok(out)
    }

    /// Ask for the plan windows: the push only comes with a model call, so the
    /// meter could show a window that has since reset. Goes out on the
    /// account sink, like the push.
    pub fn read_rate_limits(&self) -> Result<(), String> {
        let r = self.request("account/rateLimits/read", json!({}))?;
        let windows = rate_windows(&r["rateLimits"]);
        if windows.is_empty() {
            return Ok(());
        }
        if let Some(sink) = &self.account_sink {
            sink(HarnessEvent::RateLimits { windows });
        }
        Ok(())
    }

    fn thread_params(opts: &CodexThreadOpts) -> Value {
        let mut config = json!({
            // A native question has nowhere to go and would hang the turn.
            "features.default_mode_request_user_input": false,
            // `thread/start` takes a mode, not a policy, so the writable files
            // go in as config here and as `sandboxPolicy` on `turn/start`.
            "sandbox_workspace_write.writable_roots": opts.writable_files,
            "sandbox_workspace_write.network_access": true,
        });
        if let Some(e) = &opts.reasoning_effort {
            config["model_reasoning_effort"] = json!(e);
        }
        let mut p = json!({
            "cwd": opts.cwd,
            "approvalPolicy": "never",
            "sandbox": "workspace-write",
            "config": config,
        });
        if let Some(m) = &opts.model {
            p["model"] = json!(m);
        }
        if !opts.instructions.trim().is_empty() {
            p["developerInstructions"] = json!(opts.instructions);
        }
        p
    }

    fn route(&self, thread_id: &str, sink: Sink, resumed: bool) {
        let route = Arc::new(ThreadRoute {
            sink,
            state: Mutex::new(ThreadState {
                ignore_usage_until_turn: resumed,
                ..Default::default()
            }),
        });
        self.routes
            .lock()
            .unwrap()
            .insert(thread_id.to_string(), route);
    }

    /// Start a thread; returns Codex's id for it.
    pub fn start_thread(&self, opts: &CodexThreadOpts, sink: Sink) -> Result<String, String> {
        let mut p = Self::thread_params(opts);
        p["ephemeral"] = json!(opts.ephemeral);
        let r = self.request("thread/start", p)?;
        let id = thread_id_of(&r).ok_or("thread/start: no thread id")?;
        self.route(&id, sink.clone(), false);
        sink(HarnessEvent::SessionStarted {
            provider_session_id: id.clone(),
            model: r
                .pointer("/thread/model")
                .and_then(|m| m.as_str())
                .map(String::from),
            cwd: opts.cwd.display().to_string(),
        });
        Ok(id)
    }

    pub fn resume_thread(
        &self,
        thread_id: &str,
        opts: &CodexThreadOpts,
        sink: Sink,
    ) -> Result<(), String> {
        let mut p = Self::thread_params(opts);
        p["threadId"] = json!(thread_id);
        // The turns are in our own database.
        p["excludeTurns"] = json!(true);
        let r = self.request("thread/resume", p)?;
        let id = thread_id_of(&r).unwrap_or_else(|| thread_id.to_string());
        self.route(&id, sink.clone(), true);
        sink(HarnessEvent::SessionStarted {
            provider_session_id: id,
            model: r
                .pointer("/thread/model")
                .and_then(|m| m.as_str())
                .map(String::from),
            cwd: opts.cwd.display().to_string(),
        });
        Ok(())
    }

    /// The turn's payload, including its sandbox policy.
    fn turn_params(thread_id: &str, text: &str, opts: &CodexThreadOpts) -> Value {
        let mut p = json!({
            "threadId": thread_id,
            "input": [{ "type": "text", "text": text, "text_elements": [] }],
            "approvalPolicy": "never",
            "sandboxPolicy": {
                "type": "workspaceWrite",
                "writableRoots": opts.writable_files,
                "networkAccess": true,
                "excludeTmpdirEnvVar": false,
                "excludeSlashTmp": false,
            },
        });
        if let Some(m) = &opts.model {
            p["model"] = json!(m);
        }
        p
    }

    pub fn start_turn(
        &self,
        thread_id: &str,
        text: &str,
        opts: &CodexThreadOpts,
    ) -> Result<(), String> {
        let p = Self::turn_params(thread_id, text, opts);
        let r = self.request("turn/start", p)?;
        // Taken from the response: `turn/started` can lag while the server
        // warms MCP and hooks, leaving an interrupt or exit no turn to name.
        if let Some(id) = r.pointer("/turn/id").and_then(|s| s.as_str()) {
            let route = self.routes.lock().unwrap().get(thread_id).cloned();
            if let Some(route) = route {
                route
                    .state
                    .lock()
                    .unwrap()
                    .active_turn
                    .get_or_insert(id.to_string());
                // What a later `thread/revert` names.
                (route.sink)(HarnessEvent::TurnAnchor {
                    anchor: id.to_string(),
                });
            }
        }
        Ok(())
    }

    pub fn interrupt(&self, thread_id: &str) -> Result<(), String> {
        let turn = self
            .routes
            .lock()
            .unwrap()
            .get(thread_id)
            .and_then(|r| r.state.lock().unwrap().active_turn.clone());
        let Some(turn_id) = turn else {
            return Ok(());
        };
        self.request(
            "turn/interrupt",
            json!({ "threadId": thread_id, "turnId": turn_id }),
        )?;
        Ok(())
    }

    /// Drop a turn and every later one from the server's history; the thread
    /// id stays valid. Files the agent wrote are not put back.
    pub fn revert(&self, thread_id: &str, before_turn_id: &str) -> Result<(), String> {
        self.request(
            "thread/revert",
            json!({ "threadId": thread_id, "beforeTurnId": before_turn_id }),
        )?;
        Ok(())
    }

    /// Forget a thread without touching the server's copy of it.
    pub fn detach(&self, thread_id: &str) {
        self.routes.lock().unwrap().remove(thread_id);
    }

    pub fn has_thread(&self, thread_id: &str) -> bool {
        self.routes.lock().unwrap().contains_key(thread_id)
    }

    pub fn kill(&self) {
        self.proc.kill();
    }

    // ── Inbound ──────────────────────────────────────────────────────────

    fn dispatch(&self, v: Value) {
        let has_id = v.get("id").map_or(false, |i| !i.is_null());
        let method = v.get("method").and_then(|m| m.as_str());
        match (has_id, method) {
            (true, None) => {
                let id = v.get("id").and_then(|i| i.as_i64()).unwrap_or(-1);
                if let Some(tx) = self.pending.lock().unwrap().remove(&id) {
                    let r = match v.get("error") {
                        Some(e) => Err(e
                            .get("message")
                            .and_then(|m| m.as_str())
                            .map(String::from)
                            .unwrap_or_else(|| e.to_string())),
                        None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                    };
                    let _ = tx.send(r);
                }
            }
            (true, Some(m)) => self.handle_server_request(&v["id"], m, &v["params"]),
            (false, Some(m)) => self.handle_notification(m, &v["params"]),
            (false, None) => {}
        }
    }

    /// `approvalPolicy: never` should mean no approvals arrive; one that does
    /// is refused rather than left hanging.
    fn handle_server_request(&self, id: &Value, method: &str, _params: &Value) {
        match method {
            "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
                self.respond(id, json!({ "decision": "decline" }));
            }
            "item/permissions/requestApproval" => {
                self.respond(id, json!({ "permissions": {}, "scope": "turn" }));
            }
            _ => self.respond_error(id, -32601, &format!("oculus does not handle {method}")),
        }
    }

    fn handle_notification(&self, method: &str, params: &Value) {
        // Account-scoped first: they carry no `threadId`.
        let account = translate_account(method, params);
        if !account.is_empty() {
            if let Some(sink) = &self.account_sink {
                for ev in account {
                    sink(ev);
                }
            }
            return;
        }
        let thread_id = params
            .get("threadId")
            .and_then(|t| t.as_str())
            .map(String::from)
            .or_else(|| {
                params
                    .pointer("/thread/id")
                    .and_then(|t| t.as_str())
                    .map(String::from)
            });
        let Some(thread_id) = thread_id else {
            return;
        };
        let route = self.routes.lock().unwrap().get(&thread_id).cloned();
        let Some(route) = route else {
            return;
        };
        let events = {
            let mut st = route.state.lock().unwrap();
            translate(method, params, &mut st)
        };
        for ev in events {
            (route.sink)(ev);
        }
    }
}

fn thread_id_of(r: &Value) -> Option<String> {
    r.pointer("/thread/id")
        .and_then(|s| s.as_str())
        .map(String::from)
}

// ── Translation ──────────────────────────────────────────────────────────────

/// Notifications about the account, not a thread.
fn translate_account(method: &str, p: &Value) -> Vec<HarnessEvent> {
    let mut out = Vec::new();
    if method == "account/rateLimits/updated" {
        let windows = rate_windows(&p["rateLimits"]);
        if !windows.is_empty() {
            out.push(HarnessEvent::RateLimits { windows });
        }
    }
    out
}

/// The `rateLimits` object as the meter's windows. The keys are only
/// `primary`/`secondary`, so the duration is the label and the key a fallback.
fn rate_windows(rl: &Value) -> Vec<RateWindow> {
    let mut windows = Vec::new();
    for (key, fallback) in [("primary", "5-hour"), ("secondary", "Weekly")] {
        let Some(w) = rl.get(key).filter(|w| !w.is_null()) else {
            continue;
        };
        let mins = w.get("windowDurationMins").and_then(|m| m.as_u64());
        let label = match mins {
            Some(10080) => "Weekly",
            Some(300) => "5-hour",
            Some(m) if m % 60 == 0 => return_label(format!("{}-hour", m / 60)),
            _ => fallback,
        };
        windows.push(RateWindow {
            label: label.to_string(),
            used_percent: w.get("usedPercent").and_then(|u| u.as_f64()).unwrap_or(0.0),
            resets_at: w.get("resetsAt").and_then(|r| r.as_i64()),
        });
    }
    windows
}

fn translate(method: &str, p: &Value, st: &mut ThreadState) -> Vec<HarnessEvent> {
    let mut out = Vec::new();
    match method {
        "turn/started" => {
            st.active_turn = p
                .pointer("/turn/id")
                .and_then(|s| s.as_str())
                .map(String::from);
            st.ignore_usage_until_turn = false;
            st.open_items.clear();
            st.streamed_messages.clear();
            st.partial_message.clear();
            out.push(HarnessEvent::TurnStarted);
        }
        "turn/completed" => {
            st.active_turn = None;
            let status = match p.pointer("/turn/status").and_then(|s| s.as_str()) {
                Some("failed") => "failed",
                Some("interrupted") => "interrupted",
                _ => "completed",
            };
            if let Some(msg) = p.pointer("/turn/error/message").and_then(|m| m.as_str()) {
                out.push(HarnessEvent::error_for(Provider::Codex, msg));
            }
            // Commit what a cut-short turn had said; a normal turn already
            // cleared this on `item/completed`.
            let partial = std::mem::take(&mut st.partial_message);
            if !partial.trim().is_empty() {
                out.push(HarnessEvent::AssistantMessage { text: partial });
            }
            out.push(HarnessEvent::TurnFinished {
                status: status.into(),
            });
        }
        "item/started" => {
            let item = &p["item"];
            let id = s(item, "id");
            match s(item, "type").as_str() {
                "commandExecution" => {
                    let input = json!({ "command": s(item, "command"), "cwd": s(item, "cwd") });
                    let (kind, title) = classify("commandExecution", &input);
                    st.open_items.insert(id.clone(), kind);
                    out.push(HarnessEvent::ToolStarted {
                        id,
                        kind,
                        name: "commandExecution".into(),
                        title,
                        input,
                    });
                }
                "fileChange" => {
                    let paths: Vec<String> = item
                        .get("changes")
                        .and_then(|c| c.as_array())
                        .map(|a| a.iter().map(|c| s(c, "path")).collect())
                        .unwrap_or_default();
                    let title = paths
                        .iter()
                        .map(|p| p.rsplit('/').next().unwrap_or(p).to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    st.open_items.insert(id.clone(), ToolKind::Edit);
                    out.push(HarnessEvent::ToolStarted {
                        id,
                        kind: ToolKind::Edit,
                        name: "fileChange".into(),
                        title,
                        input: json!({ "paths": paths }),
                    });
                }
                "mcpToolCall" => {
                    let title = s(item, "tool");
                    st.open_items.insert(id.clone(), ToolKind::Other);
                    out.push(HarnessEvent::ToolStarted {
                        id,
                        kind: ToolKind::Other,
                        name: format!("mcp__{}__{}", s(item, "server"), s(item, "tool")),
                        title,
                        input: item.get("arguments").cloned().unwrap_or(Value::Null),
                    });
                }
                "webSearch" => {
                    let title = s(item, "query");
                    st.open_items.insert(id.clone(), ToolKind::Web);
                    out.push(HarnessEvent::ToolStarted {
                        id,
                        kind: ToolKind::Web,
                        name: "webSearch".into(),
                        title,
                        input: json!({ "query": s(item, "query") }),
                    });
                }
                _ => {}
            }
        }
        "item/agentMessage/delta" => {
            st.streamed_messages.insert(s(p, "itemId"));
            let text = s(p, "delta");
            if !text.is_empty() {
                st.partial_message.push_str(&text);
                out.push(HarnessEvent::AssistantDelta { text });
            }
        }
        "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
            let text = s(p, "delta");
            if !text.is_empty() {
                out.push(HarnessEvent::ThinkingDelta { text });
            }
        }
        "item/commandExecution/outputDelta" => {
            let text = s(p, "delta");
            if !text.is_empty() {
                out.push(HarnessEvent::ToolOutputDelta {
                    id: s(p, "itemId"),
                    text,
                });
            }
        }
        "item/completed" => {
            let item = &p["item"];
            let id = s(item, "id");
            let status = s(item, "status");
            match s(item, "type").as_str() {
                "agentMessage" => {
                    st.partial_message.clear();
                    let text = s(item, "text");
                    if !text.trim().is_empty() {
                        out.push(HarnessEvent::AssistantMessage { text });
                    }
                }
                "reasoning" => {
                    let parts: Vec<String> = ["summary", "content"]
                        .iter()
                        .filter_map(|k| item.get(*k).and_then(|a| a.as_array()))
                        .flatten()
                        .filter_map(|x| x.as_str())
                        .filter(|t| !t.trim().is_empty())
                        .map(String::from)
                        .collect();
                    if !parts.is_empty() {
                        out.push(HarnessEvent::Thinking {
                            text: parts.join("\n\n"),
                        });
                    }
                }
                "plan" => {
                    let text = s(item, "text");
                    if !text.trim().is_empty() {
                        out.push(HarnessEvent::AssistantMessage { text });
                    }
                }
                "commandExecution" => {
                    ensure_open(
                        &mut out,
                        st,
                        &id,
                        "commandExecution",
                        json!({ "command": s(item, "command"), "cwd": s(item, "cwd") }),
                    );
                    let exit = item.get("exitCode").and_then(|c| c.as_i64());
                    let mut output = s(item, "aggregatedOutput");
                    if let Some(c) = exit.filter(|c| *c != 0) {
                        if !output.is_empty() && !output.ends_with('\n') {
                            output.push('\n');
                        }
                        output.push_str(&format!("exit code {c}"));
                    }
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        ok: status == "completed" && exit.unwrap_or(0) == 0,
                        output: cap_output(&output),
                        title: None,
                    });
                }
                "fileChange" => {
                    ensure_open(&mut out, st, &id, "fileChange", json!({}));
                    let diffs: Vec<String> = item
                        .get("changes")
                        .and_then(|c| c.as_array())
                        .map(|a| {
                            a.iter()
                                .map(|c| format!("--- {}\n{}", s(c, "path"), s(c, "diff")))
                                .collect()
                        })
                        .unwrap_or_default();
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        ok: status == "completed",
                        output: cap_output(&diffs.join("\n")),
                        title: None,
                    });
                }
                "mcpToolCall" => {
                    ensure_open(&mut out, st, &id, "mcpToolCall", json!({}));
                    let output = item
                        .pointer("/error/message")
                        .and_then(|m| m.as_str())
                        .map(String::from)
                        .or_else(|| item.get("result").map(|r| r.to_string()))
                        .unwrap_or_default();
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        ok: status == "completed",
                        output: cap_output(&output),
                        title: None,
                    });
                }
                "webSearch" => {
                    ensure_open(&mut out, st, &id, "webSearch", json!({}));
                    let (title, output) = web_search_detail(item);
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        // A finished search carries no `status` (0.153.4).
                        ok: matches!(status.as_str(), "" | "completed"),
                        output: cap_output(&output),
                        // `item/started` announces an empty `query`.
                        title: Some(title),
                    });
                }
                _ => {}
            }
        }
        "thread/tokenUsage/updated" => {
            if st.ignore_usage_until_turn {
                return out;
            }
            let u = &p["tokenUsage"];
            let n = |path: &str| u.pointer(path).and_then(|x| x.as_u64());
            out.push(HarnessEvent::Usage {
                input_tokens: n("/total/inputTokens").unwrap_or(0),
                output_tokens: n("/total/outputTokens").unwrap_or(0),
                context_tokens: n("/last/totalTokens"),
                context_window: n("/modelContextWindow"),
                cost_usd: None,
            });
        }
        "error" => {
            let msg = p
                .pointer("/error/message")
                .and_then(|m| m.as_str())
                .unwrap_or("codex error");
            let will_retry = p
                .get("willRetry")
                .and_then(|b| b.as_bool())
                .unwrap_or(false);
            if !will_retry {
                out.push(HarnessEvent::error_for(Provider::Codex, msg));
            }
        }
        _ => {}
    }
    out
}

/// Leaks a few bytes per distinct window length ever seen.
fn return_label(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// A finished web search as a row title and body. `query` is an elided
/// summary; `action.queries` is what was actually sent, so it wins.
fn web_search_detail(item: &Value) -> (String, String) {
    let queries: Vec<String> = item
        .pointer("/action/queries")
        .and_then(|q| q.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|q| q.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let title = match queries.split_first() {
        Some((first, [])) => first.clone(),
        Some((first, rest)) => format!("{first} (+{} more)", rest.len()),
        None => s(item, "query"),
    };

    let mut body = String::new();
    for q in &queries {
        body.push_str(&format!("search: {q}\n"));
    }
    let results = item.get("results").and_then(|r| r.as_array());
    for r in results.into_iter().flatten() {
        let (t, url) = (s(r, "title"), s(r, "url"));
        if t.is_empty() && url.is_empty() {
            continue;
        }
        if !body.is_empty() && !body.ends_with("\n\n") {
            body.push('\n');
        }
        body.push_str(&format!("{t}\n{url}\n"));
    }
    (title, body)
}

/// The server can complete an item it never announced; give it a row to close.
fn ensure_open(
    out: &mut Vec<HarnessEvent>,
    st: &mut ThreadState,
    id: &str,
    name: &str,
    input: Value,
) {
    if st.open_items.contains_key(id) {
        return;
    }
    let (kind, title) = classify(name, &input);
    st.open_items.insert(id.to_string(), kind);
    out.push(HarnessEvent::ToolStarted {
        id: id.to_string(),
        kind,
        name: name.into(),
        title,
        input,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_failed_request_write_does_not_leave_a_pending_reply() {
        use std::io::{BufRead, BufReader, Read};

        // The child prints one line and exits, so its stdin's read end is
        // gone and the write meets a broken pipe. `kill` and `reap` would
        // mark the server dead and take the early "not running" path, so the
        // exit is awaited through stdout's EOF instead. A child that held
        // stdin open and slept could instead inherit a pipe end another
        // thread's concurrent spawn leaked, and the write would succeed.
        let mut command = Command::new("sh");
        command.args(["-c", "printf 'ready\n'"]);
        let (proc, stdout) = ChildProc::spawn("codex", &mut command, true).unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut ready = String::new();
            let first = reader.read_line(&mut ready).map(|_| ready);
            let mut rest = Vec::new();
            let _ = reader.read_to_end(&mut rest);
            let _ = tx.send(first);
        });
        let ready = rx
            .recv_timeout(Duration::from_secs(60))
            .expect("the child neither answered nor exited within 60s")
            .unwrap();
        assert_eq!(ready.trim(), "ready");
        let server = CodexServer {
            proc,
            next_id: AtomicI64::new(1),
            pending: Mutex::new(HashMap::new()),
            routes: Mutex::new(HashMap::new()),
            account_sink: None,
        };
        let result = server.request("test", Value::Null);
        server.proc.kill();
        assert!(matches!(result, Err(error) if error.starts_with("codex stdin:")));
        assert!(server.pending.lock().unwrap().is_empty());
    }

    #[test]
    fn a_thread_may_write_the_database_and_nothing_else_outside_its_cwd() {
        let library = std::path::Path::new("/Users/x/Library/Application Support/com.tchan.oculus");
        let opts = CodexThreadOpts {
            cwd: library.join("agents"),
            writable_files: crate::paths::db_write_paths(library),
            model: Some("gpt-5.3-codex".into()),
            reasoning_effort: Some("high".into()),
            instructions: String::new(),
            ephemeral: false,
        };
        let want = serde_json::json!([
            "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db",
            "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db-wal",
            "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db-shm",
        ]);

        let thread = CodexServer::thread_params(&opts);
        assert_eq!(thread["sandbox"], "workspace-write");
        assert_eq!(
            thread["cwd"],
            library.join("agents").to_string_lossy().as_ref()
        );
        assert_eq!(
            thread["config"]["sandbox_workspace_write.writable_roots"],
            want
        );

        let turn = CodexServer::turn_params("t1", "hello", &opts);
        assert_eq!(turn["sandboxPolicy"]["writableRoots"], want);
        assert_eq!(turn["sandboxPolicy"]["type"], "workspaceWrite");
        assert_eq!(
            turn["approvalPolicy"], "never",
            "a prompt has nowhere to go"
        );
    }

    /// `oculus` reaches keyd's Unix socket from inside Codex's sandbox only
    /// while the workspace-write policy has network access (measured: the
    /// connect gets EPERM without it); Codex has no per-socket grant.
    #[test]
    fn every_thread_and_turn_keeps_the_sandbox_network_open() {
        for (model, effort, instructions, ephemeral) in [
            (Some("gpt-5.3-codex"), Some("high"), "brief", false),
            (None, None, "", true),
        ] {
            let opts = CodexThreadOpts {
                cwd: "/lib/agents".into(),
                writable_files: Vec::new(),
                model: model.map(String::from),
                reasoning_effort: effort.map(String::from),
                instructions: instructions.into(),
                ephemeral,
            };
            let thread = CodexServer::thread_params(&opts);
            assert_eq!(thread["sandbox"], "workspace-write");
            assert_eq!(
                thread["config"]["sandbox_workspace_write.network_access"],
                true
            );
            let turn = CodexServer::turn_params("t", "hi", &opts);
            assert_eq!(turn["sandboxPolicy"]["type"], "workspaceWrite");
            assert_eq!(turn["sandboxPolicy"]["networkAccess"], true);
        }
    }

    /// Recorded (0.153.4): a finished `webSearch` has no `status`, and
    /// `item/started` leaves its query empty.
    #[test]
    fn a_web_search_that_worked_is_not_drawn_as_a_failure() {
        let raw = include_str!("../../fixtures/harness/codex-websearch.ndjson");
        let mut st = ThreadState::default();
        let mut events = Vec::new();
        for line in raw.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(method) = v.get("method").and_then(|m| m.as_str()) else {
                continue;
            };
            events.extend(translate(method, &v["params"], &mut st));
        }
        assert!(matches!(
            events.first(),
            Some(HarnessEvent::ToolStarted { kind: ToolKind::Web, name, .. }) if name == "webSearch"
        ));
        let Some(HarnessEvent::ToolFinished {
            ok, output, title, ..
        }) = events.last()
        else {
            panic!("no finish: {events:?}");
        };
        assert!(*ok, "a search with results is not a failure");
        let title = title.as_deref().unwrap_or_default();
        assert!(
            title.starts_with("site:torproject.org bridges obfs4"),
            "{title}"
        );
        assert!(title.ends_with("(+3 more)"), "{title}");
        assert_eq!(output.matches("search: ").count(), 4);
        assert!(output
            .contains("https://support.torproject.org/little-t-tor/circumvention/using-bridges/"));
    }

    /// Recorded from `codex app-server` 0.153.4 asked to `ls` the library.
    #[test]
    fn folds_a_recorded_thread() {
        let raw = include_str!("../../fixtures/harness/codex-ls.ndjson");
        let mut st = ThreadState::default();
        let mut events = Vec::new();
        for line in raw.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(method) = v.get("method").and_then(|m| m.as_str()) else {
                continue;
            };
            // As `handle_notification` routes them.
            let account = translate_account(method, &v["params"]);
            if !account.is_empty() {
                events.extend(account);
                continue;
            }
            events.extend(translate(method, &v["params"], &mut st));
        }
        assert!(matches!(events.first(), Some(HarnessEvent::TurnStarted)));
        let tools: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::ToolStarted { kind, title, .. } => Some((*kind, title.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(tools, vec![(ToolKind::Bash, "/bin/zsh -lc ls".to_string())]);
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::ToolFinished { ok: true, output, .. } if output.contains("oculus.db"))));
        let messages = events
            .iter()
            .filter(|e| matches!(e, HarnessEvent::AssistantMessage { .. }))
            .count();
        assert_eq!(messages, 2);
        assert!(events.iter().any(|e| matches!(
            e,
            HarnessEvent::Usage {
                context_window: Some(_),
                ..
            }
        )));
        assert!(events
            .iter()
            .any(|e| matches!(e, HarnessEvent::RateLimits { windows } if windows.len() == 2)));
        assert!(
            matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "completed")
        );
    }

    /// Recorded: an interrupted turn never sends the `item/completed`.
    #[test]
    fn a_stopped_turn_keeps_what_was_said() {
        let raw = include_str!("../../fixtures/harness/codex-interrupt.ndjson");
        let mut st = ThreadState::default();
        let mut events = Vec::new();
        for line in raw.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(method) = v.get("method").and_then(|m| m.as_str()) else {
                continue;
            };
            events.extend(translate(method, &v["params"], &mut st));
        }
        let deltas: String = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::AssistantDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(!deltas.is_empty(), "the fixture streams an answer");
        let message = events.iter().find_map(|e| match e {
            HarnessEvent::AssistantMessage { text } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(
            message.as_deref(),
            Some(deltas.as_str()),
            "the partial is committed as a row"
        );
        assert!(
            matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "interrupted")
        );
    }
}
