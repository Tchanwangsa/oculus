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

use super::event::{cap_output, classify, HarnessEvent, Provider, RateWindow, ToolKind};
use super::{RawLog, Sink};

/// A request that gets no answer in this long is a hung server, not a slow
/// one — `model/list` and `thread/start` are sub-second.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub struct CodexSpawn {
    pub bin: PathBuf,
    pub env: Vec<(String, String)>,
    pub raw_log: Option<RawLog>,
    /// Takes the notifications that are about the account rather than a
    /// thread; see `translate_account`.
    pub account_sink: Option<Sink>,
}

/// How to open a thread. `cwd` is the sandbox's writable root as well as
/// the working directory — under `workspace-write`, that plus `writable_files`
/// is the whole containment story.
pub struct CodexThreadOpts {
    pub cwd: PathBuf,
    /// Single files that are writable besides everything under `cwd`: the
    /// database and its WAL sidecars, so `oculus project` / `oculus task`
    /// can write the student's board. Without them SQLite fails with
    /// "attempt to write a readonly database" — see
    /// `paths::db_write_paths`. Codex's sandbox is the only containment it
    /// has (there are no per-path tool rules to pair with it, the way
    /// Claude's `Edit` denies pair with its seatbelt), so this list is
    /// deliberately three files and not the folder they live in.
    pub writable_files: Vec<PathBuf>,
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
    /// The answer as it streams, until the `item/completed` that commits it.
    /// An interrupted turn never sends that completion — measured, and the
    /// recording is `fixtures/harness/codex-interrupt.ndjson` — so without
    /// this the half-written answer is only ever live text, and stopping a
    /// turn wiped what the agent had already said off the screen.
    partial_message: String,
}

pub struct CodexServer {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    next_id: AtomicI64,
    pending: Mutex<HashMap<i64, mpsc::Sender<Result<Value, String>>>>,
    routes: Mutex<HashMap<String, Arc<ThreadRoute>>>,
    /// Where account-scoped events go. Set for the app, absent for headless
    /// runs, which have no place to put them.
    account_sink: Option<Sink>,
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
            account_sink: cfg.account_sink,
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
                    (r.sink)(HarnessEvent::error_for(
                        Provider::Codex,
                        format!("codex app-server exited (code {code:?})\n{tail}"),
                    ));
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

    /// Ask for the plan windows instead of waiting for a turn to push them.
    /// The push (`account/rateLimits/updated`) only comes with a model call,
    /// so without this the meter shows the last turn's numbers — which after
    /// a night's gap can be a window that has since reset. The answer goes
    /// out on the account sink, the same path the push takes, so it is stored
    /// and drawn identically.
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
            // Native questions have no UI yet; a request for one would sit
            // unanswered and hang the turn.
            "features.default_mode_request_user_input": false,
            // `thread/start` takes a sandbox *mode* and no policy, so the
            // extra writable files have to arrive as config overrides here;
            // `turn/start` below sends the same set as a policy object. Both,
            // because which one governs a given call is the server's business
            // and a thread that could not write the database would fail the
            // student's first breakdown.
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

    /// The turn's own payload, including the sandbox policy that actually
    /// governs it. A free function so a test can read the containment without
    /// a server to send it to.
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

