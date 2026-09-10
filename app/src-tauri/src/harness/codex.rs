//! The Codex bridge: JSON-RPC over stdio to `codex app-server`.
//!
//! One app-server process serves every Codex thread in the app. The
//! protocol is built for that — every notification carries a `threadId` —
//! and it keeps the process count at one rather than one per thread. bb
//! chose a process per thread for isolation; this app is a single user with
//! a handful of threads, and a shared server is what `codex` itself does for
//! its own desktop client.
//!
//! Framing is one JSON object per line in both directions. Requests carry a
//! numeric `id`; responses echo it and have no `method`; notifications have
//! a `method` and no `id`; a line with both is the server asking *us*
//! something (an approval), which must be answered or the turn hangs.
//!
//! The shapes here were taken from `codex app-server` 0.153 and cross-read
//! against bb's `provider-codex` plugin. Two of them are easy to get wrong:
//! `thread/start` takes `sandbox` (a mode string) while `turn/start` takes
//! `sandboxPolicy` (an object), and a resumed thread replays its last turn's
//! token usage before doing anything new, which must not be shown as this
//! turn's usage.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

use super::event::{cap_output, classify, HarnessEvent, RateWindow, ToolKind};
use super::{RawLog, Sink};

/// A request that gets no answer in this long is a hung server, not a slow
/// one — `model/list` and `thread/start` are sub-second.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub struct CodexSpawn {
    pub bin: PathBuf,
    pub env: Vec<(String, String)>,
    pub raw_log: Option<RawLog>,
}

/// How to open a thread. `cwd` is the sandbox's writable root as well as
/// the working directory — under `workspace-write`, that is the whole
/// containment story.
pub struct CodexThreadOpts {
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    /// Appended to Codex's own instructions (`developerInstructions`).
    pub instructions: String,
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
    /// Items that were announced with `item/started`, so a delta arriving
    /// first (which the server does) can synthesise the open.
    open_items: HashMap<String, ToolKind>,
    /// Agent messages whose deltas have been streamed; `item/completed`
    /// carries the whole text again.
    streamed_messages: std::collections::HashSet<String>,
}

pub struct CodexServer {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    next_id: AtomicI64,
    pending: Mutex<HashMap<i64, mpsc::Sender<Result<Value, String>>>>,
    routes: Mutex<HashMap<String, Arc<ThreadRoute>>>,
    alive: Arc<AtomicBool>,
}

