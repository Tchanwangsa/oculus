//! The Antigravity bridge: one long-lived `agy --print=` process per thread,
//! stream-json both ways. Claude's shape with a different vocabulary:
//! `init` / `step_update` / `result` events, `--conversation` for `--resume`.
//! No inline settings, no rewind and no protocol-level interrupt; containment
//! is rules in agy's global settings file ([`super::antigravity_rules`]) plus
//! `--sandbox`, which bounds shell commands only. See docs/harness.md.
//!
//! Two quirks of agy 1.2.9 that its reference does not show: `-p` takes the
//! prompt as its value, so stream-json mode needs `--print=` with an empty
//! *attached* value; and tool parameters are PascalCase (`CommandLine`,
//! `AbsolutePath`). `fixtures/harness/antigravity-ls.ndjson` is a real session.

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::child::{self, ChildProc, ThreadSpawn};
use super::event::{cap_output, classify, HarnessEvent, Provider};
use super::Sink;

pub struct AntigravitySpawn {
    /// `base.cwd` is also where `agy` reads `AGENTS.md` from; `base.effort`
    /// is folded into `--model` by [`model_slug`].
    pub base: ThreadSpawn,
    /// The per-thread half of the brief, riding the first user message (no
    /// system-prompt flag; `agy` reads `AGENTS.md` itself).
    pub brief: String,
    /// Approved rules; `None` reuses the last written. See
    /// [`super::antigravity_rules::install`].
    pub approved: Option<Vec<String>>,
}

pub struct AntigravitySession {
    proc: ChildProc,
    pending_brief: Mutex<Option<String>>,
    /// Set by [`Self::interrupt`], so the killed turn reads as interrupted.
    interrupting: Arc<AtomicBool>,
    /// A turn is owed a `result`; a death meanwhile must still close it.
    expecting: Arc<AtomicBool>,
}

impl AntigravitySession {
    pub fn spawn(cfg: AntigravitySpawn, sink: Sink) -> Result<Arc<Self>, String> {
        let base = cfg.base;
        // `agy` reads its rules once at start, so failing to write them fails
        // the spawn.
        super::antigravity_rules::install(&base.library, cfg.approved.clone())
            .map_err(|e| format!("Antigravity was not started: {e}"))?;
        let mut cmd = Command::new(&base.bin);
        // The `=` is load-bearing: see the module docs.
        cmd.arg("--print=")
            .args(["--input-format", "stream-json"])
            .args(["--output-format", "stream-json"])
            // A message starting with `/` is text, not a slash command.
            .arg("--disable-slash-commands")
            // Shell commands only; file tools answer to the rules. Never add
            // `--dangerously-skip-permissions`: it lets file tools write
            // outside the library (docs/harness.md).
            .arg("--sandbox")
            // Claude's `acceptEdits`: workspace edits need no approval.
            .args(["--mode", "accept-edits"])
            .arg("--add-dir")
            .arg(&base.library);
        if let Some(m) = &base.model {
            cmd.args(["--model", &model_slug(m, base.effort.as_deref())]);
        }
        if let Some(id) = &base.resume {
            cmd.args(["--conversation", id]);
        }
        cmd.current_dir(&base.cwd)
            .env_clear()
            .envs(base.env.iter().map(|(k, v)| (k, v)));
        let (proc, stdout) = ChildProc::spawn("agy", &mut cmd, true)?;

        let interrupting = Arc::new(AtomicBool::new(false));
        let expecting = Arc::new(AtomicBool::new(false));
        let session = Arc::new(AntigravitySession {
            proc,
            pending_brief: Mutex::new(
                (!cfg.brief.trim().is_empty()).then(|| cfg.brief.clone()),
            ),
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
                for ev in state.translate(&v) {
                    sink(ev);
                }
            });
            reader_session.proc.finish(&sink, Provider::Antigravity, || {
                expecting.swap(false, Ordering::SeqCst) || state.turn_open
            });
        });

        Ok(session)
    }

    pub fn is_alive(&self) -> bool {
        self.proc.is_alive()
    }

    /// One user turn, with the brief ahead of the first message.
    pub fn send(&self, text: &str) -> Result<(), String> {
        let brief = self.pending_brief.lock().unwrap().take();
        let text = match brief {
            Some(b) => format!("{}\n\n---\n\n{text}", b.trim()),
            None => text.to_string(),
        };
        self.expecting.store(true, Ordering::SeqCst);
        self.proc.write_line(&serde_json::json!({
            "event": "user",
            "message": { "content": text },
        }))
    }

    /// Stop the current turn by killing the child (the protocol has no
    /// interrupt); the next message resumes the conversation by id.
    pub fn interrupt(&self) -> Result<(), String> {
        if !self.is_alive() {
            return Ok(());
        }
        self.interrupting.store(true, Ordering::SeqCst);
        self.kill();
        Ok(())
    }

    /// Refuses: agy 1.2.9 has no headless rewind (`/rewind` is not available
    /// in print mode), and the manager deletes rows on an `Ok`.
    pub fn rewind(&self, _anchor: &str) -> Result<(), String> {
        Err("Antigravity cannot take a question back out of a conversation — \
             edit it in a new thread instead"
            .into())
    }

    pub fn kill(&self) {
        self.proc.kill();
    }
}

