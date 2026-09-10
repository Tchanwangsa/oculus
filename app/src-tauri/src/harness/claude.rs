//! The Claude Code bridge: one long-lived `claude -p` process per thread.
//!
//! The CLI is run in its stream-json mode — user turns go in as one JSON line
//! each on stdin, and everything it does comes back as JSON lines on stdout:
//! Anthropic's own stream events for live text, a full `assistant` message per
//! block, a `user` message per tool result, and a `result` line closing each
//! turn. The process stays up between turns, which is what makes the second
//! message cheap; a thread whose process has gone is resumed by session id
//! with `--resume`, which the CLI persists itself under `~/.claude`.
//!
//! Permission prompts have nowhere to go from here yet, so they are
//! configured to auto-deny (`--permission-prompts none`) rather than block:
//! under `-p` a prompt with no answerer would hang the turn forever. So what
//! the agent may do has to be settled up front, and it is settled three ways
//! at once (`settings_json`), each measured against the library:
//!
//! - `--add-dir <library>` opens the whole library to reads. Without it the
//!   CLI scopes *listing* to the cwd, and `ls ../courses` is refused.
//! - Claude's own sandbox (`sandbox.enabled`) runs every Bash command inside
//!   a seatbelt profile whose only writable root is the cwd, and auto-allows
//!   Bash while it does. That is what turns `echo x > ../courses/f` into
//!   "operation not permitted" instead of a file, and what lets a piped
//!   `oculus files | head` run without an approval it could never get.
//! - Deny rules on `Edit` for every sibling of `agents/`, because `--add-dir`
//!   would otherwise put the courses inside `acceptEdits`' reach. Deny beats
//!   allow in the CLI's rule order, so the siblings are named rather than
//!   the root denied and `agents/` re-allowed.
//!
//! Auto-memory is switched off in the same settings: the library has its own
//! memory layer under `agents/`, and asked to write there the CLI reached for
//! `~/.claude/projects/…/memory/` instead.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::event::{cap_output, classify, HarnessEvent, RateWindow};
use super::{RawLog, Sink};

pub struct ClaudeSpawn {
    pub bin: PathBuf,
    /// The library's `agents/` folder — the only writable root.
    pub cwd: PathBuf,
    /// The library root, opened for reads with `--add-dir`.
    pub library: PathBuf,
    /// Resume this session rather than starting one.
    pub resume: Option<String>,
    pub model: Option<String>,
    /// `--effort`: one of `low`, `medium`, `high`, `xhigh`, `max`. Left off
    /// when None, which leaves the model's own default in charge. Unlike
    /// Codex, which takes an effort per turn, this is fixed for the process,
    /// so a level chosen mid-thread applies from the next resume.
    pub effort: Option<String>,
    /// `default`, `acceptEdits`, `plan`, `bypassPermissions`.
    pub permission_mode: String,
    /// Appended to the CLI's own system prompt.
    pub system_append: String,
    pub env: Vec<(String, String)>,
    pub raw_log: Option<RawLog>,
}

pub struct ClaudeSession {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    alive: Arc<AtomicBool>,
    request_ids: AtomicU64,
}

