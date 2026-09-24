//! The Antigravity bridge: one long-lived `agy -p` process per thread.
//!
//! Shape-for-shape this is the Claude bridge. `agy` takes
//! `--input-format stream-json` on stdin and answers `--output-format
//! stream-json` on stdout, one NDJSON object per line, and the process stays
//! up between turns; a thread whose process has gone is resumed with
//! `--conversation <id>`, which is Claude's `--resume` under another name.
//! What differs is the vocabulary, and only the vocabulary:
//!
//! | Claude | Antigravity |
//! | --- | --- |
//! | `{"type": "system", "subtype": "init"}` | `{"event": "init"}` |
//! | stream deltas + `assistant` blocks | `{"event": "step_update"}` |
//! | `{"type": "result"}` | `{"event": "result"}` |
//! | `--resume` | `--conversation` |
//!
//! A `step_update` is the whole middle of the protocol: it carries the
//! incremental `text_delta` for agent text, the tool call and its output, and
//! a per-step `usage`. `state` moves `ACTIVE` → `DONE` and `step_type` says
//! which kind of step it is, so one arm handles what Claude spreads over a
//! stream event, an `assistant` line and a `user` line.
//!
//! ## What this bridge does *not* have, and why
//!
//! **No per-turn reasoning level.** The level is part of the model slug here
//! (`gemini-3.8-flash-high`), which is a process flag as Claude's `--effort`
//! is, so a level chosen mid-thread applies from the next resume; the manager
//! already respawns on a level change for exactly that reason. The picker
//! shows the slug's base as the model and its suffixes as levels, and
//! `model_slug` puts the two back together — see `parse_models`.
//!
//! **No inline settings document.** Claude gets its whole containment through
//! `--settings <json>` on the command line. `agy` has no such flag, and in
//! 1.2.9 reads rules from one place only — the student's global
//! `~/.gemini/antigravity-cli/settings.json`; a workspace `.agents/hooks.json`,
//! a project file, environment variables and a `HOME` override were each
//! tried and none loads. So Oculus keeps a block of its own in that file,
//! rewritten before every spawn and never touching the student's entries
//! ([`super::antigravity_rules`]).
//!
//! **No rewind.** Nothing in the published protocol takes a conversation back
//! to an earlier message: `--conversation` resumes, there is no control
//! channel to ask anything else of, and the CLI's own `/rewind` is refused in
//! print mode ("/rewind is not available in print mode", measured on 1.2.9).
//! [`AntigravitySession::rewind`] therefore refuses rather than pretending,
//! and the manager surfaces that refusal — a rewind that quietly did nothing
//! would leave the thread and the agent out of step in the one place a
//! student is guaranteed to notice.
//!
//! **No interrupt over the protocol either**, for the same reason. Stopping a
//! turn is a signal to the child, and the turn is closed here rather than by
//! anything the CLI says.
//!
//! ## Containment: rules, and a sandbox
//!
//! Measured on 1.2.9, and each fact is why the flags are what they are:
//!
//! - **`--sandbox` is a terminal sandbox only.** It bounds what a *shell*
//!   command may write, not what the file tools may. With
//!   `--dangerously-skip-permissions` beside it — this bridge's first shape —
//!   `write_to_file` wrote a file outside the library while a shell write to
//!   the same place was refused `operation not permitted`. That flag is gone.
//! - **Without it, print mode refuses whatever its rules do not allow**,
//!   rather than hanging on a prompt nothing can answer. With no rules at all,
//!   `view_file` in the library works, `write_to_file` in the workspace
//!   (`agents/`, the cwd) works under `--mode accept-edits`, and every
//!   `run_command` — `ls` included — is refused. The rules
//!   (`antigravity_rules::rules_for`) are Claude's allow and deny lists in
//!   `agy`'s syntax: the database's three files writable, the `oculus` binary
//!   readable and runnable by both spellings, a handful of read-only commands,
//!   the app's own folders and `sqlite3` denied, and whatever the student
//!   approved. A `read_file` / `write_file` grant widens the terminal
//!   sandbox's allowlists too, which is what lets a bare `oculus` run at all:
//!   the sandbox otherwise cannot *read* the binary behind the
//!   `~/.local/bin` symlink, and says `operation not permitted: oculus`.
//! - **A refusal no rule answered ends the turn.** The step arrives as a tool
//!   `step_update` with `state: "ERROR"` and a `tool_info.error.message`
//!   starting `permission check failed`, and the turn closes straight after
//!   with a `result` of `SUCCESS`, an empty `response` and a `denied_actions`
//!   list. The row is closed failed and a [`HarnessEvent::PermissionNeeded`]
//!   carries the rule that would have let it; the turn is `completed`,
//!   because it is. A *deny* rule's refusal shares the prefix but not the
//!   rest: its message ends `Matches user-configured deny rule.`, the agent is
//!   told and carries on in the same turn, and nothing is offered, since no
//!   allow beats a deny.
//! - **A live `agy` never re-reads its rules**, so approving one
//!   (`harness_antigravity_allow`) stores it and drops the thread's process;
//!   the next message resumes the conversation with the rules rewritten.
//!
//! Codex gets a seatbelt with an explicit writable-file list, opencode a
//! rendered `opencode.json`, Claude the settings document; this one gets the
//! same lists by way of a file it does not own, which is the price of the one
//! door `agy` has.
//!
//! One smaller consequence of the shape: a command that exits non-zero is
//! still a *successful tool call* — `tool_info.error` is for the tool failing
//! (or being refused), not for the command's exit status — so such a row is
//! closed `ok` with the failure in its output, which is what the timeline
//! shows.
//!
//! ## Two things the published reference gets wrong
//!
//! Both cost a turn and neither is visible from the docs, so they are written
//! down here rather than rediscovered.
//!
//! **`-p` takes the prompt as its value.** It is `--print <prompt>`, not
//! Claude's bare flag, so `-p --input-format stream-json` hands the CLI
//! `"--input-format"` as the prompt and leaves the rest as stray arguments.
//! `agy` says so and exits 2. In stream-json mode the prompt comes from
//! stdin, so the flag is `--print=` with an **empty attached value** — the
//! `=` is load-bearing, because a separate empty argument is a positional one.
//!
//! **The parameters are PascalCase.** `run_command` takes `CommandLine` and
//! `view_file` takes `AbsolutePath`, where all three other CLIs use
//! `command` / `file_path` / `path`. Read with a lowercase key every row is
//! titled with an empty string, which reads as a missing title rather than as
//! a wrong lookup — the same trap opencode's `path`-vs-`file_path` arms are
//! already in `event.rs` for.
//!
//! Both are measured off `agy` 1.2.9, as is every event shape here:
//! `fixtures/harness/antigravity-ls.ndjson` is a real session and
//! [`tests::folds_a_recorded_session`] replays it. What is still *inferred* is
//! the parameter key of the tools that recording did not exercise — the lists
//! in `event.rs` say which.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::event::{cap_output, classify, HarnessEvent, Provider};
use super::{RawLog, Sink};

