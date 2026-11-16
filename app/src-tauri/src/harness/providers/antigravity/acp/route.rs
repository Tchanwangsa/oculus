//! The guarded write back to an approval or question the student was asked.
//! agy offers Allow (`allow_once`) and Deny (`reject_once`) only, and sends
//! nothing back for an answer, so a line the pipe took is all the proof
//! there is. An answer is admitted only while the session is not being
//! torn down and the request belongs to its live generation (T3's guard):
//! a stop, the turn's end or teardown each start a new one.

use std::sync::atomic::Ordering;
use std::sync::Weak;

use serde_json::{json, Value};

use crate::harness::requests::{Decision, Reply, RequestOption, Route};

use super::session::AcpSession;

/// The option ids a request offers for allowing and refusing it once.
#[derive(Clone, Debug, Default)]
pub(super) struct Options {
    pub(super) allow: Option<String>,
    pub(super) reject: Option<String>,
}

impl Options {
    pub(super) fn of(params: &Value) -> Self {
        let by_kind = |kind: &str| {
            params["options"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|o| o["kind"] == kind)
                .and_then(|o| o["optionId"].as_str())
                .map(String::from)
        };
        Options {
            allow: by_kind("allow_once"),
            reject: by_kind("reject_once"),
        }
    }
}

/// The result an answer carries: the option chosen, or `cancelled` where
/// there is none to choose.
pub(super) fn outcome(option: Option<&str>) -> Value {
    match option {
        Some(id) => json!({ "outcome": { "outcome": "selected", "optionId": id } }),
        None => json!({ "outcome": { "outcome": "cancelled" } }),
    }
}

pub(super) struct AcpRoute {
    pub(super) session: Weak<AcpSession>,
    /// The request's id as agy sent it.
    pub(super) id: Value,
    /// The same, as JSON: its key among the session's callbacks.
    pub(super) wire: String,
    /// The callbacks' generation it was asked under.
    pub(super) generation: u64,
    pub(super) options: Options,
}

impl AcpRoute {
    pub(super) fn write(&self, result: Value) -> Reply {
        let Some(session) = self.session.upgrade() else {
            return Reply::Failed("the agy process is gone".into());
        };
        if session.closing.load(Ordering::SeqCst) || !session.is_alive() {
            return Reply::Failed("the agy process is not running".into());
        }
        {
            let mut callbacks = session.callbacks.lock().unwrap();
            if callbacks.0 != self.generation {
                return Reply::Failed("the turn it was asked in has been stopped".into());
            }
            if callbacks.1.remove(&self.wire).is_none() {
                return Reply::Failed("it was already answered".into());
            }
        }
        match session.rpc.answer(&self.id, result) {
            Ok(()) => Reply::Acknowledged,
            Err(e) => Reply::Uncertain(e),
        }
    }
}

impl Route for AcpRoute {
    fn reply(&self, option: &RequestOption) -> Reply {
        match option.decision {
            Decision::Accept | Decision::AcceptForSession => {
                self.write(outcome(self.options.allow.as_deref()))
            }
            Decision::Decline => self.write(outcome(self.options.reject.as_deref())),
        }
    }

    fn withdraw(&self) {
        let _ = self.write(outcome(self.options.reject.as_deref()));
    }
}