impl ClaudeSession {
    pub fn spawn(cfg: ClaudeSpawn, sink: Sink) -> Result<Arc<Self>, String> {
        let mut cmd = Command::new(&cfg.bin);
        cmd.arg("-p")
            .args(["--input-format", "stream-json"])
            .args(["--output-format", "stream-json"])
            .arg("--verbose")
            .arg("--include-partial-messages")
            .args(["--permission-mode", &cfg.permission_mode])
            .args(["--permission-prompts", "none"]);
        if let Some(m) = &cfg.model {
            cmd.args(["--model", m]);
        }
        if let Some(e) = &cfg.effort {
            cmd.args(["--effort", e]);
        }
        if let Some(id) = &cfg.resume {
            cmd.args(["--resume", id]);
        }
        if !cfg.system_append.trim().is_empty() {
            cmd.args(["--append-system-prompt", &cfg.system_append]);
        }
        cmd.args(["--add-dir", &cfg.library.display().to_string()]);
        cmd.args(["--settings", &settings_json(&cfg.library, &cfg.cwd)]);
        cmd.current_dir(&cfg.cwd)
            .env_clear()
            .envs(cfg.env.iter().map(|(k, v)| (k, v)))
            // bb sets this too: the CLI gates some behaviour on how it was
            // entered, and "cli" is the interactive-install path.
            .env("CLAUDE_CODE_ENTRYPOINT", "cli")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", cfg.bin.display()))?;
        let stdin = child.stdin.take().ok_or("no stdin on claude child")?;
        let stdout = child.stdout.take().ok_or("no stdout on claude child")?;
        let stderr = child.stderr.take().ok_or("no stderr on claude child")?;

        let alive = Arc::new(AtomicBool::new(true));
        let session = Arc::new(ClaudeSession {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            alive: alive.clone(),
            request_ids: AtomicU64::new(1),
        });

        // stderr is the CLI's own log. Keep a tail so a process that dies
        // before saying anything on stdout can still explain itself.
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

        let reader_session = session.clone();
        let raw_log = cfg.raw_log;
        std::thread::spawn(move || {
            let mut state = Translator::default();
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(log) = &raw_log {
                    log.write(&line);
                }
                let Ok(v) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                for ev in state.translate(&v) {
                    sink(ev);
                }
            }
            // EOF: the process is gone or going. Reap it for the code.
            alive.store(false, Ordering::SeqCst);
            let code = reader_session
                .child
                .lock()
                .unwrap()
                .wait()
                .ok()
                .and_then(|s| s.code());
            if !state.turn_open_closed_cleanly() {
                let tail = stderr_tail.lock().unwrap().join("\n");
                let msg = if tail.trim().is_empty() {
                    format!("claude exited (code {code:?}) mid-turn")
                } else {
                    format!("claude exited (code {code:?}) mid-turn:\n{tail}")
                };
                sink(HarnessEvent::error(msg));
                sink(HarnessEvent::TurnFinished {
                    status: "failed".into(),
                });
            }
            sink(HarnessEvent::Exited { code });
        });

        Ok(session)
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
            .map_err(|e| format!("claude stdin: {e}"))
    }

    /// One user turn. The CLI accepts the next line as soon as the previous
    /// turn's `result` is out; sending mid-turn queues it as a steer.
    pub fn send(&self, text: &str) -> Result<(), String> {
        self.write_line(&serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": text },
            "parent_tool_use_id": null,
            "session_id": "",
        }))
    }

    /// Stop the current turn without ending the session. The CLI answers
    /// with a `control_response` and closes the turn with a `result`.
    pub fn interrupt(&self) -> Result<(), String> {
        let id = self.request_ids.fetch_add(1, Ordering::SeqCst);
        self.write_line(&serde_json::json!({
            "type": "control_request",
            "request_id": format!("oculus-{id}"),
            "request": { "subtype": "interrupt" },
        }))
    }

    pub fn kill(&self) {
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
        self.alive.store(false, Ordering::SeqCst);
    }
}

impl Drop for ClaudeSession {
    fn drop(&mut self) {
        if let Ok(mut c) = self.child.lock() {
            let _ = c.kill();
        }
    }
}

/// The inline `--settings` document: see the module docs for what each part
/// buys. `//` prefixes an absolute path in the CLI's rule syntax. The root's
/// files are named by suffix rather than with a bare `*`: measured, `*.log`
/// at the root leaves `agents/memories/x.log` alone, but a bare `*` denied
/// every write under `agents/` too.
fn settings_json(library: &std::path::Path, cwd: &std::path::Path) -> String {
    let root = library.display().to_string();
    let abs = root.trim_start_matches('/');
    let deny: Vec<String> = [
        "courses/**",
        "lectures/**",
        "canvas-session/**",
        "oculus.db*",
        "*.cookie",
        "*.token",
        "*.json",
        "*.log",
    ]
        .iter()
        .map(|p| format!("Edit(//{abs}/{p})"))
        .collect();
    serde_json::json!({
        "permissions": { "deny": deny },
        "sandbox": {
            "enabled": true,
            "failIfUnavailable": false,
            "autoAllowBashIfSandboxed": true,
            "allowUnsandboxedCommands": false,
            "network": { "allowLocalBinding": true },
            "filesystem": { "allowWrite": [cwd.display().to_string()] },
        },
        "autoMemoryEnabled": false,
    })
    .to_string()
}

// ── Translation ──────────────────────────────────────────────────────────────

