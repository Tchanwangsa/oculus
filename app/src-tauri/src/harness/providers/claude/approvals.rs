//! Claude's permission prompts as requests (`requests/`). A chat runs with
//! `--permission-prompts host --permission-prompt-tool stdio`, so a call the
//! sandbox and the rules leave undecided arrives as a `control_request`
//! `can_use_tool` and waits for a `control_response` on the same id: allow
//! with the tool's input as sent, or deny with a message. The CLI sends
//! nothing back for an answer, so a line the pipe took is all the proof
//! there is (`Reply::Acknowledged`); the tool's result says whether it ran.
//! `control_cancel_request` withdraws a prompt.
//!
//! Recorded on 2.1.325: a Bash call (outside the sandbox only when it sets
//! `dangerouslyDisableSandbox`), a file tool writing outside the allowed
//! folders, and `SandboxNetworkAccess` for a new host. Any other tool (a read
//! outside the allowed folders, a web fetch or search, an MCP tool) is asked
//! as a `tool` request; no such prompt has been recorded, so its shape is the
//! CLI's generic one. `AskUserQuestion` is declined unseen. The CLI's
//! suggested grants are never passed on: the network one would persist to
//! `localSettings`.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

use serde_json::{json, Value};

use crate::harness::child::str_of as s;
use crate::harness::requests::{
    Action, Decision, LiveRoute, NewRequest, Reply, RequestOption, Route,
};
use crate::harness::{HarnessEvent, Sink};

use super::session::ClaudeSession;

/// The pseudo-tool a sandboxed command's new host asks under. Its
/// `tool_use_id` is not the command's.
const NETWORK: &str = "SandboxNetworkAccess";

/// The tools that write a file, asked as a file change.
const FILE_TOOLS: [&str; 4] = ["Write", "Edit", "MultiEdit", "NotebookEdit"];

/// The tools that read files or list folders.
const READ_TOOLS: [&str; 5] = ["Read", "NotebookRead", "Glob", "Grep", "LS"];

/// Questions are answered nowhere yet: a chat's launch disallows the tool,
/// and a prompt for it is declined unseen.
const QUESTION: &str = "AskUserQuestion";

/// What the agent is told when the student declines.
pub(super) const DECLINED: &str =
    "The student declined this. Do not retry it or try another way to do the same thing.";

/// What the agent is told when Oculus declines without asking: the chat is
/// set to Don't ask, no turn is open, the call names a protected path, Oculus
/// does not ask about this kind of call, or its session has ended.
pub(super) const DECLINED_UNSEEN: &str = "Oculus declined this without asking the student. Do not retry it or try another way to do the same thing.";

/// A subagent the CLI started, from its `task_started`. Its requests carry
/// the task's id as `agent_id`, not the Agent call's id.
#[derive(Clone, Debug, Default)]
pub(super) struct Agent {
    /// `subagent_type`: the name the row shows.
    pub kind: String,
    /// The `Agent` call that started it.
    pub tool: String,
    pub description: String,
}

/// The answer to callback `wire_id`.
fn response(wire_id: &str, input: &Value, decision: Decision, message: &str) -> Value {
    let answer = match decision {
        Decision::Accept | Decision::AcceptForSession => {
            json!({ "behavior": "allow", "updatedInput": input })
        }
        Decision::Decline => json!({ "behavior": "deny", "message": message }),
    };
    json!({
        "type": "control_response",
        "response": { "subtype": "success", "request_id": wire_id, "response": answer },
    })
}

/// The way back to one prompt.
struct ClaudeRoute {
    session: Weak<ClaudeSession>,
    wire_id: String,
    input: Value,
}

impl ClaudeRoute {
    fn write(&self, decision: Decision, message: &str) -> Reply {
        let Some(session) = self.session.upgrade().filter(|s| s.is_alive()) else {
            return Reply::Failed("the claude process is not running".into());
        };
        session.callbacks.lock().unwrap().remove(&self.wire_id);
        let line = response(&self.wire_id, &self.input, decision, message);
        match session.proc.write_line(&line) {
            Ok(()) => Reply::Acknowledged,
            Err(e) => Reply::Uncertain(e),
        }
    }
}

impl Route for ClaudeRoute {
    fn reply(&self, option: &RequestOption) -> Reply {
        self.write(option.decision, DECLINED)
    }