// ── Translation ──────────────────────────────────────────────────────────────

/// Per-process translation state, keyed by `step_index`.
#[derive(Default)]
struct Translator {
    turn_open: bool,
    /// The `ACTIVE` step's text, persisted as one `AssistantMessage` at `DONE`.
    step_text: String,
    /// A new index flushes too, so a missing `DONE` cannot merge two answers.
    step_index: Option<i64>,
    /// Steps whose `ToolStarted` went out (`tool_info` repeats every update).
    started_tools: std::collections::HashSet<i64>,
    /// A step already emitted `PermissionNeeded`; skip `denied_actions`.
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
                // A refusal-ended turn is a `SUCCESS` listing its refusals;
                // this covers one that never showed up as a step.
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
                // Seven statuses onto three; anything unexpected is a failure.
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

    /// One `step_update`. Only `agent_response` and `tool` steps make rows;
    /// `user_input` and `checkpoint` are ignored.
    fn step(&mut self, su: &Value, out: &mut Vec<HarnessEvent>) {
        let index = su.get("step_index").and_then(|i| i.as_i64()).unwrap_or(0);
        let state = su.get("state").and_then(|s| s.as_str()).unwrap_or("");
        let kind = su.get("step_type").and_then(|s| s.as_str()).unwrap_or("");

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
                // `tool_info` has no call id; a step is one call.
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
                // Refused: the row fails with the CLI's sentence, and only a
                // no-rule refusal (not a deny) asks for approval — see
                // [`is_question`].
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

        // The live figure; the `result`'s is the final one.
        if state == "DONE" {
            if let Some(u) = su.get("usage") {
                out.push(usage_event(u));
            }
        }
    }
}

/// Whether a refusal is print mode's automatic no (answerable; ends the turn)
/// rather than a deny rule (no allow beats it). Both start `permission check
/// failed`; a deny says `deny rule`, or for a command `for unsandboxed "…"`
/// without `user denied permission`.
fn is_question(message: &str) -> bool {
    let unsandboxed = message.starts_with("permission check failed for unsandboxed");
    message.starts_with("permission check failed")
        && !message.contains("deny rule")
        && (!unsandboxed || message.contains("user denied permission"))
}

/// A refusal's (action, target, suggested rule), off `permission check failed
/// for <action> "<target>": …` or else the tool's parameters. The rule is
/// narrow on purpose: a command's first word, a file's folder.
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

/// Antigravity's `usage` → the timeline's. No cost or window is reported.
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

/// One model `agy models` printed; the same shape as Codex's `ModelInfo`.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub reasoning_efforts: Vec<String>,
    pub default_reasoning_effort: Option<String>,
}

/// What this account can use, off `agy models` — a listing, nothing billed.
/// No `--json`, so lines are parsed (see [`parse_models`]).
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