    pub fn start_turn(&self, thread_id: &str, text: &str, opts: &CodexThreadOpts) -> Result<(), String> {
        let p = Self::turn_params(thread_id, text, opts);
        let r = self.request("turn/start", p)?;
        // The turn id also arrives on the `turn/started` notification, but
        // not until the server has warmed its MCP servers and hooks — a
        // second or more. In that window an interrupt had no turn to name
        // and a server that died had no turn to fail, so a thread waiting on
        // its `TurnFinished` (`Queue` in the manager) would have waited for
        // ever. The response says it straight away, so it is taken from here.
        if let Some(id) = r.pointer("/turn/id").and_then(|s| s.as_str()) {
            let route = self.routes.lock().unwrap().get(thread_id).cloned();
            if let Some(route) = route {
                route.state.lock().unwrap().active_turn.get_or_insert(id.to_string());
                // The same id is what a later `thread/revert` names, so it is
                // kept on the question's row. It is announced here rather
                // than from the `turn/started` notification for the same
                // reason the line above reads it here: that notification can
                // be a second or more behind.
                (route.sink)(HarnessEvent::TurnAnchor { anchor: id.to_string() });
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
        self.request("turn/interrupt", json!({ "threadId": thread_id, "turnId": turn_id }))?;
        Ok(())
    }

    /// Drop a turn and every later one from the server's own history of this
    /// thread, so the agent's context matches the thread being read.
    ///
    /// `before_turn_id` is the turn the question started, kept on its row
    /// when the turn went out. The server keeps the thread — only the history
    /// is replaced — so the session id on our row stays good.
    ///
    /// Files the agent wrote are not put back: the schema says outright that
    /// reverting those is the client's job, and this bridge does not try.
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
        // Account-scoped first: rate limits belong to the subscription, not to
        // a thread, so they arrive without a `threadId` and would otherwise be
        // dropped by the route lookup below.
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

/// Notifications about the *account*, not a thread. They carry no `threadId`
/// — one server serves every thread and the limits are the same for all of
/// them — so `handle_notification` takes them before it looks a route up, and
/// they go to the harness's account sink instead of a thread's.
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

/// The `rateLimits` object, pushed after a turn and returned by
/// `account/rateLimits/read`, as the two windows the meter draws. Codex names
/// them by length rather than by role, so the duration is the label and the
/// key is only the fallback.
fn rate_windows(rl: &Value) -> Vec<RateWindow> {
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
    windows
}

fn translate(method: &str, p: &Value, st: &mut ThreadState) -> Vec<HarnessEvent> {
    let mut out = Vec::new();
    match method {
        "turn/started" => {
            st.active_turn = p.pointer("/turn/id").and_then(|s| s.as_str()).map(String::from);
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
            // Whatever the agent had said when the turn was cut short. It is
            // committed here rather than left as live text, so the row
            // survives the turn ending and a reload after it. A turn that
            // ended on its own has already cleared this on `item/completed`,
            // so nothing is written twice.
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
                // agentMessage / reasoning: content arrives as deltas and
                // again on completion. userMessage is our own text.
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
                        // **A finished search carries no `status` at all** —
                        // measured against 0.153.4, whose completed item is
                        // the query, the action and the results and nothing
                        // else. Comparing it to `"completed"` the way the
                        // arms above do is why every web search the student
                        // ever ran was drawn `failed` in red while the model
                        // was quietly answering out of the results it got.
                        // So: no status is a search that ran, and only a
                        // status that says otherwise is a failure.
                        ok: matches!(status.as_str(), "" | "completed"),
                        output: cap_output(&output),
                        // The queries are only known now: `item/started`
                        // announces the search with an empty `query`.
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
            let msg = p.pointer("/error/message").and_then(|m| m.as_str()).unwrap_or("codex error");
            let will_retry = p.get("willRetry").and_then(|b| b.as_bool()).unwrap_or(false);
            if !will_retry {
                out.push(HarnessEvent::error_for(Provider::Codex, msg));
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
/// A finished web search, as a row title and an expandable body.
///
/// The item says what was searched for twice: `query` is the model's own
/// summary of it, elided with an ellipsis, and `action.queries` is the list it
/// actually sent. The list is the truthful one, so the title is its first
/// entry (with a count of the rest, since one row cannot hold four searches)
/// and the body is every query followed by every result — titles and URLs,
/// which are what a student reads a search row to get at.
fn web_search_detail(item: &Value) -> (String, String) {
    let queries: Vec<String> = item
        .pointer("/action/queries")
        .and_then(|q| q.as_array())
        .map(|a| a.iter().filter_map(|q| q.as_str()).map(String::from).collect())
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

    /// What the sandbox is, in the two places Codex will read it.
    ///
    /// The database's three files are the only writable thing outside the
    /// thread's cwd, and they are there because `oculus task add` is how a
    /// plan reaches the board — see `paths::db_write_paths`. `thread/start`
    /// takes a sandbox *mode* and no policy, so the same set has to go in as
    /// config overrides as well as on the turn.
    #[test]
    fn a_thread_may_write_the_database_and_nothing_else_outside_its_cwd() {
        let library = std::path::Path::new("/Users/x/Library/Application Support/com.tchan.oculus");
        let opts = CodexThreadOpts {
            cwd: library.join("agents"),
            writable_files: crate::paths::db_write_paths(library),
            model: Some("gpt-5.3-codex".into()),
            reasoning_effort: Some("high".into()),
            instructions: String::new(),
        };
        let want = serde_json::json!([
            "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db",
            "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db-wal",
            "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db-shm",
        ]);

        let thread = CodexServer::thread_params(&opts);
        assert_eq!(thread["sandbox"], "workspace-write");
        assert_eq!(thread["cwd"], library.join("agents").to_string_lossy().as_ref());
        assert_eq!(thread["config"]["sandbox_workspace_write.writable_roots"], want);

        let turn = CodexServer::turn_params("t1", "hello", &opts);
        assert_eq!(turn["sandboxPolicy"]["writableRoots"], want);
        assert_eq!(turn["sandboxPolicy"]["type"], "workspaceWrite");
        assert_eq!(turn["approvalPolicy"], "never", "a prompt has nowhere to go");
    }

    /// A real web search, recorded from a thread that ran two of them
    /// (0.153.4).
    ///
    /// The regression it pins: a finished `webSearch` item carries no
    /// `status`, so comparing it to `"completed"` marked every successful
    /// search `failed` in the timeline — and the query, which `item/started`
    /// leaves empty, never reached the row at all.
    #[test]
    fn a_web_search_that_worked_is_not_drawn_as_a_failure() {
        let raw = include_str!("../../fixtures/harness/codex-websearch.ndjson");
        let mut st = ThreadState::default();
        let mut events = Vec::new();
        for line in raw.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let Some(method) = v.get("method").and_then(|m| m.as_str()) else { continue };
            events.extend(translate(method, &v["params"], &mut st));
        }
        assert!(matches!(
            events.first(),
            Some(HarnessEvent::ToolStarted { kind: ToolKind::Web, name, .. }) if name == "webSearch"
        ));
        let Some(HarnessEvent::ToolFinished { ok, output, title, .. }) = events.last() else {
            panic!("no finish: {events:?}");
        };
        assert!(*ok, "a search with results is not a failure");
        // Titled with the first real query, and told how many others rode
        // with it — the row is one line and this search sent four.
        let title = title.as_deref().unwrap_or_default();
        assert!(title.starts_with("site:torproject.org bridges obfs4"), "{title}");
        assert!(title.ends_with("(+3 more)"), "{title}");
        // Every query, then the results, for the expanded card.
        assert_eq!(output.matches("search: ").count(), 4);
        assert!(output.contains("https://support.torproject.org/little-t-tor/circumvention/using-bridges/"));
    }

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
            // The same order `handle_notification` uses: account-scoped
            // notifications never reach the thread translator, so a test that
            // only called `translate` would pass on a stream the app drops.
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
        // Two agent messages: commentary, then the final answer.
        let messages = events.iter().filter(|e| matches!(e, HarnessEvent::AssistantMessage { .. })).count();
        assert_eq!(messages, 2);
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::Usage { context_window: Some(_), .. })));
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::RateLimits { windows } if windows.len() == 2)));
        assert!(matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "completed"));
    }

    /// A turn stopped mid-answer. Recorded: the deltas simply stop, the
    /// `item/completed` that would carry the whole text never comes, and
    /// `turn/completed` says `interrupted`. Without the partial being
    /// committed here, everything the agent had already said was live text
    /// with no row behind it — so stopping a turn wiped the answer off the
    /// screen, which is exactly what it looked like from the outside.
    #[test]
    fn a_stopped_turn_keeps_what_was_said() {
        let raw = include_str!("../../fixtures/harness/codex-interrupt.ndjson");
        let mut st = ThreadState::default();
        let mut events = Vec::new();
        for line in raw.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let Some(method) = v.get("method").and_then(|m| m.as_str()) else { continue };
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
        assert_eq!(message.as_deref(), Some(deltas.as_str()), "the partial is committed as a row");
        assert!(matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "interrupted"));
    }
}