    fn withdraw(&self) {
        let _ = self.write(Decision::Decline, DECLINED_UNSEEN);
    }
}

fn option(id: &str, label: &str, decision: Decision) -> RequestOption {
    RequestOption {
        id: id.into(),
        label: label.into(),
        decision,
        scope: None,
    }
}

/// What a file tool would write, in the shape a file change's row reads
/// (`changes[{path, kind, diff}]`), and the path it names.
fn file_change(tool: &str, input: &Value) -> Option<(String, Value)> {
    let added = |text: &str| text.lines().map(|l| format!("+{l}\n")).collect::<String>();
    let removed = |text: &str| text.lines().map(|l| format!("-{l}\n")).collect::<String>();
    let edit = |e: &Value| {
        format!(
            "{}{}",
            removed(&s(e, "old_string")),
            added(&s(e, "new_string"))
        )
    };
    let (path, kind, diff) = match tool {
        "Write" => {
            let path = s(input, "file_path");
            let kind = match std::path::Path::new(&path).exists() {
                true => "update",
                false => "add",
            };
            (path, kind, added(&s(input, "content")))
        }
        "Edit" => (s(input, "file_path"), "update", edit(input)),
        "MultiEdit" => {
            let edits = input["edits"].as_array()?;
            let diff = edits.iter().map(edit).collect::<Vec<_>>().join("@@\n");
            (s(input, "file_path"), "update", diff)
        }
        "NotebookEdit" => (
            s(input, "notebook_path"),
            "update",
            added(&s(input, "new_source")),
        ),
        _ => return None,
    };
    if path.is_empty() {
        return None;
    }
    let change = json!({ "path": path, "kind": kind, "diff": diff });
    Some((path, change))
}

/// Any other tool's call, in the shape a `tool` request's row reads: the
/// tool's `name`, a `kind` (`read`, `fetch`, `search`, `mcp` or `other`) and
/// what it would touch. A read names no `paths`: `protected.rs` keeps agents
/// from writing, and the library is readable already.
fn tool_call(tool: &str, request: &Value) -> Value {
    let input = &request["input"];
    let text = |key: &str| Some(s(input, key)).filter(|v| !v.is_empty());
    match tool {
        t if READ_TOOLS.contains(&t) => {
            let path = text("file_path")
                .or_else(|| text("notebook_path"))
                .or_else(|| text("path"))
                .or_else(|| request["blocked_path"].as_str().map(String::from));
            json!({ "name": tool, "kind": "read", "path": path, "pattern": text("pattern") })
        }
        "WebFetch" => {
            json!({ "name": tool, "kind": "fetch", "url": text("url"), "prompt": text("prompt") })
        }
        "WebSearch" => json!({ "name": tool, "kind": "search", "query": text("query") }),
        _ => match mcp_names(tool, request) {
            Some((server, name)) => json!({
                "name": tool,
                "kind": "mcp",
                "mcp": { "server": server, "tool": name },
                "arguments": input,
            }),
            None => json!({ "name": tool, "kind": "other", "arguments": input }),
        },
    }
}

/// An MCP tool's server and its own name, from `mcp__<server>__<tool>`. The
/// prompt's `mcp_server.name` is the server as configured, which the tool
/// name carries normalised.
fn mcp_names(tool: &str, request: &Value) -> Option<(String, String)> {
    let rest = tool.strip_prefix("mcp__")?;
    let configured = request["mcp_server"]["name"]
        .as_str()
        .filter(|n| !n.is_empty());
    let exact = configured.and_then(|c| Some((c, rest.strip_prefix(&format!("{c}__"))?)));
    if let Some((server, name)) = exact {
        return Some((server.to_string(), name.to_string()));
    }
    let (server, name) = rest.split_once("__")?;
    Some((configured.unwrap_or(server).to_string(), name.to_string()))
}