/// Per-process translation state. Small on purpose: the stream is almost
/// stateless, and what state there is exists to dedupe — a tool_use block
/// can appear in more than one `assistant` line of the same message.
#[derive(Default)]
struct Translator {
    started_tools: std::collections::HashSet<String>,
    /// Between the first stream event of a turn and its `result`.
    turn_open: bool,
    /// Whether anything was ever received: an EOF before the first line is
    /// a spawn failure, not a mid-turn death.
    saw_result: bool,
    /// What the last request occupied — `result.usage` sums every request in
    /// the turn, which is spend, not context.
    last_context_tokens: Option<u64>,
}

impl Translator {
    fn turn_open_closed_cleanly(&self) -> bool {
        !self.turn_open
    }

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
                        // A start can carry a prefix of text; the deltas that
                        // follow do not repeat it.
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
                                // Tool inputs are never streamed to the
                                // timeline: the complete call arrives on the
                                // `assistant` line right after.
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
                self.open_turn(&mut out);
                if let Some(u) = v.pointer("/message/usage") {
                    let n = |k: &str| u.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
                    let ctx = n("input_tokens") + n("cache_read_input_tokens") + n("cache_creation_input_tokens");
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
                // Only tool results come back this way; the user's own text
                // is what we sent.
                let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) else {
                    return out;
                };
                for block in content {
                    if block.get("type").and_then(|t| t.as_str()) != Some("tool_result") {
                        continue;
                    }
                    let id = str_of(block, "tool_use_id");
                    let is_error = block.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false);
                    let output = tool_result_text(block, v.get("tool_use_result"));
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        ok: !is_error,
                        output: cap_output(&output),
                    });
                }
            }
            "result" => {
                self.saw_result = true;
                let is_error = v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false);
                let subtype = v.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
                if let Some(u) = v.get("usage") {
                    let n = |k: &str| u.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
                    let input = n("input_tokens");
                    let cached = n("cache_read_input_tokens") + n("cache_creation_input_tokens");
                    let context_window = v
                        .get("modelUsage")
                        .and_then(|m| m.as_object())
                        .and_then(|m| m.values().filter_map(|x| x.get("contextWindow")?.as_u64()).max());
                    out.push(HarnessEvent::Usage {
                        input_tokens: input + cached,
                        output_tokens: n("output_tokens"),
                        context_tokens: self.last_context_tokens,
                        context_window,
                        cost_usd: v.get("total_cost_usd").and_then(|c| c.as_f64()),
                    });
                }
                if is_error {
                    let msg = v
                        .get("result")
                        .and_then(|r| r.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .or_else(|| {
                            v.get("errors")
                                .and_then(|e| e.as_array())
                                .map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("\n"))
                        })
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| format!("claude: {subtype}"));
                    out.push(HarnessEvent::error(msg));
                }
                let interrupted = matches!(
                    v.get("stop_reason").and_then(|s| s.as_str()),
                    Some("interrupted") | Some("interrupt")
                ) || subtype.contains("interrupt");
                let status = if interrupted {
                    "interrupted"
                } else if is_error {
                    "failed"
                } else {
                    "completed"
                };
                self.turn_open = false;
                out.push(HarnessEvent::TurnFinished {
                    status: status.into(),
                });
            }
            "rate_limit_event" => {
                if let Some(w) = v.pointer("/rate_limit_info/unifiedWindows").and_then(|w| w.as_object()) {
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
                            used_percent: win.get("utilization").and_then(|u| u.as_f64()).unwrap_or(0.0) * 100.0,
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

fn str_of(v: &Value, key: &str) -> String {
    v.get(key).and_then(|s| s.as_str()).unwrap_or("").to_string()
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

    /// Replays a recorded `claude -p` session and checks the folded shape.
    /// The fixture is the real output of `claude 2.1.267` asked to `ls` the
    /// library and describe it, captured with the flags `spawn` uses.
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
            HarnessEvent::SessionStarted { provider_session_id, .. } => Some(provider_session_id.clone()),
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

        let finished = events.iter().filter(|e| matches!(e, HarnessEvent::ToolFinished { ok: true, .. })).count();
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
        assert_eq!(message.as_deref(), Some(deltas.as_str()), "deltas add up to the message");

        assert!(matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "completed"));
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::Usage { cost_usd: Some(_), .. })));
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::RateLimits { windows } if windows.len() == 2)));
    }
}
