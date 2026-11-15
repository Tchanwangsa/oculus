//! Codex's approval callbacks as requests (`requests/`). A callback is the
//! server asking us something with a JSON-RPC id; it is answered on the
//! same id, kept exactly as sent (a number in every recording, a string by
//! the schema). `serverRequest/resolved` retires a callback after an
//! answer and after a withdrawal alike, so it proves only that.
//!
//! A file change's request carries no paths: they are on the `fileChange`
//! item that came before it, joined here so Rust can check them before the
//! student sees the request, and declined when the join finds nothing. A
//! spawned child agent asks under its own thread id, which is joined to its
//! parent's chat through the parent's `subAgentActivity` item.

use std::sync::{Arc, Weak};

use serde_json::{json, Value};

use crate::harness::child::str_of as s;
use crate::harness::requests::{
    Action, Decision, LiveRoute, NewRequest, Reply, RequestOption, Route,
};
use crate::harness::HarnessEvent;

use super::server::CodexServer;
use super::{ThreadRoute, ThreadState};

pub(super) const COMMAND: &str = "item/commandExecution/requestApproval";
pub(super) const FILE_CHANGE: &str = "item/fileChange/requestApproval";
pub(super) const PERMISSIONS: &str = "item/permissions/requestApproval";

/// The scope an exact-command grant was shown to have (probe `16`): the
/// same command in this chat's later turns; a changed argument or another
/// chat asks again. Whether it outlives a restart is not known, so this
/// does not say.
pub(super) const SESSION_SCOPE: &str = "Codex runs this exact command again in this chat without asking while its session lasts. A changed command, or another chat, asks again.";

/// What a reply to `method` is for `decision`. A permissions request is
/// answered with the profile it asked for, or an empty one to decline,
/// for this turn only.
fn response(method: &str, permissions: &Value, decision: Decision) -> Value {
    match (method, decision) {
        (PERMISSIONS, Decision::Decline) => json!({ "permissions": {}, "scope": "turn" }),
        (PERMISSIONS, _) => json!({ "permissions": permissions, "scope": "turn" }),
        (_, Decision::Accept) => json!({ "decision": "accept" }),
        (_, Decision::AcceptForSession) => json!({ "decision": "acceptForSession" }),
        (_, Decision::Decline) => json!({ "decision": "decline" }),
    }
}

/// The way back to one callback.
struct CodexRoute {
    server: Weak<CodexServer>,
    /// The route the callback was raised on: its chat's, a child's too.
    thread: Arc<ThreadRoute>,
    wire_id: Value,
    method: String,
    permissions: Value,
}

impl CodexRoute {
    fn write(&self, decision: Decision) -> Reply {
        let Some(server) = self.server.upgrade().filter(|s| s.is_alive()) else {
            return Reply::Failed("the codex app-server is not running".into());
        };
        let native = self.wire_id.to_string();
        self.thread.state.lock().unwrap().callbacks.remove(&native);
        let result = response(&self.method, &self.permissions, decision);
        let line = json!({ "jsonrpc": "2.0", "id": self.wire_id, "result": result });
        match server.proc.write_line(&line) {
            Ok(()) => Reply::Written,
            Err(e) => Reply::Uncertain(e),
        }
    }
}

impl Route for CodexRoute {
    fn reply(&self, option: &RequestOption) -> Reply {
        self.write(option.decision)
    }

    fn withdraw(&self) {
        let _ = self.write(Decision::Decline);
    }
}

fn option(id: &str, label: &str, decision: Decision, scope: Option<&str>) -> RequestOption {
    RequestOption {
        id: id.into(),
        label: label.into(),
        decision,
        scope: scope.map(String::from),
    }
}

