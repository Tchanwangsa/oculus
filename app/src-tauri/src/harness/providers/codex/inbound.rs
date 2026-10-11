//! What the app-server sends unprompted: responses, the server's own
//! requests, and thread or account notifications.

use serde_json::{json, Value};

use super::server::CodexServer;
use super::translate::{translate, translate_account};

impl CodexServer {
    pub(super) fn dispatch(&self, v: Value) {
        let has_id = v.get("id").map_or(false, |i| !i.is_null());
        let method = v.get("method").and_then(|m| m.as_str());
        match (has_id, method) {
            (true, None) => {
                let id = v.get("id").and_then(|i| i.as_i64()).unwrap_or(-1);
                if let Some(tx) = self.pending.lock().unwrap().remove(&id) {
                    let r = match v.get("error") {
                        Some(e) => Err(e
                            .get("message")
                            .and_then(|m| m.as_str())
                            .map(String::from)
                            .unwrap_or_else(|| e.to_string())),
                        None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                    };
                    let _ = tx.send(r);
                }
            }
            (true, Some(m)) => self.handle_server_request(&v["id"], m, &v["params"]),
            (false, Some(m)) => self.handle_notification(m, &v["params"]),
            (false, None) => {}
        }
    }

    /// `approvalPolicy: never` should mean no approvals arrive; one that does
    /// is refused rather than left hanging.
    fn handle_server_request(&self, id: &Value, method: &str, _params: &Value) {
        match method {
            "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
                self.respond(id, json!({ "decision": "decline" }));
            }
            "item/permissions/requestApproval" => {
                self.respond(id, json!({ "permissions": {}, "scope": "turn" }));
            }
            _ => self.respond_error(id, -32601, &format!("oculus does not handle {method}")),
        }
    }

    pub(super) fn handle_notification(&self, method: &str, params: &Value) {
        // Account-scoped first: they carry no `threadId`.
        let account = translate_account(method, params);
        if !account.is_empty() {
            if let Some(sink) = &self.account_sink {
                for ev in account {
                    sink(ev);
                }
            }
            return;
        }
        let thread_id = params
            .get("threadId")
            .and_then(|t| t.as_str())
            .map(String::from)
            .or_else(|| {
                params
                    .pointer("/thread/id")
                    .and_then(|t| t.as_str())
                    .map(String::from)
            });
        let Some(thread_id) = thread_id else {
            return;
        };
        let route = self.routes.lock().unwrap().get(&thread_id).cloned();
        let Some(route) = route else {
            return;
        };
        let events = {
            let mut st = route.state.lock().unwrap();
            translate(method, params, &mut st)
        };
        for ev in events {
            (route.sink)(ev);
        }
    }
}