pub struct AntigravitySpawn {
    pub bin: PathBuf,
    /// The library's `agents/` folder — the session's workspace, the sandbox's
    /// writable root, and the directory `AGENTS.md` is read from.
    pub cwd: PathBuf,
    /// The library root, opened for reads with `--add-dir` — the same flag
    /// and the same job as Claude's. The workspace is `cwd` (`agents/`), so
    /// without this the course folders beside it are outside every tool's
    /// reach.
    pub library: PathBuf,
    /// Resume this conversation rather than starting one.
    pub resume: Option<String>,
    pub model: Option<String>,
    /// The level picked beside `model`. Antigravity has no working level
    /// flag of its own for these: the level *is* the slug's suffix, so it is
    /// folded back into `--model` by `model_slug` rather than sent alongside.
    pub effort: Option<String>,
    /// The per-thread half of the brief. There is no `--append-system-prompt`,
    /// and the library-wide half is already on disk as `agents/AGENTS.md`,
    /// which `agy` reads by itself — so this rides the first user message, the
    /// way opencode's `brief` does.
    pub brief: String,
    pub env: Vec<(String, String)>,
    pub raw_log: Option<RawLog>,
    /// The student's approved rules, read from the database by a caller that
    /// has it; `None` reuses the ones the last spawn wrote. See
    /// [`super::antigravity_rules::install`].
    pub approved: Option<Vec<String>>,
}

pub struct AntigravitySession {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    alive: Arc<AtomicBool>,
    /// Prepended to the first message and then gone, like opencode's.
    pending_brief: Mutex<Option<String>>,
    /// Set between asking the child to stop and the turn closing, so the
    /// translator can call the difference between a cancelled turn and a
    /// failed one.
    interrupting: Arc<AtomicBool>,
    /// A turn is owed a `result`. A process that dies inside that window has
    /// to close the turn anyway, or the manager never releases the thread for
    /// its next message.
    expecting: Arc<AtomicBool>,
}

