//! Folding agy's stream-json events into [`HarnessEvent`]s.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;

use crate::harness::event::{cap_output, classify, HarnessEvent, Provider};

/// Per-process translation state, keyed by `step_index`.
#[derive(Default)]
pub(super) struct Translator {
    pub(super) turn_open: bool,
    /// The `ACTIVE` step's text, persisted as one `AssistantMessage` at `DONE`.
    pub(super) step_text: String,
    /// A new index flushes too, so a missing `DONE` cannot merge two answers.
    pub(super) step_index: Option<i64>,
    /// Steps whose `ToolStarted` went out (`tool_info` repeats every update).
    pub(super) started_tools: std::collections::HashSet<i64>,
    /// A step already emitted `PermissionNeeded`; skip `denied_actions`.
    pub(super) refused: bool,
    pub(super) interrupting: Arc<AtomicBool>,
    pub(super) expecting: Arc<AtomicBool>,
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

    pub(super) fn translate(&mut self, v: &Value) -> Vec<HarnessEvent> {
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
                        let field =
                            |k: &str| d.get(k).and_then(|s| s.as_str()).unwrap_or("").to_string();
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
    pub(super) fn step(&mut self, su: &Value, out: &mut Vec<HarnessEvent>) {
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
pub(super) fn is_question(message: &str) -> bool {
    let unsandboxed = message.starts_with("permission check failed for unsandboxed");
    message.starts_with("permission check failed")
        && !message.contains("deny rule")
        && (!unsandboxed || message.contains("user denied permission"))
}

/// A refusal's (action, target, suggested rule), off `permission check failed
/// for <action> "<target>": …` or else the tool's parameters. The rule is
/// narrow on purpose: a command's first word, a file's folder.
pub(super) fn refusal(
    tool: &str,
    params: &Value,
    message: &str,
) -> (String, Option<String>, Option<String>) {
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
        "write_to_file"
        | "replace_file_content"
        | "multi_replace_file_content"
        | "sed_file"
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
        _ => param(&[
            "TargetFile",
            "AbsolutePath",
            "DirectoryPath",
            "SearchDirectory",
            "SearchPath",
            "Path",
        ]),
    }
    .or_else(|| said.map(|(_, t)| t).filter(|t| !t.is_empty()));
    let rule = target.as_deref().and_then(|t| match action.as_str() {
        "command" => command_word(t).map(|w| format!("command({w})")),
        "write_file" | "read_file" => {
            // A folder is granted as itself; a file by the folder it is in.
            let p = std::path::Path::new(t);
            let dir = if tool == "list_dir" || p.is_dir() {
                Some(p)
            } else {
                p.parent()
            };
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
pub(super) fn command_word(line: &str) -> Option<String> {
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
