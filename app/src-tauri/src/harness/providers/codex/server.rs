//! The `codex app-server` child: spawn, requests and the thread calls.

use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::harness::child::{self, ChildProc};
use crate::harness::event::{HarnessEvent, Provider};
use crate::harness::Sink;

use super::translate::rate_windows;
use super::{CodexSpawn, CodexThreadOpts, ModelInfo, ThreadRoute, ThreadState};

/// No answer in this long is a hung server, not a slow one.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub struct CodexServer {
    pub(super) proc: ChildProc,
    pub(super) next_id: AtomicI64,
    pub(super) pending: Mutex<HashMap<i64, mpsc::Sender<Result<Value, String>>>>,
    pub(super) routes: Mutex<HashMap<String, Arc<ThreadRoute>>>,
    /// Where account-scoped events go; absent for headless runs.
    pub(super) account_sink: Option<Sink>,
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

    pub(super) fn request(&self, method: &str, params: Value) -> Result<Value, String> {
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

    pub(super) fn respond(&self, id: &Value, result: Value) {
        let _ = self
            .proc
            .write_line(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }

    pub(super) fn respond_error(&self, id: &Value, code: i64, message: &str) {
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

    pub(super) fn thread_params(opts: &CodexThreadOpts) -> Value {
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

    pub(super) fn route(&self, thread_id: &str, sink: Sink, resumed: bool) {
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
    pub(super) fn turn_params(thread_id: &str, text: &str, opts: &CodexThreadOpts) -> Value {
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
}

fn thread_id_of(r: &Value) -> Option<String> {
    r.pointer("/thread/id")
        .and_then(|s| s.as_str())
        .map(String::from)
}
