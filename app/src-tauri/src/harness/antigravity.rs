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
//! **No per-turn reasoning level.** `--effort` is a process flag here as it is
//! for Claude, so a level chosen mid-thread applies from the next resume; the
//! manager already respawns on a level change for exactly that reason.
//!
//! **No inline settings document.** Claude gets its whole containment through
//! `--settings <json>` on the command line. `agy` has no such flag: its rules
//! live in `~/.gemini/antigravity-cli/settings.json`, which is the student's
//! own file and shared with their editor, and this app does not write other
//! programs' global config. So containment here rests on two documented flags
//! and on the cwd:
//!
//! - `--sandbox` turns on the terminal sandbox, whose writable root is the
//!   workspace — and the workspace is `agents/`, because that is what the
//!   child is spawned in.
//! - `--dangerously-skip-permissions` is the headless counterpart of Claude's
//!   `--permission-prompts none`, and is here for the same reason: under `-p`
//!   a prompt with no answerer hangs the turn for ever. The two CLIs differ in
//!   which way they resolve it — Claude auto-*denies* and is handed an allow
//!   list to compensate, `agy` has no way to take an allow list and so must
//!   auto-*allow*, with the sandbox left as the thing that actually bounds it.
//!
//! **That is weaker than the other three bridges and is recorded as such.**
//! Codex gets a seatbelt with an explicit writable-file list, opencode a
//! rendered `opencode.json`, Claude the settings document above. This one gets
//! a sandbox and a working directory. What that costs in practice is the
//! section below.
//!
//! **No rewind.** Nothing in the published protocol takes a conversation back
//! to an earlier message: `--conversation` resumes, and there is no control
//! channel to ask anything else of. [`AntigravitySession::rewind`] therefore
//! refuses rather than pretending, and the manager surfaces that refusal — a
//! rewind that quietly did nothing would leave the thread and the agent out of
//! step in the one place a student is guaranteed to notice.
//!
//! **No interrupt over the protocol either**, for the same reason. Stopping a
//! turn is a signal to the child, and the turn is closed here rather than by
//! anything the CLI says.
//!
//! ## The sandbox refuses `oculus` by its bare name
//!
//! Measured: the first `oculus list` of a thread comes back
//! `zsh:1: operation not permitted: oculus`, and the agent recovers by finding
//! the binary and running it by absolute path — at the cost of a turn. This is
//! the gap `--dangerously-skip-permissions` cannot close, because it is the
//! *sandbox* refusing, not a permission prompt: Claude's bridge answers the
//! same problem with `Bash(oculus:*)` plus the absolute path in its allow
//! list, and there is no allow list to write here.
//!
//! Two smaller consequences of the same shape. A command that exits non-zero
//! is still a *successful tool call* — `tool_info.error` is for the tool
//! failing, not for the command's exit status — so such a row is closed `ok`
//! with the failure in its output, which is what the timeline shows. And the
//! database is not reachable through a hole punched for it the way Claude's
//! `allowWrite` and Codex's `writable_files` punch one, so whether
//! `oculus task add` can open `oculus.db-wal` is the sandbox's decision.
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
    /// `--effort`: `low`, `medium` or `high`. Antigravity bakes the level into
    /// most model slugs as well, so this is left off whenever the picker did
    /// not offer one and the slug speaks for itself.
    pub effort: Option<String>,
    /// The per-thread half of the brief. There is no `--append-system-prompt`,
    /// and the library-wide half is already on disk as `agents/AGENTS.md`,
    /// which `agy` reads by itself — so this rides the first user message, the
    /// way opencode's `brief` does.
    pub brief: String,
    pub env: Vec<(String, String)>,
    pub raw_log: Option<RawLog>,
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
            // The headless pair: the sandbox bounds what a command may touch,
            // and skipping the prompts is what stops a turn hanging on an
            // approval nothing can answer. See the module docs.
            .arg("--sandbox")
            .arg("--dangerously-skip-permissions")
            // Claude's `acceptEdits` by another name: edits land without an
            // approval round-trip, which is the only workable setting when
            // nothing can approve.
            .args(["--mode", "accept-edits"])
            // The library, opened for reads. The workspace is `agents/`
            // because that is the cwd, and without this the courses beside it
            // are outside every tool's reach — the same job Claude's
            // `--add-dir` does, spelled the same way.
            .arg("--add-dir")
            .arg(&cfg.library);
        if let Some(m) = &cfg.model {
            cmd.args(["--model", m]);
        }
        if let Some(e) = &cfg.effort {
            cmd.args(["--effort", e]);
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
        // agent is the likely shape of a first run that needs a browser.
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
    /// drops a message and everything after it. The manager deletes rows on
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
                        name,
                        title,
                        input,
                    });
                }
                if state == "DONE" {
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
/// was.)
///
/// It has no `--json` flag in the published reference, so the output is parsed
/// as lines. Anything that does not look like a slug is skipped rather than
/// guessed at, and an empty list is returned as an empty list — the picker
/// says "no models" and the student can run `agy models` themselves to see
/// the same nothing.
pub fn list_models(bin: &std::path::Path, env: &[(String, String)]) -> Result<Vec<ModelInfo>, String> {
    let out = Command::new(bin)
        .arg("models")
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run {}: {e}", bin.display()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        return Err(if err.is_empty() {
            format!("`agy models` exited {}", out.status)
        } else {
            err.to_string()
        });
    }
    Ok(parse_models(&String::from_utf8_lossy(&out.stdout)))
}

/// Slugs out of `agy models`' output.
///
/// The listing is a column of ids, possibly with a marker or a description
/// beside them, so the first whitespace-separated word of each line is the
/// candidate and everything else on the line is ignored. A line whose first
/// word is not slug-shaped — a heading, a blank, a box-drawing rule — is not a
/// model.
///
/// Effort is **baked into most slugs** (`gemini-3.8-flash-high`), which is why
/// no model here declares `reasoning_efforts`: offering a level beside a slug
/// that already names one would let a picker ask for `-high` at `low`. The
/// `--effort` flag still exists for slugs that carry no suffix, and a student
/// who wants it picks the slug that says it.
fn parse_models(stdout: &str) -> Vec<ModelInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut models = Vec::new();
    for line in stdout.lines() {
        let Some(word) = line.split_whitespace().next() else {
            continue;
        };
        let word = word.trim_matches(|c: char| !c.is_alphanumeric());
        if !is_slug(word) || !seen.insert(word.to_string()) {
            continue;
        }
        models.push(ModelInfo {
            id: word.to_string(),
            display_name: word.to_string(),
            reasoning_efforts: Vec::new(),
            default_reasoning_effort: None,
        });
    }
    models
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
        assert_eq!(
            ids,
            [
                "gemini-3.8-flash-high",
                "gemini-3.8-flash-medium",
                "gemini-3.8-flash-low",
                "claude-opus-5",
            ]
        );
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
}