/// The request a `can_use_tool` prompt stands for, or `None` for a kind
/// Oculus does not ask about. A file change is judged by the file it writes
/// and the CLI's `blocked_path`, checked against the protected ones before
/// the student sees it; the folder the CLI suggests granting is never
/// granted, so it is not checked.
pub(super) fn request_of(
    request: &Value,
    wire_id: &str,
    agents: &HashMap<String, Agent>,
) -> Option<NewRequest> {
    let tool = s(request, "tool_name");
    let input = &request["input"];
    let (action, mut payload, paths) = match tool.as_str() {
        "Bash" => {
            let payload = json!({
                "command": input["command"],
                "reason": input["description"],
                "outsideSandbox": input["dangerouslyDisableSandbox"] == true,
            });
            (Action::Command, payload, Vec::new())
        }
        NETWORK => {
            let host = s(input, "host");
            if host.is_empty() {
                return None;
            }
            (Action::Network, json!({ "host": host }), Vec::new())
        }
        QUESTION => return None,
        t if FILE_TOOLS.contains(&t) => {
            let (path, change) = file_change(&tool, input)?;
            let mut paths = vec![path];
            if let Some(blocked) = request["blocked_path"].as_str() {
                paths.push(blocked.to_string());
            }
            let payload = json!({ "changes": [change], "reason": Value::Null });
            (Action::FileChange, payload, paths)
        }
        _ => (Action::Tool, tool_call(&tool, request), Vec::new()),
    };
    if action != Action::Network {
        payload["tool"] = json!(s(request, "tool_use_id"));
    }
    let agent_id = s(request, "agent_id");
    if !agent_id.is_empty() {
        let agent = agents.get(&agent_id).cloned().unwrap_or(Agent {
            kind: agent_id.clone(),
            ..Agent::default()
        });
        payload["agent"] = json!({
            "path": agent.kind,
            "tool": agent.tool,
            "description": agent.description,
        });
    }
    Some(NewRequest {
        action,
        payload,
        options: vec![
            option("accept", "Approve once", Decision::Accept),
            option("decline", "Decline", Decision::Decline),
        ],
        native_ref: json!(wire_id).to_string(),
        paths,
    })
}

impl ClaudeSession {
    /// A `control_request` from the CLI. A permission prompt goes to the
    /// chat, whose saved mode decides whether it is shown; it is declined
    /// at once on a one-off turn, while the turn is being stopped, or when
    /// Oculus does not ask about its kind. Anything else is refused.
    pub(super) fn control_request(
        self: &Arc<Self>,
        v: &Value,
        agents: &HashMap<String, Agent>,
        sink: &Sink,
    ) {
        let Some(wire_id) = v["request_id"].as_str() else {
            return;
        };
        let request = &v["request"];
        if request["subtype"] != "can_use_tool" {
            let refused = json!({
                "type": "control_response",
                "response": {
                    "subtype": "error",
                    "request_id": wire_id,
                    "error": "Oculus does not answer this control request",
                },
            });
            if let Err(e) = self.proc.write_line(&refused) {
                elog!("[oculus] claude: refusing a control request: {e}");
            }
            return;
        }
        let stopping = self.interrupting.load(Ordering::SeqCst);
        let asked = match self.one_off || stopping {
            true => None,
            false => request_of(request, wire_id, agents),
        };
        let Some(asked) = asked else {
            elog!(
                "[oculus] claude: declined a {} prompt without asking",
                s(request, "tool_name")
            );
            let line = response(wire_id, &Value::Null, Decision::Decline, DECLINED_UNSEEN);
            if let Err(e) = self.proc.write_line(&line) {
                elog!("[oculus] claude: declining a prompt: {e}");
            }
            return;
        };
        self.callbacks.lock().unwrap().insert(wire_id.to_string());
        let route = ClaudeRoute {
            session: Arc::downgrade(self),
            wire_id: wire_id.to_string(),
            input: request["input"].clone(),
        };
        sink(HarnessEvent::RequestAsked {
            request: asked,
            route: Some(LiveRoute(Arc::new(route))),
        });
    }

    /// `control_cancel_request`: the CLI gave a prompt up. After our own
    /// interrupt the turn's end settles it as stopped; otherwise the CLI
    /// withdrew it. One already answered changes nothing.
    pub(super) fn control_cancelled(&self, v: &Value, sink: &Sink) {
        let wire_id = s(v, "request_id");
        if !self.callbacks.lock().unwrap().remove(&wire_id) {
            return;
        }
        if self.interrupting.load(Ordering::SeqCst) {
            return;
        }
        sink(HarnessEvent::RequestRetired {
            native_ref: json!(wire_id).to_string(),
        });
    }

    /// The CLI is waiting on a prompt nobody has answered.
    pub fn has_pending_request(&self) -> bool {
        !self.callbacks.lock().unwrap().is_empty()
    }
}
