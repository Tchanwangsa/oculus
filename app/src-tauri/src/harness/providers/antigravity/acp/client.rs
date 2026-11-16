//! JSON-RPC 2.0 over `agy acp`'s stdio: our requests and their replies by
//! id, notifications, and answers to the agent's own requests. Protocol
//! only; what the messages mean is the session's.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::harness::child::ChildProc;

/// A setup call (`initialize`, `session/new` or `/load`, a setting) with no
/// reply in this long is a hung agent. A load replays the conversation
/// before it answers, so the bound is generous. `session/prompt` has none.
const SETUP_TIMEOUT: Duration = Duration::from_secs(120);

/// Run on the reader thread as a reply arrives, before any later line.
pub(super) type OnReply = Box<dyn FnOnce(Result<Value, String>) + Send>;

enum Waiter {
    Blocking(mpsc::Sender<Result<Value, String>>),
    Then(OnReply),
}

pub(in crate::harness) struct Rpc {
    pub(in crate::harness) proc: ChildProc,
    next_id: AtomicI64,
    waiting: Mutex<HashMap<i64, Waiter>>,
}

impl Rpc {
    pub(super) fn new(proc: ChildProc, first_id: i64) -> Self {
        Self {
            proc,
            next_id: AtomicI64::new(first_id),
            waiting: Mutex::new(HashMap::new()),
        }
    }

    fn write(&self, id: i64, method: &str, params: Value, waiter: Waiter) -> Result<(), String> {
        if !self.proc.is_alive() {
            return Err("agy is not running".into());
        }
        self.waiting.lock().unwrap().insert(id, waiter);
        let frame = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        self.proc.write_line(&frame).inspect_err(|_| {
            self.waiting.lock().unwrap().remove(&id);
        })
    }

    /// A request, waited on.
    pub(super) fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (reply, rx) = mpsc::channel();
        self.write(id, method, params, Waiter::Blocking(reply))?;
        match rx.recv_timeout(SETUP_TIMEOUT) {
            Ok(r) => r.map_err(|e| format!("agy {method}: {e}")),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.waiting.lock().unwrap().remove(&id);
                Err(format!(
                    "agy {method}: no reply in {}s",
                    SETUP_TIMEOUT.as_secs()
                ))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!("agy {method}: agy exited")),
        }
    }

    /// A request whose reply `on_reply` hears on the reader thread, however
    /// long it takes.
    pub(super) fn request_then(
        &self,
        method: &str,
        params: Value,
        on_reply: OnReply,
    ) -> Result<(), String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.write(id, method, params, Waiter::Then(on_reply))
    }

    pub(super) fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        self.proc
            .write_line(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
    }

    /// Answer the agent's request `id`, exactly as it sent the id.
    pub(super) fn answer(&self, id: &Value, result: Value) -> Result<(), String> {
        self.proc
            .write_line(&json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }

    /// Refuse the agent's request `id`.
    pub(super) fn refuse(&self, id: &Value, code: i64, message: &str) -> Result<(), String> {
        self.proc.write_line(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        }))
    }

    /// A reply to one of our requests, handed to its waiter; `false` when
    /// `frame` is not one.
    pub(super) fn resolve(&self, frame: &Value) -> bool {
        if frame.get("method").is_some() {
            return false;
        }
        let Some(id) = frame.get("id").and_then(Value::as_i64) else {
            return false;
        };
        if frame.get("result").is_none() && frame.get("error").is_none() {
            return false;
        }
        let Some(waiter) = self.waiting.lock().unwrap().remove(&id) else {
            return true;
        };
        let reply = match frame.get("error") {
            Some(e) => Err(e
                .get("message")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| e.to_string())),
            None => Ok(frame["result"].clone()),
        };
        match waiter {
            Waiter::Blocking(tx) => {
                let _ = tx.send(reply);
            }
            Waiter::Then(on_reply) => on_reply(reply),
        }
        true
    }

    /// The process is gone: every request still waiting hears `why`.
    pub(super) fn fail_all(&self, why: &str) {
        let waiting: Vec<Waiter> = self
            .waiting
            .lock()
            .unwrap()
            .drain()
            .map(|(_, w)| w)
            .collect();
        for waiter in waiting {
            match waiter {
                Waiter::Blocking(tx) => {
                    let _ = tx.send(Err(why.to_string()));
                }
                Waiter::Then(on_reply) => on_reply(Err(why.to_string())),
            }
        }
    }
}
