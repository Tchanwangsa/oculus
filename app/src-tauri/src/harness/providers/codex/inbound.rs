//! What the app-server sends unprompted: responses, the server's own
//! requests, and thread or account notifications.

use serde_json::Value;

use super::approvals::{COMMAND, FILE_CHANGE, PERMISSIONS};
use super::server::CodexServer;
use super::translate::{translate, translate_account};

impl CodexServer {
    pub(super) fn dispatch(&self, v: Value) {
        let has_id = v.get("id").is_some_and(|i| !i.is_null());
        let method = v.get("method").and_then(|m| m.as_str());
        match (has_id, method) {
            (true, None) => {
                let id = v.get("id").and_then(|i| i.as_i64()).unwrap_or(-1);
                let pending = self.pending.lock().unwrap().remove(&id);
                if let Some(pending) = pending {
                    let r = match v.get("error") {
                        Some(e) => Err(e
                            .get("message")
                            .and_then(|m| m.as_str())
                            .map(String::from)
                            .unwrap_or_else(|| e.to_string())),
                        None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                    };
                    if let (Ok(result), Some(on_result)) = (&r, pending.on_result) {
                        on_result(result);
                    }
                    let _ = pending.reply.send(r);
                }
            }
            (true, Some(m)) => self.handle_server_request(&v["id"], m, &v["params"]),
            (false, Some(m)) => self.handle_notification(m, &v["params"]),
            (false, None) => {}
        }
    }

    /// An approval becomes a request (`approvals.rs`); anything else the
    /// server asks is refused rather than left hanging.
    fn handle_server_request(&self, id: &Value, method: &str, params: &Value) {
        match method {
            COMMAND | FILE_CHANGE | PERMISSIONS => self.approval(id, method, params),
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
        match method {
            "serverRequest/resolved" => return self.retired(params),
            "item/started" => self.learn_child(params),
            _ => {}
        }
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