/// The request a callback stands for, from its params and what the thread
/// has seen: `None` when its paths cannot be checked (a file change whose
/// item is unknown, a permissions grant on a glob or a special root).
pub(super) fn request_of(
    method: &str,
    id: &Value,
    params: &Value,
    state: &ThreadState,
    agent: Option<&str>,
) -> Option<NewRequest> {
    let item = s(params, "itemId");
    let reason = params.get("reason").cloned().unwrap_or(Value::Null);
    let (action, mut payload, paths, options) = match method {
        FILE_CHANGE => {
            let changes = state.changes.get(&item)?.clone();
            let paths: Vec<String> = changes
                .as_array()?
                .iter()
                .flat_map(|c| {
                    let moved = c.pointer("/kind/move_path").and_then(|p| p.as_str());
                    std::iter::once(s(c, "path")).chain(moved.map(String::from))
                })
                .filter(|p| !p.is_empty())
                .collect();
            if paths.is_empty() {
                return None;
            }
            let payload =
                json!({ "changes": changes, "reason": reason, "grantRoot": params["grantRoot"] });
            let options = vec![
                option("accept", "Approve", Decision::Accept, None),
                option("decline", "Decline", Decision::Decline, None),
            ];
            (Action::FileChange, payload, paths, options)
        }
        PERMISSIONS => {
            let asked = &params["permissions"];
            let mut paths: Vec<String> = asked
                .pointer("/fileSystem/write")
                .and_then(|w| w.as_array())
                .map(|w| {
                    w.iter()
                        .filter_map(|p| p.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let entries = asked
                .pointer("/fileSystem/entries")
                .and_then(|e| e.as_array());
            for entry in entries.into_iter().flatten() {
                if entry["access"] == "read" || entry["access"] == "deny" {
                    continue;
                }
                // A glob or a special root (`root`, `project_roots`, …)
                // cannot be checked against the protected paths.
                let path = entry.pointer("/path/path").and_then(|p| p.as_str());
                match (entry.pointer("/path/type").and_then(|t| t.as_str()), path) {
                    (Some("path"), Some(path)) => paths.push(path.to_string()),
                    _ => return None,
                }
            }
            let payload = json!({ "permissions": asked, "cwd": params["cwd"], "reason": reason });
            let options = vec![
                option(
                    "accept",
                    "Allow for this turn",
                    Decision::Accept,
                    Some("Until this turn ends."),
                ),
                option("decline", "Decline", Decision::Decline, None),
            ];
            (Action::Permissions, payload, paths, options)
        }
        _ => {
            let network = !params["networkApprovalContext"].is_null();
            let payload = json!({
                "command": params["command"],
                "cwd": params["cwd"],
                "reason": reason,
                "actions": params["commandActions"],
                "network": params["networkApprovalContext"],
                "outsideSandbox": true,
            });
            let options = vec![
                option("accept", "Approve once", Decision::Accept, None),
                option(
                    "accept_for_session",
                    "Always allow this exact command",
                    Decision::AcceptForSession,
                    Some(SESSION_SCOPE),
                ),
                option("decline", "Decline", Decision::Decline, None),
            ];
            let action = match network {
                true => Action::Network,
                false => Action::Command,
            };
            (action, payload, Vec::new(), options)
        }
    };
    payload["tool"] = json!(item);
    if let Some(path) = agent {
        payload["agent"] = json!({ "path": path, "thread": s(params, "threadId") });
    }
    Some(NewRequest {
        action,
        payload,
        options,
        native_ref: id.to_string(),
        paths,
    })
}

impl CodexServer {
    /// The route a thread's callbacks go to: its own, or for a child agent
    /// its parent chat's, with the child's path.
    fn asker(&self, thread: &str) -> Option<(Arc<ThreadRoute>, Option<String>)> {
        let routes = self.routes.lock().unwrap();
        if let Some(route) = routes.get(thread) {
            return Some((route.clone(), None));
        }
        let children = self.children.lock().unwrap();
        let (parent, path) = children.get(thread)?;
        routes.get(parent).map(|r| (r.clone(), Some(path.clone())))
    }

    /// A callback: declined at once where nobody is asked (a one-off turn,
    /// a thread nobody routes, paths that cannot be checked), else handed to
    /// the chat, whose saved mode decides whether it is shown.
    pub(super) fn approval(&self, id: &Value, method: &str, params: &Value) {
        let decline = |server: &Self| {
            let permissions = Value::Null;
            server.respond(id, response(method, &permissions, Decision::Decline));
        };
        let Some((route, agent)) = self.asker(&s(params, "threadId")) else {
            return decline(self);
        };
        let request = {
            let state = route.state.lock().unwrap();
            if state.one_off {
                None
            } else {
                request_of(method, id, params, &state, agent.as_deref())
            }
        };
        let Some(request) = request else {
            return decline(self);
        };
        route
            .state
            .lock()
            .unwrap()
            .callbacks
            .insert(request.native_ref.clone());
        let live = CodexRoute {
            server: self.me.clone(),
            thread: route.clone(),
            wire_id: id.clone(),
            method: method.into(),
            permissions: params["permissions"].clone(),
        };
        (route.sink)(HarnessEvent::RequestAsked {
            request,
            route: Some(LiveRoute(Arc::new(live))),
        });
    }

    /// `serverRequest/resolved`: the callback is retired, answered or not.
    pub(super) fn retired(&self, params: &Value) {
        let Some((route, _)) = self.asker(&s(params, "threadId")) else {
            return;
        };
        let native = params["requestId"].to_string();
        route.state.lock().unwrap().callbacks.remove(&native);
        (route.sink)(HarnessEvent::RequestRetired { native_ref: native });
    }

    /// A child agent's thread, learned from its parent's `subAgentActivity`.
    pub(super) fn learn_child(&self, params: &Value) {
        let item = &params["item"];
        if s(item, "type") != "subAgentActivity" {
            return;
        }
        let (child, parent) = (s(item, "agentThreadId"), s(params, "threadId"));
        if !child.is_empty() && !parent.is_empty() {
            let path = s(item, "agentPath");
            self.children.lock().unwrap().insert(child, (parent, path));
        }
    }

    /// The thread's CLI is waiting on a callback nobody has answered.
    pub fn has_pending_request(&self, thread_id: &str) -> bool {
        self.routes
            .lock()
            .unwrap()
            .get(thread_id)
            .is_some_and(|r| !r.state.lock().unwrap().callbacks.is_empty())
    }
}