impl AntigravitySession {
    pub fn spawn(cfg: AntigravitySpawn, sink: Sink) -> Result<Arc<Self>, String> {
        // The rules first, and a failure to write them is a failure to spawn:
        // `agy` reads them once, at start, and a process started without them
        // would run with whatever the file held before — or nothing.
        super::antigravity_rules::install(&cfg.library, cfg.approved.clone())
            .map_err(|e| format!("Antigravity was not started: {e}"))?;
        let mut cmd = Command::new(&cfg.bin);
        // `--print=` with an **empty attached value**, and the `=` is the whole
        // point. `-p` here is not Claude's bare flag: it is
        // `--print <prompt>`, so `-p --input-format …` hands the CLI
        // "--input-format" as the prompt and leaves the rest as stray
        // arguments — which it says out loud and exits 2 over. In stream-json
        // mode the prompt comes from stdin, so the value is empty and has to
        // be attached rather than positional.
        cmd.arg("--print=")
            .args(["--input-format", "stream-json"])
            .args(["--output-format", "stream-json"])
            // A chat message is text, not a command line. Without this a
            // student who opens a message with `/` has it expanded as a slash
            // command or a skill, which is never what they meant in a bubble.
            .arg("--disable-slash-commands")
            // The terminal sandbox. It bounds shell commands only — the file
            // tools answer to the rules `antigravity_rules` just wrote — and
            // there is deliberately no `--dangerously-skip-permissions` beside
            // it: measured, with that flag `write_to_file` wrote outside the
            // library. Without it print mode refuses what the rules do not
            // allow, and the refusal ends the turn. See the module docs.
            .arg("--sandbox")
            // Claude's `acceptEdits` by another name: edits inside the
            // workspace land without an approval round-trip, which is the only
            // workable setting when nothing can approve.
            .args(["--mode", "accept-edits"])
            // The library, opened for reads. The workspace is `agents/`
            // because that is the cwd, and without this the courses beside it
            // are outside every tool's reach — the same job Claude's
            // `--add-dir` does, spelled the same way.
            .arg("--add-dir")
            .arg(&cfg.library);
        if let Some(m) = &cfg.model {
            cmd.args(["--model", &model_slug(m, cfg.effort.as_deref())]);
        }
        if let Some(id) = &cfg.resume {
            cmd.args(["--conversation", id]);
        }
        cmd.current_dir(&cfg.cwd)
            .env_clear()
            .envs(cfg.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", cfg.bin.display()))?;
        let stdin = child.stdin.take().ok_or("no stdin on agy child")?;
        let stdout = child.stdout.take().ok_or("no stdout on agy child")?;
        let stderr = child.stderr.take().ok_or("no stderr on agy child")?;

        let alive = Arc::new(AtomicBool::new(true));
        let interrupting = Arc::new(AtomicBool::new(false));
        let expecting = Arc::new(AtomicBool::new(false));
        let session = Arc::new(AntigravitySession {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            alive: alive.clone(),
            pending_brief: Mutex::new(
                (!cfg.brief.trim().is_empty()).then(|| cfg.brief.clone()),
            ),
            interrupting: interrupting.clone(),
            expecting: expecting.clone(),
        });

        // The CLI's own log. Kept as a tail so a process that dies before
        // saying anything on stdout can still explain itself — which for this
        // agent is the likely shape of a run on a signed-out machine.
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
                for ev in state.translate(&v) {
                    sink(ev);
                }
            }
            alive.store(false, Ordering::SeqCst);
            let code = reader_session
                .child
                .lock()
                .unwrap()
                .wait()
                .ok()
                .and_then(|s| s.code());
            if expecting.swap(false, Ordering::SeqCst) || state.turn_open {
                let tail = stderr_tail.lock().unwrap().join("\n");
                let msg = if tail.trim().is_empty() {
                    format!("agy exited (code {code:?}) mid-turn")
                } else {
                    format!("agy exited (code {code:?}) mid-turn:\n{tail}")
                };
                sink(HarnessEvent::error_for(Provider::Antigravity, msg));
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
            .map_err(|e| format!("agy stdin: {e}"))
    }

    /// One user turn. The brief, if this is the first one, goes out ahead of
    /// the message in the same envelope — there is no system-prompt flag to
    /// carry it and no second channel to send it down.
    pub fn send(&self, text: &str) -> Result<(), String> {
        let brief = self.pending_brief.lock().unwrap().take();
        let text = match brief {
            Some(b) => format!("{}\n\n---\n\n{text}", b.trim()),
            None => text.to_string(),
        };
        self.expecting.store(true, Ordering::SeqCst);
        self.write_line(&serde_json::json!({
            "event": "user",
            "message": { "content": text },
        }))
    }

    /// Stop the current turn.
    ///
    /// The protocol has no interrupt: there is no control channel, and the
    /// only thing that ends a turn early is the process ending. So this kills
    /// the child and closes the turn itself — the flag tells the reader thread
    /// that the death it is about to see was asked for, so the turn is
    /// reported `interrupted` rather than `failed`. The thread's next message
    /// resumes the conversation by id, which is what makes this survivable:
    /// nothing is lost but the half-written answer.
    pub fn interrupt(&self) -> Result<(), String> {
        if !self.is_alive() {
            return Ok(());
        }
        self.interrupting.store(true, Ordering::SeqCst);
        self.kill();
        Ok(())
    }

    /// Antigravity cannot rewind, and says so rather than no-opping.
    ///
    /// `--conversation` resumes a conversation whole; nothing in the protocol
    /// drops a message and everything after it, and the CLI's own `/rewind`
    /// answers "not available in print mode" (measured, 1.2.9) — so there is
    /// no headless way to it at all. The manager deletes rows on
    /// the strength of this call, so answering `Ok(())` here would leave the
    /// timeline shorter than the agent's context with nothing to show for it.
    pub fn rewind(&self, _anchor: &str) -> Result<(), String> {
        Err("Antigravity cannot take a question back out of a conversation — \
             edit it in a new thread instead"
            .into())
    }

    pub fn kill(&self) {
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
        self.alive.store(false, Ordering::SeqCst);
    }
}

impl Drop for AntigravitySession {
    fn drop(&mut self) {
        if let Ok(mut c) = self.child.lock() {
            let _ = c.kill();
        }
    }
}

// ── Translation ──────────────────────────────────────────────────────────────

/// Per-process translation state.
///
/// Smaller than Claude's, because `step_update` already carries the structure
/// Claude's stream has to be reassembled into: a step is identified by its
/// `step_index`, so a delta and the block it belongs to arrive under the same
/// number and nothing has to be matched up after the fact.
#[derive(Default)]
struct Translator {
    /// Between the first event of a turn and its `result`.
    turn_open: bool,
    /// Text accumulated for the step currently `ACTIVE`, flushed as one
    /// `AssistantMessage` when it goes `DONE`. The deltas are display only;
    /// this is what gets persisted.
    step_text: String,
    /// Which step `step_text` belongs to. A step index that changes without a
    /// `DONE` in between still flushes, so a dropped terminator cannot merge
    /// two answers into one row.
    step_index: Option<i64>,
    /// Tool steps that have had their `ToolStarted` emitted, by step index —
    /// `tool_info` is repeated on every update of the step, and the row must
    /// open once.
    started_tools: std::collections::HashSet<i64>,
    /// A refused step already said what it needed this turn, so the
    /// `result`'s `denied_actions` has nothing to add.
    refused: bool,
    interrupting: Arc<AtomicBool>,
    expecting: Arc<AtomicBool>,
}

impl Translator {
    fn open_turn(&mut self, out: &mut Vec<HarnessEvent>) {
        if !self.turn_open {
            self.turn_open = true;
            out.push(HarnessEvent::TurnStarted);
        }
    }

    /// Close the open assistant step, if there is one with anything in it.
    fn flush_text(&mut self, out: &mut Vec<HarnessEvent>) {
        let text = std::mem::take(&mut self.step_text);
        if !text.trim().is_empty() {
            out.push(HarnessEvent::AssistantMessage { text });
        }
        self.step_index = None;
    }