impl CodexServer {
    pub fn spawn(cfg: CodexSpawn) -> Result<Arc<Self>, String> {
        let mut child = Command::new(&cfg.bin)
            .arg("app-server")
            .env_clear()
            .envs(cfg.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", cfg.bin.display()))?;
        let stdin = child.stdin.take().ok_or("no stdin on codex child")?;
        let stdout = child.stdout.take().ok_or("no stdout on codex child")?;
        let stderr = child.stderr.take().ok_or("no stderr on codex child")?;

        let alive = Arc::new(AtomicBool::new(true));
        let server = Arc::new(CodexServer {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            next_id: AtomicI64::new(1),
            pending: Mutex::new(HashMap::new()),
            routes: Mutex::new(HashMap::new()),
            alive: alive.clone(),
        });

        // stderr is tracing plus every MCP server in the user's own codex
        // config failing to start; none of it is ours. Keep a tail for the
        // exit message and drop the rest.
        let stderr_tail: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        {
            let tail = stderr_tail.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let mut t = tail.lock().unwrap();
                    if t.len() >= 20 {
                        t.remove(0);
                    }
                    t.push(line);
                }
            });
        }

        let reader = server.clone();
        let raw_log = cfg.raw_log;
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(log) = &raw_log {
                    log.write(&line);
                }
                let Ok(v) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                reader.dispatch(v);
            }
            alive.store(false, Ordering::SeqCst);
            let code = reader.child.lock().unwrap().wait().ok().and_then(|s| s.code());
            // Every waiting request fails, every routed thread hears it.
            reader.pending.lock().unwrap().clear();
            let routes: Vec<Arc<ThreadRoute>> = reader.routes.lock().unwrap().drain().map(|(_, r)| r).collect();
            for r in routes {
                let mid_turn = r.state.lock().unwrap().active_turn.take().is_some();
                if mid_turn {
                    let tail = stderr_tail.lock().unwrap().join("\n");
                    (r.sink)(HarnessEvent::error(format!("codex app-server exited (code {code:?})\n{tail}")));
                    (r.sink)(HarnessEvent::TurnFinished {
                        status: "failed".into(),
                    });
                }
                (r.sink)(HarnessEvent::Exited { code });
            }
        });

        server.initialize()?;
        Ok(server)
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    fn write_line(&self, v: &Value) -> Result<(), String> {
        let mut stdin = self.stdin.lock().unwrap();
        let line = serde_json::to_string(v).map_err(|e| e.to_string())?;
        stdin
            .write_all(line.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("codex stdin: {e}"))
    }

    fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        if !self.is_alive() {
            return Err("codex app-server is not running".into());
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, tx);
        self.write_line(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        match rx.recv_timeout(REQUEST_TIMEOUT) {
            Ok(r) => r,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.pending.lock().unwrap().remove(&id);
                Err(format!("codex {method}: no reply in {}s", REQUEST_TIMEOUT.as_secs()))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!("codex {method}: server exited")),
        }
    }

    fn notify(&self, method: &str) -> Result<(), String> {
        self.write_line(&json!({ "jsonrpc": "2.0", "method": method }))
    }

    fn respond(&self, id: &Value, result: Value) {
        let _ = self.write_line(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }

    fn respond_error(&self, id: &Value, code: i64, message: &str) {
        let _ = self.write_line(&json!({
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
        let data = r.get("data").and_then(|d| d.as_array()).ok_or("model/list: no data")?;
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
                        .filter_map(|x| x.get("reasoningEffort").and_then(|s| s.as_str()).map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            out.push(ModelInfo {
                display_name: m.get("displayName").and_then(|s| s.as_str()).unwrap_or(&id).to_string(),
                description: m.get("description").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                reasoning_efforts: efforts,
                default_reasoning_effort: m
                    .get("defaultReasoningEffort")
                    .and_then(|s| s.as_str())
                    .map(String::from),
                is_default: m.get("isDefault").and_then(|b| b.as_bool()).unwrap_or(false),
                id,
            });
        }
        Ok(out)
    }

    fn thread_params(opts: &CodexThreadOpts) -> Value {
        let mut config = json!({
            // Native questions have no UI yet; a request for one would sit
            // unanswered and hang the turn.
            "features.default_mode_request_user_input": false,
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
        self.routes.lock().unwrap().insert(thread_id.to_string(), route);
    }

    /// Start a thread; returns Codex's id for it.
    pub fn start_thread(&self, opts: &CodexThreadOpts, sink: Sink) -> Result<String, String> {
        let mut p = Self::thread_params(opts);
        p["ephemeral"] = json!(false);
        let r = self.request("thread/start", p)?;
        let id = thread_id_of(&r).ok_or("thread/start: no thread id")?;
        self.route(&id, sink.clone(), false);
        sink(HarnessEvent::SessionStarted {
            provider_session_id: id.clone(),
            model: r.pointer("/thread/model").and_then(|m| m.as_str()).map(String::from),
            cwd: opts.cwd.display().to_string(),
        });
        Ok(id)
    }

    pub fn resume_thread(&self, thread_id: &str, opts: &CodexThreadOpts, sink: Sink) -> Result<(), String> {
        let mut p = Self::thread_params(opts);
        p["threadId"] = json!(thread_id);
        // The turns are in our own database; asking for them back would be
        // a full transcript on every resume.
        p["excludeTurns"] = json!(true);
        let r = self.request("thread/resume", p)?;
        let id = thread_id_of(&r).unwrap_or_else(|| thread_id.to_string());
        self.route(&id, sink.clone(), true);
        sink(HarnessEvent::SessionStarted {
            provider_session_id: id,
            model: r.pointer("/thread/model").and_then(|m| m.as_str()).map(String::from),
            cwd: opts.cwd.display().to_string(),
        });
        Ok(())
    }

    pub fn start_turn(&self, thread_id: &str, text: &str, opts: &CodexThreadOpts) -> Result<(), String> {
        let mut p = json!({
            "threadId": thread_id,
            "input": [{ "type": "text", "text": text, "text_elements": [] }],
            "approvalPolicy": "never",
            "sandboxPolicy": {
                "type": "workspaceWrite",
                "writableRoots": [],
                "networkAccess": true,
                "excludeTmpdirEnvVar": false,
                "excludeSlashTmp": false,
            },
        });
        if let Some(m) = &opts.model {
            p["model"] = json!(m);
        }
        self.request("turn/start", p)?;
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
        self.request("turn/interrupt", json!({ "threadId": thread_id, "turnId": turn_id }))?;
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
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
        self.alive.store(false, Ordering::SeqCst);
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

    /// Approvals cannot reach the user yet, so they are refused rather than
    /// left hanging. `approvalPolicy: never` should mean none arrive; this is
    /// the guard for the one that does.
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
        let thread_id = params
            .get("threadId")
            .and_then(|t| t.as_str())
            .map(String::from)
            .or_else(|| params.pointer("/thread/id").and_then(|t| t.as_str()).map(String::from));
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

impl Drop for CodexServer {
    fn drop(&mut self) {
        if let Ok(mut c) = self.child.lock() {
            let _ = c.kill();
        }
    }
}

fn thread_id_of(r: &Value) -> Option<String> {
    r.pointer("/thread/id").and_then(|s| s.as_str()).map(String::from)
}

fn s(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

// ── Translation ──────────────────────────────────────────────────────────────

fn translate(method: &str, p: &Value, st: &mut ThreadState) -> Vec<HarnessEvent> {
    let mut out = Vec::new();
    match method {
        "turn/started" => {
            st.active_turn = p.pointer("/turn/id").and_then(|s| s.as_str()).map(String::from);
            st.ignore_usage_until_turn = false;
            st.open_items.clear();
            st.streamed_messages.clear();
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
                out.push(HarnessEvent::error(msg));
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
                // agentMessage / reasoning: content arrives as deltas and
                // again on completion. userMessage is our own text.
                _ => {}
            }
        }
        "item/agentMessage/delta" => {
            st.streamed_messages.insert(s(p, "itemId"));
            let text = s(p, "delta");
            if !text.is_empty() {
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
                    ensure_open(&mut out, st, &id, "commandExecution", json!({ "command": s(item, "command"), "cwd": s(item, "cwd") }));
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
                    });
                }
                "webSearch" => {
                    ensure_open(&mut out, st, &id, "webSearch", json!({}));
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        ok: status == "completed",
                        output: String::new(),
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
        "account/rateLimits/updated" => {
            let rl = &p["rateLimits"];
            let mut windows = Vec::new();
            for (key, fallback) in [("primary", "5-hour"), ("secondary", "Weekly")] {
                let Some(w) = rl.get(key).filter(|w| !w.is_null()) else { continue };
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
            if !windows.is_empty() {
                out.push(HarnessEvent::RateLimits { windows });
            }
        }
        "error" => {
            let msg = p.pointer("/error/message").and_then(|m| m.as_str()).unwrap_or("codex error");
            let will_retry = p.get("willRetry").and_then(|b| b.as_bool()).unwrap_or(false);
            if !will_retry {
                out.push(HarnessEvent::error(msg));
            }
        }
        // thread/status/changed, mcpServer/*, hook/*, warning, deprecationNotice,
        // turn/plan/updated, turn/diff/updated, rawResponseItem/*: not
        // surfaced yet.
        _ => {}
    }
    out
}

/// `String` → `&'static str` for the one label built at runtime. Leaks a
/// few bytes per distinct window length ever seen, which is a handful.
fn return_label(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// The server can complete an item it never announced (or announce it after
/// its first delta). Give the timeline an open row to close.
fn ensure_open(out: &mut Vec<HarnessEvent>, st: &mut ThreadState, id: &str, name: &str, input: Value) {
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

    /// Replays a recorded `codex app-server` thread (0.153.4) asked to `ls`
    /// the library and describe it.
    #[test]
    fn folds_a_recorded_thread() {
        let raw = include_str!("../../fixtures/harness/codex-ls.ndjson");
        let mut st = ThreadState::default();
        let mut events = Vec::new();
        for line in raw.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let Some(method) = v.get("method").and_then(|m| m.as_str()) else { continue };
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
        // Two agent messages: commentary, then the final answer.
        let messages = events.iter().filter(|e| matches!(e, HarnessEvent::AssistantMessage { .. })).count();
        assert_eq!(messages, 2);
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::Usage { context_window: Some(_), .. })));
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::RateLimits { windows } if windows.len() == 2)));
        assert!(matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "completed"));
    }
}
