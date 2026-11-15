//! Claude's permission prompts as requests (`requests/`). A chat runs with
//! `--permission-prompts host --permission-prompt-tool stdio`, so a call the
//! sandbox and the rules leave undecided arrives as a `control_request`
//! `can_use_tool` and waits for a `control_response` on the same id: allow
//! with the tool's input as sent, or deny with a message. The CLI sends
//! nothing back for an answer, so a line the pipe took is all the proof
//! there is (`Reply::Acknowledged`); the tool's result says whether it ran.
//! `control_cancel_request` withdraws a prompt.
//!
//! Three kinds are asked, as recorded on 2.1.325: a Bash call (outside the
//! sandbox only when it sets `dangerouslyDisableSandbox`), a file tool
//! writing outside the allowed folders, and `SandboxNetworkAccess` for a new
//! host. Any other prompt is declined unseen. The CLI's suggested grants are
//! never passed on: the network one would persist to `localSettings`.

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

/// The request a `can_use_tool` prompt stands for, or `None` for a kind
/// Oculus does not ask about. A file change's paths, and the folders the CLI
/// suggests granting for it, are checked against the protected ones before
/// the student sees it.
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
        _ => {
            let (path, change) = file_change(&tool, input)?;
            let mut paths = vec![path];
            paths.extend(suggested_folders(request));
            if let Some(blocked) = request["blocked_path"].as_str() {
                paths.push(blocked.to_string());
            }
            let payload = json!({ "changes": [change], "reason": Value::Null });
            (Action::FileChange, payload, paths)
        }
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

/// The folders the CLI's `addDirectories` suggestions name. Never granted
/// here, but a grant there is what approving would invite, so a protected
/// one refuses the request.
fn suggested_folders(request: &Value) -> Vec<String> {
    let suggestions = request["permission_suggestions"].as_array();
    suggestions
        .into_iter()
        .flatten()
        .filter(|s| s["type"] == "addDirectories")
        .flat_map(|s| s["directories"].as_array().cloned().unwrap_or_default())
        .filter_map(|d| d.as_str().map(String::from))
        .collect()
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
