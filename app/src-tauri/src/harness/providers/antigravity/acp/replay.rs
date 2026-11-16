//! A recorded `agy acp` session, opened already: the replay starts at the
//! first prompt. A request of ours other than `session/prompt` is answered
//! with its manifest response as it is written; a prompt's reply is in the
//! recording. `$LIBRARY` in a recorded frame is the replay's library.

use std::path::Path;
use std::sync::{Arc, Weak};

use serde_json::{json, Value};

use crate::harness::child::{ChildProc, ThreadSpawn};
use crate::harness::manager::Handle;
use crate::harness::testkit::{outbound::Outbound, replay::Bridge};
use crate::harness::Sink;

use super::session::AcpSession;

/// `frame` with `$LIBRARY` in its strings replaced by `root`.
pub(in crate::harness) fn place(frame: &Value, root: &Path) -> Value {
    match frame {
        Value::String(s) => Value::String(s.replace("$LIBRARY", &root.display().to_string())),
        Value::Array(a) => a.iter().map(|v| place(v, root)).collect(),
        Value::Object(o) => o.iter().map(|(k, v)| (k.clone(), place(v, root))).collect(),
        v => v.clone(),
    }
}

/// The recorded session, with nothing under a raw log, as a one-off turn
/// when `one_off`.
pub(in crate::harness) fn session(
    outbound: Outbound,
    sink: Sink,
    native_session: &str,
    root: &Path,
    one_off: bool,
) -> Arc<AcpSession> {
    let first_id = outbound.next_rpc_id().unwrap_or(1);
    let base = ThreadSpawn {
        bin: "agy".into(),
        cwd: root.join("agents"),
        library: root.to_path_buf(),
        resume: Some(native_session.to_string()),
        model: None,
        effort: None,
        env: Vec::new(),
        raw_log: None,
    };
    let reply_to: Arc<std::sync::Mutex<Weak<AcpSession>>> = Arc::default();
    let replier = reply_to.clone();
    let proc = ChildProc::replay(Arc::new(move |frame: Value| {
        let response = outbound.try_exchange(frame.clone())?;
        let ours = frame.get("method").is_some() && frame.get("id").is_some();
        if ours && frame["method"] != "session/prompt" {
            if let Some(session) = replier.lock().unwrap().upgrade() {
                session.rpc_reply(json!({ "id": frame["id"], "result": response }));
            }
        }
        Ok(())
    }));
    let session = AcpSession::new(proc, first_id, sink, &base, one_off);
    *reply_to.lock().unwrap() = Arc::downgrade(&session);
    session.opened_for_replay();
    session
}

pub(in crate::harness) fn bridge(
    outbound: Outbound,
    sink: Sink,
    native_session: &str,
    root: &Path,
) -> Bridge {
    let session = session(outbound, sink, native_session, root, false);
    let (reading, closing, root) = (session.clone(), session.clone(), root.to_path_buf());
    Bridge {
        handle: Handle::AntigravityAcp(session),
        inbound: Box::new(move |frame| reading.inbound(&place(frame, &root))),
        eof: Box::new(move || closing.closed()),
    }
}
