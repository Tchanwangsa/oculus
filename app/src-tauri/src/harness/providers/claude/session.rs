//! One `claude -p` process per thread: spawn, send, interrupt, rewind.

use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::harness::child::{self, ChildProc};
use crate::harness::event::Provider;
use crate::harness::Sink;

use super::settings::settings_json;
use super::translate::Translator;
use super::ClaudeSpawn;

pub struct ClaudeSession {
    pub(super) proc: ChildProc,
    pub(super) request_ids: AtomicU64,
    /// Set between asking the CLI to stop and the `result` that answers; the
    /// `result` line does not say plainly that it was interrupted.
    pub(super) interrupting: Arc<AtomicBool>,
    /// Control requests (only `rewind`) waiting on their `control_response`.
    pub(super) pending: Mutex<HashMap<String, mpsc::Sender<Value>>>,
    /// Set between a message going in and the `result` that closes its turn,
    /// so a process that dies before its first stream event (a rejected model
    /// name) still emits the `TurnFinished` the manager's queue waits on.
    pub(super) expecting: Arc<AtomicBool>,
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
    /// transcript ([`super::transcript::anchor_for`]). `last_seen` is the newest question's: with
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
