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
//!   That auto-allow is the analyser's judgement of each command's *shape*,
//!   though, and it does not stretch to a plan: a multi-line `--brief`, a loop
//!   over subjects or a compound line falls through to a prompt nobody can
//!   answer, and the denial sticks for the rest of the session. So
//!   `Bash(oculus:*)` is allowed by name as well — the one binary a thread is
//!   meant to write the board through, cleared however the command is shaped.
//!   The one exception is the database, which is writable *as three files*
//!   (`oculus.db` and its WAL sidecars) because `oculus project` / `oculus
//!   task` are how a plan becomes the board's rows, and SQLite answers a
//!   sandbox that will not let it touch `oculus.db-wal` with "attempt to
//!   write a readonly database" — which is what every `oculus task add` from
//!   a thread used to do. An `Edit` deny on `oculus.db*` cannot be the thing
//!   that keeps the file tools off it: the CLI merges `Edit(...)` deny rules
//!   into the sandbox's own `denyWrite`, so that rule denied the database at
//!   the OS level too and cancelled the three paths it had just been given —
//!   deny beats allow, so the sandbox half of this fix could never land while
//!   the deny stood. `sqlite3` is denied by name instead: the CLI is the only
//!   door, because it is the only thing that knows what a valid row is.
//! - Deny rules on `Edit` for every sibling of `agents/`, because `--add-dir`
//!   would otherwise put the courses inside `acceptEdits`' reach. Deny beats
//!   allow in the CLI's rule order, so the siblings are named rather than
//!   the root denied and `agents/` re-allowed. Three paths *inside*
//!   `agents/` are denied too — `skills/` and the `.claude/skills` and
//!   `.agents/skills` links the scanning CLIs find them through
//!   (`crate::agents`) — since all are generated and a thread could otherwise
//!   edit the procedures it runs under.
//!
//! Auto-memory is switched off in the same settings: the library has its own
//! memory layer under `agents/`, and asked to write there the CLI reached for
//! `~/.claude/projects/…/memory/` instead.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use super::event::{cap_output, classify, HarnessEvent, Provider, RateWindow};
use super::{RawLog, Sink};