    fn translate(&mut self, v: &Value) -> Vec<HarnessEvent> {
        let mut out = Vec::new();
        match v.get("event").and_then(|e| e.as_str()).unwrap_or("") {
            "init" => {
                let init = v.get("init").cloned().unwrap_or(Value::Null);
                out.push(HarnessEvent::SessionStarted {
                    provider_session_id: v
                        .get("conversation_id")
                        .and_then(|s| s.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    model: init.get("model").and_then(|s| s.as_str()).map(String::from),
                    cwd: init
                        .get("cwd")
                        .and_then(|s| s.as_str())
                        .unwrap_or_default()
                        .to_string(),
                });
            }
            "step_update" => {
                let su = v.get("step_update").cloned().unwrap_or(Value::Null);
                self.open_turn(&mut out);
                self.step(&su, &mut out);
            }
            "result" => {
                let r = v.get("result").cloned().unwrap_or(Value::Null);
                self.flush_text(&mut out);
                if let Some(u) = r.get("usage") {
                    out.push(usage_event(u));
                }
                // A turn a refusal ended is a `SUCCESS` with an empty
                // `response` and the refusals listed — measured, and closed as
                // `completed` below, because nothing failed: the agent asked
                // for something it was not given. The step itself normally
                // said what it needed; this is the fallback for a refusal
                // that never showed up as a step.
                if !std::mem::take(&mut self.refused) {
                    for d in r
                        .get("denied_actions")
                        .and_then(|a| a.as_array())
                        .into_iter()
                        .flatten()
                    {
                        let field = |k: &str| d.get(k).and_then(|s| s.as_str()).unwrap_or("").to_string();
                        out.push(HarnessEvent::PermissionNeeded {
                            tool: field("display_name"),
                            action: field("action"),
                            target: None,
                            rule: None,
                        });
                    }
                }
                // `status` is an enum of seven, not the three the timeline
                // knows. INTERRUPTED and CANCELED are the same thing to a
                // reader; WAITING and RUNNING should never close a turn, and
                // if one does it is a failure rather than a success, because
                // the turn is over either way and nothing more is coming.
                let status = r.get("status").and_then(|s| s.as_str()).unwrap_or("");
                let interrupted = self.interrupting.swap(false, Ordering::SeqCst);
                let mapped = match status {
                    _ if interrupted => "interrupted",
                    "SUCCESS" => "completed",
                    "INTERRUPTED" | "CANCELED" => "interrupted",
                    _ => "failed",
                };
                if mapped == "failed" {
                    let why = r
                        .get("error")
                        .and_then(|s| s.as_str())
                        .filter(|s| !s.trim().is_empty())
                        .map(String::from)
                        .unwrap_or_else(|| format!("Antigravity ended the turn with {status}"));
                    out.push(HarnessEvent::error_for(Provider::Antigravity, why));
                }
                self.turn_open = false;
                self.expecting.store(false, Ordering::SeqCst);
                out.push(HarnessEvent::TurnFinished {
                    status: mapped.into(),
                });
            }
            _ => {}
        }
        out
    }

    /// One `step_update`. Four `step_type`s, and only two of them say
    /// anything the timeline has a row for: `agent_response` is the answer,
    /// `tool` is a call. `user_input` is the message this app already echoed
    /// itself, and `checkpoint` is the CLI's own bookkeeping.
    fn step(&mut self, su: &Value, out: &mut Vec<HarnessEvent>) {
        let index = su.get("step_index").and_then(|i| i.as_i64()).unwrap_or(0);
        let state = su.get("state").and_then(|s| s.as_str()).unwrap_or("");
        let kind = su.get("step_type").and_then(|s| s.as_str()).unwrap_or("");

        // A new step means the previous one is over, whatever it claimed.
        if self.step_index.is_some_and(|i| i != index) {
            self.flush_text(out);
        }

        match kind {
            "agent_response" => {
                self.step_index = Some(index);
                if let Some(d) = su.get("text_delta").and_then(|s| s.as_str()) {
                    if !d.is_empty() {
                        self.step_text.push_str(d);
                        out.push(HarnessEvent::AssistantDelta { text: d.into() });
                    }
                }
                if state == "DONE" {
                    self.flush_text(out);
                }
            }
            "tool" => {
                let info = su.get("tool_info").cloned().unwrap_or(Value::Null);
                let name = info
                    .get("name")
                    .and_then(|s| s.as_str())
                    .or_else(|| su.get("tool_name").and_then(|s| s.as_str()))
                    .unwrap_or("")
                    .to_string();
                let input = info
                    .get("parameters")
                    .cloned()
                    .unwrap_or(Value::Object(Default::default()));
                // The step index is the id: `tool_info` has no call id of its
                // own, and a step is exactly one call.
                let id = format!("step-{index}");
                if self.started_tools.insert(index) {
                    let (tool_kind, title) = classify(&name, &input);
                    out.push(HarnessEvent::ToolStarted {
                        id: id.clone(),
                        kind: tool_kind,
                        name: name.clone(),
                        title,
                        input,
                    });
                }
                // Refused: the row closes failed with the CLI's own sentence.
                // Two refusals share the `permission check failed` prefix and
                // only one is a question. No rule allowed it ("user denied
                // permission to run command: …") is print mode's automatic
                // no, the turn ends right after, and the event says what to
                // allow. A deny rule matched ("Matches user-configured deny
                // rule", or for a command the `for unsandboxed "…"` wording)
                // is Oculus's own answer or the student's: measured, the agent
                // is told and carries on in the same turn, and there is
                // nothing to approve — an allow never beats a deny.
                if state == "ERROR" {
                    let message = info
                        .pointer("/error/message")
                        .and_then(|s| s.as_str())
                        .unwrap_or("tool failed")
                        .to_string();
                    let params = info.get("parameters").cloned().unwrap_or(Value::Null);
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        ok: false,
                        output: cap_output(&message),
                        title: None,
                    });
                    if is_question(&message) {
                        self.refused = true;
                        let (action, target, rule) = refusal(&name, &params, &message);
                        out.push(HarnessEvent::PermissionNeeded {
                            tool: name,
                            action,
                            target,
                            rule,
                        });
                    }
                } else if state == "DONE" {
                    let err = info.get("error").filter(|e| !e.is_null());
                    let output = match err {
                        Some(e) => e
                            .get("message")
                            .and_then(|s| s.as_str())
                            .unwrap_or("tool failed")
                            .to_string(),
                        None => info
                            .get("output")
                            .and_then(|s| s.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    };
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        ok: err.is_none(),
                        output: cap_output(&output),
                        title: None,
                    });
                }
            }
            _ => {}
        }

        // Per-step usage is cumulative for the turn in the `result`, so this
        // is the live figure and the `result`'s is the final one. Both are
        // emitted: the timeline shows the last it was told.
        if state == "DONE" {
            if let Some(u) = su.get("usage") {
                out.push(usage_event(u));
            }
        }
    }
}

/// Whether a refused step is one the student can answer — print mode's
/// automatic no, which ends the turn — rather than a deny rule, which does not
/// and cannot be approved past. Both begin `permission check failed`. A deny
/// says `Matches user-configured deny rule`, and a command deny was measured
/// arriving as `permission check failed for unsandboxed "sqlite3 …"`; that
/// wording counts as a deny unless it also carries the automatic no's own
/// `user denied permission`.
fn is_question(message: &str) -> bool {
    let unsandboxed = message.starts_with("permission check failed for unsandboxed");
    message.starts_with("permission check failed")
        && !message.contains("deny rule")
        && (!unsandboxed || message.contains("user denied permission"))
}

