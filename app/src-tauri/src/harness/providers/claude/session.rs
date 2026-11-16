//! One `claude -p` process per thread: spawn, send, interrupt, rewind.

use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::harness::child::{self, ChildProc};
use crate::harness::event::{Bound, HarnessEvent, Provider, Turns};
use crate::harness::requests::PermissionMode;
use crate::harness::Sink;

use super::settings::settings_json;
use super::translate::{Steering, Translator};
use super::ClaudeSpawn;

fn configure_thinking(cmd: &mut Command, one_off: bool) {
    if one_off {
        // Tool-less jobs consume final output and keep their existing display policy.
        cmd.env("CLAUDE_CODE_THINKING_DISPLAY_UPDATES", "0");
    } else {
        cmd.args(["--thinking", "adaptive", "--thinking-display", "summarized"]);
    }
}

/// How a session is started: its CLI permission mode, who answers its
/// prompts, and the tools it may never call. A chat's prompts come to the
/// host (`approvals.rs`); Auto hands them to the CLI's classifier first. A
/// one-off turn has nobody to ask, so its prompts are denied by the CLI.
/// A chat may ask questions; one-off turns have no tools.
pub(super) fn permission_args(one_off: bool, mode: PermissionMode) -> Vec<&'static str> {
    if one_off {
        // `default` auto-allows no tool, and the turn has none anyway.
        return vec![
            "--permission-mode",
            "default",
            "--permission-prompts",
            "none",
        ];
    }
    let cli_mode = match mode {
        PermissionMode::Auto => "auto",
        PermissionMode::Ask | PermissionMode::Quiet => "acceptEdits",
    };
    vec![
        "--permission-mode",
        cli_mode,
        "--permission-prompts",
        "host",
        "--permission-prompt-tool",
        "stdio",
    ]
}

/// Whether a shell command may ask to leave the sandbox
/// (`dangerouslyDisableSandbox`). Only in Ask, where the student answers:
/// in Auto the classifier would be all that stands between a command and a
/// protected folder, and in Don't ask nobody would approve it.
pub(super) fn unsandboxed_commands(one_off: bool, mode: PermissionMode) -> bool {
    !one_off && mode == PermissionMode::Ask
}

pub struct ClaudeSession {
    pub(super) proc: ChildProc,
    pub(super) request_ids: AtomicU64,
    /// Set between asking the CLI to stop and the `result` that answers; the
    /// `result` line does not say plainly that it was interrupted.
    pub(super) interrupting: Arc<AtomicBool>,
    /// Control requests (only `rewind`) waiting on their `control_response`.
    pub(super) pending: Mutex<HashMap<String, mpsc::Sender<Value>>>,
    /// Set between a message going in and its turn opening (or, for a turn
    /// that never opens, its `result`), so a process that dies before its
    /// first stream event (a rejected model name) still emits the
    /// `TurnFinished` the manager's queue waits on. A turn that opens without
    /// it is the CLI's own ([`crate::harness::HarnessEvent::UnpromptedTurn`]).
    pub(super) expecting: Arc<AtomicBool>,
    /// The process's turn ordinals, shared with its reader: the stream names
    /// no turn.
    pub(super) turns: Arc<Turns>,
    /// How many background tasks its reader counts as running.
    pub(super) background: Arc<AtomicUsize>,
    /// A one-off turn: any permission prompt is declined, unseen.
    pub(super) one_off: bool,
    /// Permission prompts raised and not yet answered or withdrawn, by
    /// their wire id (`approvals.rs`).
    pub(super) callbacks: Mutex<HashSet<String>>,
    /// Whether a turn of ours is open, and the steers written into it,
    /// shared with the reader ([`Steering`]).
    pub(super) steering: Arc<Mutex<Steering>>,
}

impl ClaudeSession {
    /// A background `Agent` or Bash is still running in this process, and
    /// would die with it.
    pub fn has_background_tasks(&self) -> bool {
        self.background.load(Ordering::SeqCst) > 0
    }

    pub fn spawn(cfg: ClaudeSpawn, sink: Sink) -> Result<Arc<Self>, String> {
        if cfg!(windows) {
            return Err(
                "Claude Code is unavailable on Windows: its native sandbox is not supported".into(),
            );
        }
        let base = cfg.base;
        let mut cmd = crate::runtime::platform::cli_command(&base.bin)?;
        cmd.arg("-p")
            .args(["--input-format", "stream-json"])
            .args(["--output-format", "stream-json"])
            .arg("--verbose")
            .arg("--include-partial-messages")
            .args(permission_args(cfg.one_off, cfg.mode));
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
        // The TTS server, so `oculus studio narrate` can reach it; a one-off
        // turn has no tools to use it with.
        let hosts: Vec<String> = if cfg.one_off {
            Vec::new()
        } else {
            crate::studio::voice::server_host(&crate::studio::voice::settings())
                .map(|(host, port)| format!("{host}:{port}"))
                .into_iter()
                .collect()
        };
        cmd.args([
            "--settings",
            &settings_json(
                &base.library,
                &base.cwd,
                cfg.oculus.as_deref(),
                &hosts,
                unsandboxed_commands(cfg.one_off, cfg.mode),
                !cfg.one_off,
            ),
        ]);
        // The Studio subagents, on the models Settings → Jobs gives Claude.
        if !cfg.one_off {
            let roles = crate::harness::studio_agents::assigned_now(Provider::Claude);
            if let Some(agents) = crate::harness::studio_agents::claude_agents_json(&roles) {
                cmd.args(["--agents", &agents]);
            }
        }
        cmd.current_dir(&base.cwd)
            .env_clear()
            .envs(base.env.iter().map(|(k, v)| (k, v)))
            // The CLI gates some behaviour on its entrypoint.
            .env("CLAUDE_CODE_ENTRYPOINT", "cli");
        configure_thinking(&mut cmd, cfg.one_off);
        let (proc, stdout) = ChildProc::spawn("claude", &mut cmd, true)?;

        let interrupting = Arc::new(AtomicBool::new(false));
        let expecting = Arc::new(AtomicBool::new(false));
        let turns = Arc::new(Turns::default());
        let background = Arc::new(AtomicUsize::new(0));
        let steering = Arc::new(Mutex::new(Steering::default()));
        let session = Arc::new(ClaudeSession {
            proc,
            request_ids: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            interrupting: interrupting.clone(),
            expecting: expecting.clone(),
            turns: turns.clone(),
            background: background.clone(),
            one_off: cfg.one_off,
            callbacks: Mutex::new(HashSet::new()),
            steering: steering.clone(),
        });

        let reader_session = session.clone();
        let raw_log = base.raw_log;
        std::thread::spawn(move || {
            let mut state = Translator {
                interrupting,
                expecting: expecting.clone(),
                turns,
                background_count: background,
                steering,
                ..Default::default()
            };
            child::read_json_lines(stdout, raw_log.as_ref(), |v| {
                reader_session.line(&v, &mut state, &sink);
            });
            reader_session.closed(&mut state, &sink);
        });

        Ok(session)
    }