/// Enough that a wedged `agy` cannot hold a model picker open.
const MODELS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// `agy models`, killed past [`MODELS_TIMEOUT`].
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
    // One thread per pipe, or a child filling the unread one blocks.
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

/// Models out of `agy models`' output: the first slug-shaped word per line.
/// The level is baked into the slug (`gemini-3.8-flash-high`), so slugs
/// sharing a base fold into one model whose `reasoning_efforts` are the
/// suffixes; [`model_slug`] rebuilds the slug at spawn.
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
    // Medium where offered, else the listing's first.
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

/// The name after the tab in a `<slug>\t<Display Name>` row (agy 1.2.9),
/// minus a folded level's ` (High)`. `None` without a tab.
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

/// The level suffixes a slug can end in; the names `validate_effort` accepts.
const LEVELS: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

/// `gemini-3.8-flash-high` → (`gemini-3.8-flash`, `high`). A slug that is
/// nothing but a level, or has no level suffix, keeps its whole self.
fn split_level(slug: &str) -> (&str, Option<&str>) {
    match slug.rsplit_once('-') {
        Some((base, l)) if !base.is_empty() && LEVELS.contains(&l) => (base, Some(l)),
        _ => (slug, None),
    }
}

/// The inverse of `split_level`. A slug that already ends in a level is
/// passed as it is rather than doubled.
fn model_slug(model: &str, effort: Option<&str>) -> String {
    match effort {
        Some(e) if split_level(model).1.is_none() => format!("{model}-{e}"),
        _ => model.to_string(),
    }
}

/// A slug as a person reads it: `claude-opus-4-6-thinking` → "Claude Opus 4.6
/// Thinking", `gpt-oss-120b` → "GPT-OSS 120B".
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

/// Slug-shaped, strictly: a false positive is an unselectable picker row.
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

    /// A real `agy` 1.2.9 session, recorded with the bridge's flags.
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

        // PascalCase `CommandLine`: a lowercase lookup titles this "".
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

        let finished: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::ToolFinished { ok, output, .. } => Some((*ok, output.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(finished.len(), 1);
        assert!(finished[0].0, "the command succeeded");
        assert!(finished[0].1.contains(".."), "stdout rode `output`");

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

        assert_eq!(
            events.iter().filter(|e| matches!(e, HarnessEvent::TurnStarted)).count(),
            1
        );
        assert!(matches!(
            events.last(),
            Some(HarnessEvent::TurnFinished { status }) if status == "completed"
        ));
        assert!(!events.iter().any(|e| matches!(e, HarnessEvent::Error { .. })));
    }

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

    /// The listing as 1.2.9 prints it.
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

    #[test]
    fn the_level_is_folded_back_into_the_slug() {
        assert_eq!(model_slug("gemini-3.8-flash", Some("high")), "gemini-3.8-flash-high");
        assert_eq!(model_slug("claude-sonnet-4-6", None), "claude-sonnet-4-6");
        assert_eq!(model_slug("gemini-3.8-flash-low", Some("high")), "gemini-3.8-flash-low");
        assert_eq!(model_slug("gemini-3.8-flash-low", None), "gemini-3.8-flash-low");
    }

    #[test]
    fn furniture_is_not_a_model() {
        assert!(parse_models("Models\n\n──────────\nNone found.\n").is_empty());
        assert!(!is_slug("models"));
        assert!(!is_slug("──────────"));
        assert!(!is_slug("a-"));
        assert!(is_slug("gemini-3.8-flash"));
    }

    #[test]
    fn a_repeated_slug_is_one_model() {
        let ids: Vec<String> = parse_models("gemini-3.8-flash\ngemini-3.8-flash\n")
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, ["gemini-3.8-flash"]);
    }

    /// A refused step as 1.2.9 sends it, then the `result` that ends the turn.
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

    /// A deny rule's refusal, verbatim off 1.2.9.
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
        assert!(!is_question(
            "permission check failed for unsandboxed \"sqlite3 /lib/oculus.db 'select 1'\": denied"
        ));
        assert!(is_question(
            "permission check failed for command \"python3\": user denied permission to run command: python3"
        ));
    }

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