/// What a refused step needed: the permission (`agy`'s own word for it), the
/// thing it was refused on, and a rule that would allow it.
///
/// The message is `permission check failed for <action> "<target>": …`
/// (measured for `command`; the file variant reads the same way), so the
/// action and target come off it first and off the tool's parameters when it
/// does not parse. The suggestion is deliberately narrow: a command's first
/// word, a file's folder — the student is approving *this* kind of thing, not
/// the whole machine.
fn refusal(tool: &str, params: &Value, message: &str) -> (String, Option<String>, Option<String>) {
    let param = |ks: &[&str]| {
        ks.iter()
            .find_map(|k| params.get(*k).and_then(|v| v.as_str()))
            .filter(|s| !s.trim().is_empty())
            .map(String::from)
    };
    // `for command "python3 -c …":` → ("command", "python3 -c …").
    let said = message
        .strip_prefix("permission check failed for ")
        .and_then(|rest| {
            let (action, rest) = rest.split_once(' ')?;
            let quoted = rest.strip_prefix('"')?;
            let end = quoted.find("\":").or_else(|| quoted.rfind('"'))?;
            Some((action.to_string(), quoted[..end].to_string()))
        });
    let by_tool = match tool {
        "run_command" => "command",
        "write_to_file" | "replace_file_content" | "multi_replace_file_content" | "sed_file"
        | "notebook_edit" => "write_file",
        "view_file" | "read_resource" | "list_dir" | "find_by_name" | "grep_search" => "read_file",
        "read_url_content" | "open_browser_url" => "read_url",
        _ => "",
    };
    let action = match &said {
        Some((a, _)) if !a.is_empty() => a.clone(),
        _ => by_tool.to_string(),
    };
    let target = match action.as_str() {
        "command" => param(&["CommandLine", "Command"]),
        "read_url" => param(&["Url", "URL"]),
        _ => param(&["TargetFile", "AbsolutePath", "DirectoryPath", "SearchDirectory", "SearchPath", "Path"]),
    }
    .or_else(|| said.map(|(_, t)| t).filter(|t| !t.is_empty()));
    let rule = target.as_deref().and_then(|t| match action.as_str() {
        "command" => command_word(t).map(|w| format!("command({w})")),
        "write_file" | "read_file" => {
            // A folder is granted as itself; a file by the folder it is in.
            let p = std::path::Path::new(t);
            let dir = if tool == "list_dir" || p.is_dir() { Some(p) } else { p.parent() };
            dir.filter(|d| d.is_absolute() && d.parent().is_some())
                .map(|d| format!("{action}({})", d.display()))
        }
        "read_url" => url::Url::parse(t)
            .ok()
            .and_then(|u| u.host_str().map(|h| format!("read_url({h})"))),
        _ => None,
    });
    (action, target, rule)
}

/// The command a command line runs: its first word once leading `FOO=1`
/// assignments are stripped, unquoted. An absolute path stays one, since
/// that is what the rule has to match.
fn command_word(line: &str) -> Option<String> {
    line.split_whitespace()
        .map(|w| w.trim_matches(|c| c == '"' || c == '\''))
        .find(|w| {
            let assignment = w.split_once('=').is_some_and(|(k, _)| {
                !k.is_empty()
                    && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && !k.starts_with(|c: char| c.is_ascii_digit())
            });
            !w.is_empty() && !assignment
        })
        .map(String::from)
}

/// Antigravity's `usage` object → the timeline's. It reports no cost and no
/// context window, so both stay `None` rather than being invented; `context`
/// is the total the last step occupied, which is the same thing Claude's
/// per-request figure means.
fn usage_event(u: &Value) -> HarnessEvent {
    let n = |k: &str| u.get(k).and_then(|v| v.as_u64());
    HarnessEvent::Usage {
        input_tokens: n("input_tokens").unwrap_or(0),
        output_tokens: n("output_tokens").unwrap_or(0),
        context_tokens: n("total_tokens"),
        context_window: None,
        cost_usd: None,
    }
}

// ── The catalogue ────────────────────────────────────────────────────────────

/// One model `agy models` printed.
///
/// The same shape Codex's `ModelInfo` has, so the frontend adapts both with
/// one function and the picker stays provider-blind.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub reasoning_efforts: Vec<String>,
    pub default_reasoning_effort: Option<String>,
}

/// Ask `agy` what this account can actually use.
///
/// `agy models` is a plain listing subcommand — no session, no turn, nothing
/// billed — which is what makes it safe to call from a picker at all. (The
/// rule this repo learned the hard way: nothing in Settings may spend money.
/// A listing that costs a request is exactly what opencode's deleted probe
/// was.) It opens nothing either: signed out, it prints `Please sign in to
/// view available models` and exits non-zero in about a second (measured,
/// 1.2.9), which is why `signin::status` asks it too.
///
/// It has no `--json` flag, so the output is parsed as lines. Anything that
/// does not look like a slug is skipped rather than guessed at, and an empty
/// list is returned as an empty list — the picker says "no models" and the
/// student can run `agy models` themselves to see the same nothing.
pub fn list_models(bin: &std::path::Path, env: &[(String, String)]) -> Result<Vec<ModelInfo>, String> {
    let out = run_models(bin, env)?;
    if !out.success {
        let said = [out.stderr.trim(), out.stdout.trim()]
            .into_iter()
            .find(|s| !s.is_empty())
            .map(String::from);
        return Err(said.unwrap_or_else(|| "`agy models` failed and said nothing".into()));
    }
    Ok(parse_models(&out.stdout))
}