    /// One line from stdout: the control channel's own lines, or the stream.
    pub(super) fn line(self: &Arc<Self>, v: &Value, state: &mut Translator, sink: &Sink) {
        match v.get("type").and_then(|t| t.as_str()) {
            Some("control_response") => self.settle(v),
            Some("control_request") => self.control_request(v, &state.agents, sink),
            Some("control_cancel_request") => self.control_cancelled(v, sink),
            _ => {
                for ev in state.translate(v) {
                    sink(ev);
                }
            }
        }
    }

    /// stdout reached EOF: fail what is open, then announce the exit.
    pub(super) fn closed(&self, state: &mut Translator, sink: &Sink) {
        *self.steering.lock().unwrap() = Steering::default();
        for ev in state.close() {
            sink(ev);
        }
        self.proc.finish(sink, Provider::Claude, || {
            let sent = self
                .expecting
                .swap(false, Ordering::SeqCst)
                .then(|| self.turns.take_sent())
                .flatten();
            let open = state.turn.take();
            // A send that raced a turn the CLI started itself: both
            // turns need their `TurnFinished`.
            if let (Some(sent), Some(_)) = (&sent, &open) {
                sink(HarnessEvent::TurnFinished {
                    status: "failed".into(),
                    turn: Some(sent.clone()),
                });
            }
            open.or(sent)
        });
    }

    pub fn is_alive(&self) -> bool {
        self.proc.is_alive()
    }

    /// One user turn. A plain line sent mid-turn is folded into the turn by
    /// the CLI (probe `11-steer-tool-plain`), which is why a send waits for
    /// `TurnFinished` and a correction goes through [`Self::steer`]. `bound`
    /// hears the turn's ordinal before the CLI hears the message.
    pub fn send(&self, text: &str, bound: &Bound) -> Result<(), String> {
        bound(self.turns.reserve());
        self.expecting.store(true, Ordering::SeqCst);
        self.proc.write_line(&serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": text },
            "parent_tool_use_id": null,
            "session_id": "",
        }))
    }

    /// Write a correction into the running turn: a user message with
    /// `priority: "now"` and `client` as its `uuid`, which the CLI echoes
    /// in `command_lifecycle` frames. Written only into a turn a send of
    /// ours opened, under the lock the reader closes that turn with, so a
    /// turn that has ended (or one the CLI started itself) is missed, never
    /// written to. It never sets `expecting`: the CLI takes it up inside
    /// the same turn, which ends with the steer's own `result`. A running
    /// tool finishes first; it is not cut short (probe `11-steer-tool`).
    pub fn steer(&self, text: &str, client: &str) -> crate::harness::Steered {
        use crate::harness::Steered;
        let mut steering = self.steering.lock().unwrap();
        if !steering.open {
            return Steered::Missed;
        }
        let written = self.proc.write_line(&serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": text },
            "parent_tool_use_id": null,
            "session_id": "",
            "priority": "now",
            "uuid": client,
        }));
        match written {
            Ok(()) => {
                steering.waiting.push(client.to_string());
                Steered::Accepted
            }
            Err(e) => Steered::Unknown(e),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_requests_summaries_without_overriding_the_updates_environment() {
        let mut cmd = Command::new("claude");
        configure_thinking(&mut cmd, false);
        assert_eq!(
            cmd.get_args().collect::<Vec<_>>(),
            ["--thinking", "adaptive", "--thinking-display", "summarized"]
        );
        assert_eq!(cmd.get_envs().count(), 0);
    }

    #[test]
    fn tool_less_jobs_keep_their_display_policy() {
        let mut cmd = Command::new("claude");
        configure_thinking(&mut cmd, true);
        assert_eq!(cmd.get_args().count(), 0);
        assert_eq!(
            cmd.get_envs().collect::<Vec<_>>(),
            vec![(
                std::ffi::OsStr::new("CLAUDE_CODE_THINKING_DISPLAY_UPDATES"),
                Some(std::ffi::OsStr::new("0"))
            )]
        );
    }
}