pub struct ClaudeSpawn {
    pub bin: PathBuf,
    /// The library's `agents/` folder — the only writable root.
    pub cwd: PathBuf,
    /// The library root, opened for reads with `--add-dir`.
    pub library: PathBuf,
    /// Where the `oculus` binary actually is, so the permission rule can name
    /// the absolute path as well as the bare command. `None` only when
    /// discovery found nothing, in which case the name is all there is.
    pub oculus: Option<PathBuf>,
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
    /// Set between asking the CLI to stop and the `result` that answers.
    /// The translator reads it, because an interrupted turn is not something
    /// the `result` line says plainly — see the `result` arm below.
    interrupting: Arc<AtomicBool>,
    /// Control requests waiting on their `control_response`, by request id.
    /// `interrupt` does not wait — it is answered by the `result` that
    /// follows — but a rewind has to know whether it actually happened
    /// before the rows are deleted on the strength of it.
    pending: Mutex<HashMap<String, mpsc::Sender<Value>>>,
    /// Set between a message going in and the `result` that closes its turn.
    /// A process that dies in that window has to close the turn anyway:
    /// upstream a thread is only released for its next message by a
    /// `TurnFinished` (`Queue` in the manager), and the window is a real one
    /// — a model name the CLI rejects kills it before the first stream
    /// event, which is the point at which `turn_open` would otherwise notice.
    expecting: Arc<AtomicBool>,
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
        cmd.args(["--settings", &settings_json(&cfg.library, &cfg.cwd, cfg.oculus.as_deref())]);
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
        let interrupting = Arc::new(AtomicBool::new(false));
        let expecting = Arc::new(AtomicBool::new(false));
        let session = Arc::new(ClaudeSession {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            alive: alive.clone(),
            request_ids: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            interrupting: interrupting.clone(),
            expecting: expecting.clone(),
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
            let mut state = Translator {
                interrupting,
                expecting: expecting.clone(),
                ..Default::default()
            };
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(log) = &raw_log {
                    log.write(&line);
                }
                let Ok(v) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if v.get("type").and_then(|t| t.as_str()) == Some("control_response") {
                    reader_session.settle(&v);
                    continue;
                }
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
            if expecting.swap(false, Ordering::SeqCst) || !state.turn_open_closed_cleanly() {
                let tail = stderr_tail.lock().unwrap().join("\n");
                let msg = if tail.trim().is_empty() {
                    format!("claude exited (code {code:?}) mid-turn")
                } else {
                    format!("claude exited (code {code:?}) mid-turn:\n{tail}")
                };
                sink(HarnessEvent::error_for(Provider::Claude, msg));
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

    /// One user turn. The CLI takes the next line as soon as the previous
    /// turn's `result` is out — and takes one mid-turn too, queueing it
    /// itself and running it the instant the current turn ends, which is why
    /// the manager holds messages back rather than letting them through.
    pub fn send(&self, text: &str) -> Result<(), String> {
        self.expecting.store(true, Ordering::SeqCst);
        self.write_line(&serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": text },
            "parent_tool_use_id": null,
            "session_id": "",
        }))
    }

    /// Stop the current turn without ending the session. The CLI answers
    /// with a `control_response`, emits the half-written assistant message as
    /// an ordinary `assistant` line, and closes the turn with a `result` that
    /// calls itself an error. The flag is how the translator tells that one
    /// apart from a real failure.
    pub fn interrupt(&self) -> Result<(), String> {
        self.interrupting.store(true, Ordering::SeqCst);
        let id = self.request_ids.fetch_add(1, Ordering::SeqCst);
        self.write_line(&serde_json::json!({
            "type": "control_request",
            "request_id": format!("oculus-{id}"),
            "request": { "subtype": "interrupt" },
        }))
    }

    /// Hand a `control_response` to whoever is waiting on it. A response
    /// nobody asked about — the one an `interrupt` gets — is dropped.
    fn settle(&self, v: &Value) {
        let Some(id) = v.pointer("/response/request_id").and_then(|s| s.as_str()) else {
            return;
        };
        if let Some(tx) = self.pending.lock().unwrap().remove(id) {
            let _ = tx.send(v.clone());
        }
    }

    /// Drop a question and everything after it from the CLI's *own* session,
    /// so the agent's context matches the thread the student is reading.
    ///
    /// `target_message_uuid` is the CLI's id for the user message, which it
    /// never puts on stdout — it is read out of the session transcript when
    /// the turn ends ([`anchor_for`]) and kept on the row.
    ///
    /// This waits for the `control_response`, unlike every other line written
    /// here: the rows are deleted on the strength of the answer, so a rewind
    /// that quietly did nothing would put the thread and the agent back out
    /// of step in the one place the student is guaranteed to notice.
    pub fn rewind(&self, target_message_uuid: &str) -> Result<(), String> {
        let id = format!("oculus-{}", self.request_ids.fetch_add(1, Ordering::SeqCst));
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id.clone(), tx);
        let sent = self.write_line(&serde_json::json!({
            "type": "control_request",
            "request_id": id,
            "request": {
                "subtype": "rewind_conversation",
                "target_message_uuid": target_message_uuid,
                // The manager only rewinds a thread that is between turns, so
                // there is nothing running to cut short.
                "interrupt_if_running": false,
            },
        }));
        let answer = sent.and_then(|()| {
            rx.recv_timeout(Duration::from_secs(30))
                .map_err(|_| "claude did not answer the rewind".to_string())
        });
        // A write that failed and a wait that timed out both leave the id in
        // the map, where it would hold a sender for a response that is never
        // coming.
        self.pending.lock().unwrap().remove(&id);
        let v = answer?;
        if v.pointer("/response/subtype").and_then(|s| s.as_str()) != Some("success") {
            let why = v
                .pointer("/response/error")
                .and_then(|e| e.as_str())
                .unwrap_or("refused");
            return Err(format!("claude would not rewind: {why}"));
        }
        if v.pointer("/response/response/rewound").and_then(|b| b.as_bool()) == Some(false) {
            return Err("claude found nothing to rewind to".into());
        }
        Ok(())
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

/// One row of the CLI's own `/model` catalogue, as `initialize` reports it.
/// Raw on purpose: which of `value` and `resolvedModel` becomes the id the
/// picker stores, and how a row is labelled, is the frontend's adapter
/// (`claudeAsModels` in `app/src/lib/harness.ts`), next to Codex's and
/// opencode's.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// What `/model` would pass on: an alias (`sonnet`, `opus[1m]`,
    /// `default`) or a full name (`claude-fable-5-1[1m]`).
    pub value: String,
    /// The concrete model the alias stands for today.
    pub resolved_model: String,
    pub display_name: String,
    pub description: String,
    /// Empty for a model that takes no `--effort` (Haiku).
    pub supported_effort_levels: Vec<String>,
}

/// How long the model probe may take. It answers in well under a second; this
/// is for a CLI that is wedged on something — a login prompt, an update — not
/// for a slow one.
const MODELS_TIMEOUT: Duration = Duration::from_secs(20);

/// Ask the installed CLI which models it offers, without starting a turn.
///
/// The catalogue rides the answer to the SDK's `initialize` control request —
/// the handshake the Agent SDK opens every session with — so one line in and
/// one `control_response` out is the whole exchange, and no API call is made.
/// It is a throwaway process rather than a question put to a live session
/// because a session only exists per thread, and the picker needs the list
/// before there is one. The flags keep that process inert: no hooks, no MCP
/// servers, nothing written to `~/.claude`. `cwd` is the threads' own folder,
/// so the catalogue is the one a thread will actually be offered.
///
/// The child is killed rather than left to exit: in stream-json mode it sits
/// waiting for a user message that is never coming.
pub fn list_models(
    bin: &Path,
    cwd: &Path,
    env: &[(String, String)],
) -> Result<Vec<ModelInfo>, String> {
    const REQUEST_ID: &str = "oculus-models";
    let mut child = Command::new(bin)
        .arg("-p")
        .args(["--input-format", "stream-json"])
        .args(["--output-format", "stream-json"])
        .arg("--verbose")
        .arg("--no-session-persistence")
        .arg("--strict-mcp-config")
        .args(["--settings", r#"{"disableAllHooks":true}"#])
        .current_dir(cwd)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        // As the bridge does, so the catalogue is the one a thread gets.
        .env("CLAUDE_CODE_ENTRYPOINT", "cli")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start {}: {e}", bin.display()))?;

    // Held until the child is killed: closing stdin is end-of-input, and a
    // CLI that sees it first may leave without answering.
    let mut stdin = child.stdin.take().ok_or("no stdin on claude child")?;
    let stdout = child.stdout.take().ok_or("no stdout on claude child")?;
    let stderr = child.stderr.take().ok_or("no stderr on claude child")?;

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
        // EOF with no answer: the process is gone.
        let _ = tx.send(None);
    });

    let request = serde_json::json!({
        "type": "control_request",
        "request_id": REQUEST_ID,
        "request": { "subtype": "initialize" },
    });
    let written = stdin
        .write_all(format!("{request}\n").as_bytes())
        .and_then(|_| stdin.flush())
        .map_err(|e| format!("claude stdin: {e}"));
    let answer = written.and_then(|()| {
        rx.recv_timeout(MODELS_TIMEOUT).map_err(|_| {
            format!(
                "claude did not list its models within {}s",
                MODELS_TIMEOUT.as_secs()
            )
        })
    });

    let _ = child.kill();
    let code = child.wait().ok().and_then(|s| s.code());
    drop(stdin);
    match answer? {
        Some(result) => result,
        None => {
            let tail = stderr_tail.lock().unwrap().join("\n");
            Err(if tail.trim().is_empty() {
                format!("claude exited (code {code:?}) before listing its models")
            } else {
                format!("claude exited (code {code:?}) before listing its models:\n{tail}")
            })
        }
    }
}

/// The catalogue out of one stdout line, or None when the line is not the
/// answer to `request_id`. Tolerant per row: a field a future CLI drops reads
/// as empty, and a row with neither name is skipped rather than failing the
/// list — the adapter decides what an empty field costs.
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
    let Some(rows) = v.pointer("/response/response/models").and_then(|m| m.as_array()) else {
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
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default(),
        })
        .filter(|m| !m.value.is_empty() || !m.resolved_model.is_empty())
        .collect()))
}