/// What one `agy models` run said.
pub struct ModelsRun {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Well past any answer measured — a signed-in listing takes about 3.5 s, a
/// signed-out refusal about 1 s — and short enough that a wedged `agy` cannot
/// hold a model picker open. The same bound `claude::list_models` has.
const MODELS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// `agy models`, with a deadline: past it the child is killed and the answer
/// is an error that says so.
pub fn run_models(bin: &std::path::Path, env: &[(String, String)]) -> Result<ModelsRun, String> {
    use std::io::Read;
    let mut child = Command::new(bin)
        .arg("models")
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run {}: {e}", bin.display()))?;
    // Each pipe drained on a thread of its own: a child that fills the unread
    // one blocks, and would then look like exactly the hang this bounds.
    fn drain(r: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<String> {
        std::thread::spawn(move || {
            let mut s = String::new();
            if let Some(mut r) = r {
                let _ = r.read_to_string(&mut s);
            }
            s
        })
    }
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let deadline = std::time::Instant::now() + MODELS_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "`agy models` did not answer within {}s",
                    MODELS_TIMEOUT.as_secs()
                ));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(format!("`agy models`: {e}"));
            }
        }
    };
    Ok(ModelsRun {
        success: status.success(),
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

/// Models out of `agy models`' output.
///
/// The listing is a column of ids, possibly with a marker or a description
/// beside them, so the first whitespace-separated word of each line is the
/// candidate and everything else on the line is ignored. A line whose first
/// word is not slug-shaped — a heading, a blank, a box-drawing rule — is not a
/// model.
///
/// **Effort is baked into most slugs** (`gemini-3.8-flash-high`), so the
/// listing is one row per level: eleven Gemini rows that are three models. The
/// suffix is split off here and slugs sharing a base become one model whose
/// `reasoning_efforts` are those suffixes — the same shape Codex and Claude
/// report, so the picker gets a level row rather than a special case. The
/// model's id is the base, and `model_slug` rebuilds the real slug at spawn.
/// A slug with no level suffix (`claude-sonnet-4-6`) is a model with no
/// levels, and is passed through untouched.
fn parse_models(stdout: &str) -> Vec<ModelInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut models: Vec<ModelInfo> = Vec::new();
    for line in stdout.lines() {
        let Some(word) = line.split_whitespace().next() else {
            continue;
        };
        let word = word.trim_matches(|c: char| !c.is_alphanumeric());
        if !is_slug(word) || !seen.insert(word.to_string()) {
            continue;
        }
        let (base, level) = split_level(word);
        match models.iter_mut().find(|m| m.id == base) {
            Some(m) => {
                if let Some(l) = level {
                    m.reasoning_efforts.push(l.to_string());
                }
            }
            None => models.push(ModelInfo {
                id: base.to_string(),
                display_name: listed_name(line, level).unwrap_or_else(|| display_name(base)),
                reasoning_efforts: level.map(|l| vec![l.to_string()]).unwrap_or_default(),
                default_reasoning_effort: None,
            }),
        }
    }
    // Medium where the model has it, as the middle of the road; otherwise the
    // first the listing gave, which is the order `agy` itself leads with.
    for m in &mut models {
        m.default_reasoning_effort = m
            .reasoning_efforts
            .iter()
            .find(|l| *l == "medium")
            .or(m.reasoning_efforts.first())
            .cloned();
    }
    models
}

/// The name `agy models` prints beside a slug, when it prints one.
///
/// Measured on 1.2.9: after a `Fetching available models...` line, each row
/// is `<slug>\t<Display Name>` — `gemini-3.8-flash-high\tGemini 3.8 Flash
/// (High)`, `claude-sonnet-4-6\tClaude Sonnet 4.6 (Thinking)`. The vendor's
/// own name beats one rebuilt from the slug; for a row folded into its base,
/// the level's parenthetical is dropped, since the level is now its own row in
/// the picker. `None` for a line with no tab, which falls back to
/// [`display_name`].
fn listed_name(line: &str, level: Option<&str>) -> Option<String> {
    let name = line.split_once('\t')?.1.trim();
    let name = match level {
        Some(l) => match name.rsplit_once(" (") {
            Some((head, tail)) if tail.trim_end_matches(')').eq_ignore_ascii_case(l) => head.trim(),
            _ => name,
        },
        None => name,
    };
    (!name.is_empty()).then(|| name.to_string())
}

/// The level suffixes a slug can end in: the names the manager's
/// `validate_effort` accepts, so a level split off here is one it will pass.
const LEVELS: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

/// `gemini-3.8-flash-high` → (`gemini-3.8-flash`, `high`). A slug that is
/// nothing but a level, or has no level suffix, keeps its whole self.
fn split_level(slug: &str) -> (&str, Option<&str>) {
    match slug.rsplit_once('-') {
        Some((base, l)) if !base.is_empty() && LEVELS.contains(&l) => (base, Some(l)),
        _ => (slug, None),
    }
}

/// The inverse of `split_level`: the slug `agy --model` is given for a model
/// and the level picked beside it. A slug that already ends in a level — a
/// thread started before the listing was grouped, which stored the whole slug
/// — is passed as it is rather than doubled.
fn model_slug(model: &str, effort: Option<&str>) -> String {
    match effort {
        Some(e) if split_level(model).1.is_none() => format!("{model}-{e}"),
        _ => model.to_string(),
    }
}

/// A slug as a person reads it: `claude-opus-4-6-thinking` → "Claude Opus 4.6
/// Thinking", `gpt-oss-120b` → "GPT-OSS 120B".
///
/// Runs of bare numbers are one version (`4-6` is 4.6), a size suffix is
/// upper-cased, and `gpt` keeps the hyphen its vendor writes it with. The
/// brand stays: unlike Claude's or Codex's own lists, this one mixes vendors
/// under one mark, so the name is the only thing saying whose model it is.
fn display_name(slug: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut prev_number = false;
    for part in slug.split('-').filter(|p| !p.is_empty()) {
        let number = part.chars().all(|c| c.is_ascii_digit());
        if number && prev_number {
            if let Some(last) = words.last_mut() {
                last.push('.');
                last.push_str(part);
            }
            continue;
        }
        prev_number = number;
        let word = match part {
            "gpt" | "oss" => part.to_uppercase(),
            p if p.starts_with(|c: char| c.is_ascii_digit()) => p.to_uppercase(),
            p => {
                let mut c = p.chars();
                c.next()
                    .map(|f| f.to_uppercase().chain(c).collect())
                    .unwrap_or_default()
            }
        };
        match words.last_mut() {
            Some(last) if last == "GPT" => {
                last.push('-');
                last.push_str(&word);
            }
            _ => words.push(word),
        }
    }
    words.join(" ")
}

/// Slug-shaped: lowercase alphanumerics and dashes, containing at least one
/// dash and one digit-or-letter run, and long enough not to be a table rule.
/// Deliberately strict — a false positive is a row in a picker that cannot be
/// selected, which is worse than a missing one the student can report.
fn is_slug(w: &str) -> bool {
    w.len() >= 3
        && w.contains('-')
        && w.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
        && w.chars().any(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::event::ToolKind;

    /// A real `agy` 1.2.9 session, recorded off the wire with the same flags
    /// the bridge spawns — the thing that turns every field name in this
    /// module from documented into measured.
    #[test]
    fn folds_a_recorded_session() {
        let raw = include_str!("../../fixtures/harness/antigravity-ls.ndjson");
        let mut t = Translator::default();
        let events: Vec<HarnessEvent> = raw
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .flat_map(|v| t.translate(&v))
            .collect();

        let session = events.iter().find_map(|e| match e {
            HarnessEvent::SessionStarted { provider_session_id, cwd, .. } => {
                Some((provider_session_id.clone(), cwd.clone()))
            }
            _ => None,
        });
        let (id, cwd) = session.expect("conversation_id and cwd off the init event");
        assert!(!id.is_empty());
        assert!(cwd.ends_with("agytest"));

        // The one place the PascalCase parameter bites: a lowercase lookup
        // titles this row with an empty string instead of the command.
        let tools: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::ToolStarted { kind, title, name, .. } => {
                    Some((*kind, title.clone(), name.clone()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            tools,
            vec![(ToolKind::Bash, "ls -a".to_string(), "run_command".to_string())]
        );

        // Opened once, though `tool_info` is repeated on every update of the
        // step, and closed once with the command's output.
        let finished: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::ToolFinished { ok, output, .. } => Some((*ok, output.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(finished.len(), 1);
        assert!(finished[0].0, "the command succeeded");
        // `ls -a` in the sandbox's own empty working directory. The point is
        // that stdout rode `tool_info.output` at all, not what it said.
        assert!(finished[0].1.contains(".."), "stdout rode `output`");

        // Text arrives as deltas and is persisted once per step.
        let deltas: String = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::AssistantDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(!deltas.trim().is_empty(), "the agent said something");
        let messages = events
            .iter()
            .filter(|e| matches!(e, HarnessEvent::AssistantMessage { .. }))
            .count();
        assert!(messages >= 1);

        // One turn, opened once and closed once.
        assert_eq!(
            events.iter().filter(|e| matches!(e, HarnessEvent::TurnStarted)).count(),
            1
        );
        assert!(matches!(
            events.last(),
            Some(HarnessEvent::TurnFinished { status }) if status == "completed"
        ));
        // SUCCESS is not an error.
        assert!(!events.iter().any(|e| matches!(e, HarnessEvent::Error { .. })));
    }

    /// The statuses that are not `SUCCESS`. `INTERRUPTED` is a stop the
    /// student asked for and must not paint the timeline red; anything
    /// unrecognised is a failure, because the turn is over either way and a
    /// quiet non-answer is the one thing a parse or a turn may never be.
    #[test]
    fn a_result_status_maps_to_one_of_three() {
        for (status, want, err) in [
            ("SUCCESS", "completed", false),
            ("INTERRUPTED", "interrupted", false),
            ("CANCELED", "interrupted", false),
            ("ERROR", "failed", true),
            ("INVALID", "failed", true),
            ("WAITING", "failed", true),
        ] {
            let mut t = Translator::default();
            let v = serde_json::json!({
                "event": "result",
                "result": { "conversation_id": "x", "status": status },
            });
            let out = t.translate(&v);
            assert!(
                matches!(out.last(), Some(HarnessEvent::TurnFinished { status: s }) if s == want),
                "{status} → {want}"
            );
            assert_eq!(
                out.iter().any(|e| matches!(e, HarnessEvent::Error { .. })),
                err,
                "{status} error row"
            );
        }
    }

    /// An interrupt is a signal here, not a protocol message, so the flag is
    /// what tells a killed turn from a failed one — whatever the CLI managed
    /// to put in `status` on its way out.
    #[test]
    fn an_asked_for_stop_is_not_a_failure() {
        let mut t = Translator::default();
        t.interrupting.store(true, Ordering::SeqCst);
        let out = t.translate(&serde_json::json!({
            "event": "result",
            "result": { "status": "ERROR", "error": "killed" },
        }));
        assert!(
            matches!(out.last(), Some(HarnessEvent::TurnFinished { status }) if status == "interrupted")
        );
        assert!(!out.iter().any(|e| matches!(e, HarnessEvent::Error { .. })));
    }

    #[test]
    fn model_slugs_are_read_off_the_first_column() {
        let out = "\
Models available to your account

  gemini-3.8-flash-high      Fastest, highest effort
  gemini-3.8-flash-medium    Balanced
  gemini-3.8-flash-low
  claude-opus-5              via Antigravity
";
        let ids: Vec<String> = parse_models(out).into_iter().map(|m| m.id).collect();
        assert_eq!(ids, ["gemini-3.8-flash", "claude-opus-5"]);
    }

    /// The listing as 1.2.9 prints it, header and tabs included: the name
    /// beside the slug is the one shown, less the level a folded row has
    /// moved into its own picker row.
    #[test]
    fn a_real_listing_keeps_the_names_it_prints() {
        let out = "Fetching available models...\n\
gemini-3.8-flash-high\tGemini 3.8 Flash (High)\n\
gemini-3.8-flash-low\tGemini 3.8 Flash (Low)\n\
claude-sonnet-4-6\tClaude Sonnet 4.6 (Thinking)\n";
        let got: Vec<(String, String, Vec<String>)> = parse_models(out)
            .into_iter()
            .map(|m| (m.id, m.display_name, m.reasoning_efforts))
            .collect();
        assert_eq!(
            got,
            vec![
                (
                    "gemini-3.8-flash".to_string(),
                    "Gemini 3.8 Flash".to_string(),
                    vec!["high".to_string(), "low".to_string()]
                ),
                ("claude-sonnet-4-6".to_string(), "Claude Sonnet 4.6 (Thinking)".to_string(), vec![]),
            ]
        );
    }

    /// The listing `agy` 1.2.9 actually prints: one row per level. Levels fold
    /// into their base, a slug with no level stands alone, and a lone suffix
    /// is still a level.
    #[test]
    fn level_suffixes_become_reasoning_levels() {
        let out = "\
gemini-3.8-flash-high
gemini-3.8-flash-medium
gemini-3.8-flash-low
gemini-3.1-pro-high
gemini-3.1-pro-low
claude-sonnet-4-6
claude-opus-4-6-thinking
gpt-oss-120b-medium
";
        let got: Vec<(String, String, Vec<String>, Option<String>)> = parse_models(out)
            .into_iter()
            .map(|m| (m.id, m.display_name, m.reasoning_efforts, m.default_reasoning_effort))
            .collect();
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(
            got,
            vec![
                ("gemini-3.8-flash".into(), "Gemini 3.8 Flash".into(), s(&["high", "medium", "low"]), Some("medium".into())),
                ("gemini-3.1-pro".into(), "Gemini 3.1 Pro".into(), s(&["high", "low"]), Some("high".into())),
                ("claude-sonnet-4-6".into(), "Claude Sonnet 4.6".into(), vec![], None),
                ("claude-opus-4-6-thinking".into(), "Claude Opus 4.6 Thinking".into(), vec![], None),
                ("gpt-oss-120b".into(), "GPT-OSS 120B".into(), s(&["medium"]), Some("medium".into())),
            ]
        );
    }

    /// The level goes back on the slug at spawn, and a whole slug stored by a
    /// thread from before the grouping is not given a second one.
    #[test]
    fn the_level_is_folded_back_into_the_slug() {
        assert_eq!(model_slug("gemini-3.8-flash", Some("high")), "gemini-3.8-flash-high");
        assert_eq!(model_slug("claude-sonnet-4-6", None), "claude-sonnet-4-6");
        assert_eq!(model_slug("gemini-3.8-flash-low", Some("high")), "gemini-3.8-flash-low");
        assert_eq!(model_slug("gemini-3.8-flash-low", None), "gemini-3.8-flash-low");
    }

    /// Headings, rules and prose are not models. A picker row that cannot be
    /// selected is worse than one that is missing.
    #[test]
    fn furniture_is_not_a_model() {
        assert!(parse_models("Models\n\n──────────\nNone found.\n").is_empty());
        assert!(!is_slug("models"));
        assert!(!is_slug("──────────"));
        assert!(!is_slug("a-"));
        assert!(is_slug("gemini-3.8-flash"));
    }

    /// The same slug twice — a listing that groups by provider and repeats —
    /// is one row.
    #[test]
    fn a_repeated_slug_is_one_model() {
        let ids: Vec<String> = parse_models("gemini-3.8-flash\ngemini-3.8-flash\n")
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, ["gemini-3.8-flash"]);
    }

    /// A refused step, as 1.2.9 sends it, and the `result` that ends the turn
    /// straight after: the row closes failed, the rule to allow is named, and
    /// the turn is `completed` — nothing broke, the agent was stopped.
    #[test]
    fn a_refusal_closes_the_row_and_names_the_rule() {
        let mut t = Translator::default();
        let step = |state: &str, error: Value| {
            serde_json::json!({
                "event": "step_update",
                "step_update": {
                    "step_index": 4, "state": state, "step_type": "tool",
                    "tool_info": {
                        "name": "run_command",
                        "parameters": { "CommandLine": "python3 -c \"print(6*7)\"" },
                        "error": error,
                    },
                },
            })
        };
        let mut out = t.translate(&step("ACTIVE", Value::Null));
        out.extend(t.translate(&step(
            "ERROR",
            serde_json::json!({ "message": "permission check failed for command \"python3 -c \\\"print(6*7)\\\"\": user denied permission to run command: python3" }),
        )));
        out.extend(t.translate(&serde_json::json!({
            "event": "result",
            "result": {
                "status": "SUCCESS", "response": "",
                "denied_actions": [{ "action": "command", "display_name": "RunCommand" }],
            },
        })));

        assert!(out.iter().any(|e| matches!(
            e,
            HarnessEvent::ToolFinished { id, ok: false, output, .. }
                if id == "step-4" && output.starts_with("permission check failed")
        )));
        let needed: Vec<_> = out
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::PermissionNeeded { tool, action, target, rule } => {
                    Some((tool.clone(), action.clone(), target.clone(), rule.clone()))
                }
                _ => None,
            })
            .collect();
        // Once: the step said it, so `denied_actions` adds nothing.
        assert_eq!(
            needed,
            vec![(
                "run_command".to_string(),
                "command".to_string(),
                Some("python3 -c \"print(6*7)\"".to_string()),
                Some("command(python3)".to_string()),
            )]
        );
        assert!(matches!(out.last(), Some(HarnessEvent::TurnFinished { status }) if status == "completed"));
        assert!(!out.iter().any(|e| matches!(e, HarnessEvent::Error { .. })));
    }

    /// A deny rule's refusal, verbatim off 1.2.9: the row fails, but nothing
    /// is offered — the agent carries on in the same turn, and approving it
    /// could not beat the deny.
    #[test]
    fn a_deny_rule_is_not_a_question() {
        let mut t = Translator::default();
        let out = t.translate(&serde_json::json!({
            "event": "step_update",
            "step_update": {
                "step_index": 2, "state": "ERROR", "step_type": "tool",
                "tool_info": {
                    "name": "write_to_file",
                    "parameters": { "TargetFile": "/lib/agents/skills/x.md" },
                    "error": { "type": "TOOL_ERROR", "message": "permission check failed for write_file \"/lib/agents/skills/x.md\": Permission denied for write_file(/lib/agents/skills/x.md). Matches user-configured deny rule." },
                },
            },
        }));
        assert!(out.iter().any(|e| matches!(e, HarnessEvent::ToolFinished { ok: false, .. })));
        assert!(!out.iter().any(|e| matches!(e, HarnessEvent::PermissionNeeded { .. })));
        // The command deny's wording, measured without `--sandbox`.
        assert!(!is_question(
            "permission check failed for unsandboxed \"sqlite3 /lib/oculus.db 'select 1'\": denied"
        ));
        assert!(is_question(
            "permission check failed for command \"python3\": user denied permission to run command: python3"
        ));
    }

    /// `denied_actions` alone still says the turn was stopped, with no rule
    /// to offer since it names no target.
    #[test]
    fn a_denied_action_with_no_step_is_still_reported() {
        let mut t = Translator::default();
        let out = t.translate(&serde_json::json!({
            "event": "result",
            "result": { "status": "SUCCESS", "response": "",
                        "denied_actions": [{ "action": "write_file", "display_name": "WriteFile" }] },
        }));
        assert!(out.iter().any(|e| matches!(
            e,
            HarnessEvent::PermissionNeeded { action, target: None, rule: None, .. } if action == "write_file"
        )));
    }

    #[test]
    fn a_refused_file_write_suggests_its_folder() {
        let (action, target, rule) = refusal(
            "write_to_file",
            &serde_json::json!({ "TargetFile": "/Users/s/elsewhere/notes.md" }),
            "permission check failed for write_file \"/Users/s/elsewhere/notes.md\": user denied permission",
        );
        assert_eq!(action, "write_file");
        assert_eq!(target.as_deref(), Some("/Users/s/elsewhere/notes.md"));
        assert_eq!(rule.as_deref(), Some("write_file(/Users/s/elsewhere)"));
    }

    #[test]
    fn a_command_rule_is_its_first_real_word() {
        assert_eq!(command_word("python3 -c 'x'").as_deref(), Some("python3"));
        assert_eq!(command_word("FOO=1 BAR_2=x node a.js").as_deref(), Some("node"));
        assert_eq!(command_word("/opt/bin/tool --flag").as_deref(), Some("/opt/bin/tool"));
        assert_eq!(command_word("  ").as_deref(), None);
    }
}