/// The inline `--settings` document: see the module docs for what each part
/// buys. `//` prefixes an absolute path in the CLI's rule syntax. The root's
/// files are named by suffix rather than with a bare `*`: measured, `*.log`
/// at the root leaves `agents/memories/x.log` alone, but a bare `*` denied
/// every write under `agents/` too.
fn settings_json(
    library: &std::path::Path,
    cwd: &std::path::Path,
    oculus: Option<&std::path::Path>,
) -> String {
    let root = library.display().to_string();
    let abs = root.trim_start_matches('/');
    let mut deny: Vec<String> = [
        "courses/**",
        "lectures/**",
        "canvas-session/**",
        // `oculus.db*` is deliberately NOT denied here. The CLI merges
        // `Edit(...)` deny rules into the sandbox's own `denyWrite` — "Merged
        // with paths from Edit(...) deny permission rules", its settings
        // schema — so denying the database to the file tools denied it at the
        // OS level as well, cancelling the three `allowWrite` paths below and
        // handing every `oculus task add` from a thread SQLite's "attempt to
        // write a readonly database". The two halves of the fix were fighting
        // each other. `Bash(sqlite3:*)` below is what still guards the row
        // format: the CLI stays the only door.
        "*.cookie",
        "*.token",
        "*.json",
        "*.log",
        // The paths inside the writable root that are not the agent's work
        // but the app's: the generated skills, and both directories a
        // scanning CLI discovers them through. `agents/` is the one place a
        // thread may write, so without these the agent can rewrite the
        // procedures it is about to follow — and the next `oculus docs` would
        // silently put them back, which is a confusing way to lose an
        // afternoon. `.agents/` is denied here as well as in Codex's own
        // sandbox: a thread's rules are about what *this* thread may touch,
        // not about which CLI is running it.
        "agents/skills/**",
        "agents/.claude/**",
        "agents/.agents/**",
    ]
        .iter()
        .map(|p| format!("Edit(//{abs}/{p})"))
        .collect();
    // The database is writable at the OS level (see `allowWrite` below), so
    // the one command that could go around the CLI with it is named here.
    // `oculus task` knows that a column id must exist and that a breakdown is
    // one transaction; a hand-written `UPDATE` knows neither, and a mangled
    // board is the one thing in the library that no re-sync repairs.
    deny.push("Bash(sqlite3:*)".to_string());

    // `autoAllowBashIfSandboxed` clears a Bash call only when the CLI's own
    // analyser can statically vouch for the command, and a plan is the thing it
    // cannot vouch for: a `--brief` with newlines in it, a loop over subjects, a
    // compound line. Those fall through to an approval prompt that
    // `--permission-prompts none` then denies — and the denial is *sticky*, so
    // one long brief costs the thread every write it had left. Measured: of 39
    // `oculus` calls in one thread 36 cleared the analyser, and the 3 that did
    // not included the `project create` the whole turn was for. So the board's
    // own door is allowed by name rather than by the shape of each command,
    // which is what opencode's ruleset already does (`oculus`, `oculus *`) and
    // what Codex gets for free from `approvalPolicy: never`. This is a prompt
    // rule, not a sandbox one: the seatbelt below still bounds what the command
    // may touch, and deny still beats allow, so `sqlite3` stays shut.
    let mut allow = vec!["Bash(oculus:*)".to_string()];
    // And by absolute path, when discovery knows it. The match is on the
    // command *name*, so `/…/target/release/oculus project create` is not
    // `oculus` and falls straight through this rule — into an approval prompt
    // that `--permission-prompts none` denies without a word and that sticks
    // for the rest of the session. A thread reaches for the full path more
    // often than it looks: any note or transcript that once learned it while
    // the bare name was broken keeps using it long after the name is fixed.
    if let Some(cli) = oculus {
        allow.push(format!("Bash({}:*)", cli.display()));
    }

    // The cwd — `agents/` — plus the database's three files. Nothing else in
    // the library is writable from a thread; see `paths::db_write_paths` for
    // why it is the files and not the folder they are in.
    let write: Vec<String> = std::iter::once(cwd.display().to_string())
        .chain(
            crate::paths::db_write_paths(library)
                .iter()
                .map(|p| p.display().to_string()),
        )
        .collect();
    serde_json::json!({
        "permissions": { "allow": allow, "deny": deny },
        "sandbox": {
            "enabled": true,
            "failIfUnavailable": false,
            "autoAllowBashIfSandboxed": true,
            "allowUnsandboxedCommands": false,
            "network": { "allowLocalBinding": true },
            "filesystem": { "allowWrite": write },
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
    /// Shared with the session: whether the turn being closed was stopped on
    /// purpose.
    interrupting: Arc<AtomicBool>,
    /// Shared with the session: a turn is owed a `result`. Cleared here, on
    /// the `result` itself.
    expecting: Arc<AtomicBool>,
    /// From the `init` line: what this session's transcript is called and
    /// which working directory files it under. Together they are the only
    /// way to the uuid a rewind needs, since the uuid of the question never
    /// comes back on stdout.
    session_id: String,
    cwd: String,
    /// The first `assistant` line of the open turn. Its ancestry in the
    /// transcript names the question that started the turn — see
    /// [`anchor_for`].
    turn_first_assistant: Option<String>,
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
                if self.turn_first_assistant.is_none() {
                    self.turn_first_assistant = v.get("uuid").and_then(|u| u.as_str()).map(String::from);
                }
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
                        title: None,
                    });
                }
            }
            "result" => {
                self.saw_result = true;
                let is_error = v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false);
                let subtype = v.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
                // A stopped turn does not say so. Recorded (the fixture this
                // line's test replays): `is_error: true`, `subtype:
                // "error_during_execution"`, `stop_reason: null`, and an
                // `errors` array holding the CLI's own diagnostic —
                // `[ede_diagnostic] result_type=user …`, which is the string
                // that was reaching the timeline as a red row every time the
                // student pressed stop. `terminal_reason: "aborted_streaming"`
                // is the only field that names it, so that and the flag we
                // set when we asked are what decide; the rest is a fallback
                // for a CLI that words it differently.
                let interrupted = self.interrupting.swap(false, Ordering::SeqCst)
                    || v.get("terminal_reason")
                        .and_then(|t| t.as_str())
                        .is_some_and(|t| t.starts_with("aborted"))
                    || matches!(
                        v.get("stop_reason").and_then(|s| s.as_str()),
                        Some("interrupted") | Some("interrupt")
                    )
                    || subtype.contains("interrupt");
                // An interrupted result reports zeros for everything —
                // `duration_api_ms: 0`, no tokens, no cost. Folding that in
                // would blank the thread's usage for a turn that did happen.
                if let Some(u) = v.get("usage").filter(|_| !interrupted) {
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
                if is_error && !interrupted {
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
                    out.push(HarnessEvent::error_for(Provider::Claude, msg));
                }
                let status = if interrupted {
                    "interrupted"
                } else if is_error {
                    "failed"
                } else {
                    "completed"
                };
                // The question's uuid is readable now that the turn's rows
                // are on disk, and this is the last moment it can be had:
                // the transcript is walked back from this turn's first
                // answer, and the next turn would move that landmark.
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

/// Where the CLI keeps a session's transcript.
///
/// It is not announced: `memory_paths` on the `init` line would give the
/// folder away, but it is null whenever auto-memory is off, which is how this
/// bridge runs it (`settings_json`). So the path is rebuilt the way the CLI
/// builds it — every character of the working directory that is not a letter
/// or a digit becomes `-`, measured against real folders rather than assumed.
/// A slug that does not resolve falls back to finding the file by name, since
/// the session id is unique across projects and the rule is the CLI's to
/// change.
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

/// The uuid of the question a turn answered, read out of the transcript.
///
/// The transcript is a tree, not a list: every row names its `parentUuid`,
/// and a turn's rows hang off the question that started it. So the walk goes
/// up from the turn's first answer until it reaches a `user` row — skipping
/// the attachments the CLI threads in between, and skipping `user` rows that
/// are tool results rather than anything a student typed.
///
/// With no answer to start from — an interrupted turn can produce none — the
/// newest question in the file is taken instead. That is this turn's: the
/// manager runs one at a time, and the file has just been written.
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
                parent: v.get("parentUuid").and_then(|p| p.as_str()).map(String::from),
                question,
            },
        );
    }
    let Some(start) = first_assistant else {
        return newest_question;
    };
    let mut at = start.to_string();
    // Bounded by the file: a malformed parent chain must not loop forever.
    for _ in 0..by_uuid.len() {
        let row = by_uuid.get(&at)?;
        if row.question {
            return Some(at);
        }
        at = row.parent.clone()?;
    }
    None
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

    /// The containment, as the CLI will read it — the counterpart of
    /// opencode's `the_rendered_config_denies_the_right_things`.
    ///
    /// Two halves that have to agree: the seatbelt may write the thread's cwd
    /// and the database's three files and nothing else, and the file tools are
    /// denied every sibling of `agents/` *including* the database, so the only
    /// way a row reaches the board is the `oculus` CLI. `sqlite3` is named
    /// because the sandbox can no longer stop it.
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

        let deny: Vec<&str> = v
            .pointer("/permissions/deny")
            .and_then(|d| d.as_array())
            .unwrap()
            .iter()
            .filter_map(|r| r.as_str())
            .collect();
        for rule in [
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/courses/**)",
            // Inside the writable root, and the reason they have to be named:
            // everything else in `agents/` is the agent's to write.
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/agents/skills/**)",
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/agents/.claude/**)",
            "Bash(sqlite3:*)",
        ] {
            assert!(deny.contains(&rule), "missing {rule} in {deny:?}");
        }

        // And the database must NOT be denied, however tempting it looks
        // beside the others: an `Edit(...)` deny is merged into the sandbox's
        // `denyWrite`, so this one rule cancels the `allowWrite` paths above
        // and every board write from a thread fails as readonly.
        assert!(
            !deny.iter().any(|r| r.contains("oculus.db")),
            "oculus.db must stay out of deny — it cancels allowWrite: {deny:?}"
        );

        // The CLI is allowed by name, because the sandbox's own auto-allow only
        // clears commands its analyser can vouch for and a breakdown is not one
        // of those. `sqlite3` must not have followed it in: deny beats allow,
        // but only while the two lists stay this far apart.
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

        // Given the binary's location, the same door is allowed by path too:
        // a thread that calls the CLI by its full path matches no name rule
        // and is denied silently.
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

    /// Replays a recorded `claude -p` session and checks the folded shape.
    /// The fixture is the real output of `claude 2.1.267` asked to `ls` the
    /// library and describe it, captured with the flags `spawn` uses.
    /// A transcript in the shape the CLI writes one: a question, the
    /// attachments it threads in after it, the answer, then a tool result
    /// that is also a `user` row and must not be mistaken for a question.
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

    /// The question a turn answered is found by walking the transcript's
    /// parent chain up from the turn's first answer — past the attachments
    /// the CLI inserts, and never stopping on a tool result.
    #[test]
    fn the_anchor_is_the_question_the_answer_hangs_off() {
        let dir = std::env::temp_dir().join(format!("oculus-anchor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = transcript(&dir);

        assert_eq!(anchor_for(&path, Some("a2")).as_deref(), Some("q2"));
        assert_eq!(anchor_for(&path, Some("a1")).as_deref(), Some("q1"));
        // A later answer in the same turn walks back through the tool result
        // to the same question, not to the tool row.
        assert_eq!(anchor_for(&path, Some("a3")).as_deref(), Some("q2"));
        // An interrupted turn can produce no answer at all; the newest
        // question in the file is this turn's.
        assert_eq!(anchor_for(&path, None).as_deref(), Some("q2"));
        // An answer the file has never heard of anchors nothing rather than
        // guessing.
        assert_eq!(anchor_for(&path, Some("nope")), None);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// The transcript is filed under the working directory with every
    /// character that is not a letter or a digit replaced — measured against
    /// the CLI's own folders, including the double dash a dotfile produces.
    #[test]
    fn the_transcript_slug_flattens_everything_but_letters_and_digits() {
        let dir = std::env::temp_dir().join(format!("oculus-slug-{}", std::process::id()));
        let projects = dir.join("projects").join("-tmp-a-b--claude-c-d");
        std::fs::create_dir_all(&projects).unwrap();
        std::fs::write(projects.join("sess.jsonl"), "").unwrap();
        std::env::set_var("CLAUDE_CONFIG_DIR", &dir);

        let found = transcript_path("/tmp/a b/.claude/c_d", "sess");
        assert_eq!(found.as_deref(), Some(projects.join("sess.jsonl").as_path()));
        // A session that is nowhere under `projects` is not invented.
        assert_eq!(transcript_path("/tmp/a b/.claude/c_d", "gone"), None);

        std::env::remove_var("CLAUDE_CONFIG_DIR");
        std::fs::remove_dir_all(&dir).ok();
    }

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

    /// A turn stopped mid-answer. The recording is a real one: the CLI sends
    /// the half-written text as an ordinary `assistant` line, then closes the
    /// turn with a `result` that calls itself an error and carries its own
    /// diagnostic — `[ede_diagnostic] result_type=user …`. That string was
    /// reaching the timeline as a red row every time stop was pressed, and
    /// the zeroed usage on the same line was blanking the thread's numbers.
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
            !events.iter().any(|e| matches!(e, HarnessEvent::Error { .. })),
            "the CLI's own diagnostic is not something the student did"
        );
        assert!(
            !events.iter().any(|e| matches!(e, HarnessEvent::Usage { .. })),
            "an interrupted result reports zeros; folding them in blanks the meter"
        );
        // What the agent had already said is a row like any other, so it
        // survives the turn ending and the reload after it.
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
        assert!(matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "interrupted"));
    }

    /// The `initialize` answer from CLI 2.1.281, cut down to its models (the
    /// real line also carries the commands, agents and account). Haiku is the
    /// row that declares no effort levels at all, and must still arrive.
    #[test]
    fn the_initialize_answer_lists_the_models() {
        let line = r#"{"type":"control_response","response":{"subtype":"success","request_id":"oculus-models","response":{"models":[
            {"value":"default","resolvedModel":"claude-opus-5-5[1m]","displayName":"Default (recommended)","description":"Opus 5.5 with 1M context · Best for everyday, complex tasks","supportsEffort":true,"supportedEffortLevels":["low","medium","high","xhigh","max"]},
            {"value":"claude-fable-5-1[1m]","resolvedModel":"claude-fable-5-1","displayName":"Fable","description":"Fable 5.1 · Most capable for your hardest and longest-running tasks","supportedEffortLevels":["low","medium","high","xhigh","max"]},
            {"value":"haiku","resolvedModel":"claude-haiku-4-5-20251001","displayName":"Haiku","description":"Haiku 4.5 · Fastest for quick answers"},
            {"displayName":"nameless"}
        ]}}}"#;
        let v: Value = serde_json::from_str(line).unwrap();

        assert!(models_from_response(&v, "someone-else").is_none(), "another request's answer");
        let models = models_from_response(&v, "oculus-models").unwrap().unwrap();
        assert_eq!(models.len(), 3, "a row with neither name is dropped, not fatal");
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
        assert!(models_from_response(&refused, "oculus-models").unwrap().is_err());
    }

    /// The probe against the installed CLI. Ignored because it needs one:
    /// `cargo test --lib list_models_from_the_real_cli -- --ignored`.
    #[test]
    #[ignore]
    fn list_models_from_the_real_cli() {
        let bin = crate::harness::discover::binary(Provider::Claude).expect("claude on PATH");
        let cwd = std::env::temp_dir();
        let started = std::time::Instant::now();
        let models = list_models(&bin, &cwd, &crate::harness::discover::child_env()).unwrap();
        eprintln!("{} models in {:?}: {models:#?}", models.len(), started.elapsed());
        assert!(!models.is_empty());
    }
}
