use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicI64;
use std::sync::{Arc, Mutex, Weak};

use serde_json::{json, Value};

use crate::harness::child::ChildProc;
use crate::harness::manager::Handle;
use crate::harness::testkit::{outbound::Outbound, replay::Bridge};
use crate::harness::Sink;

use super::{CodexServer, CodexThreadOpts, ThreadRoute, ThreadState};

pub(in crate::harness) fn bridge(outbound: Outbound, sink: Sink, id: &str, root: &Path) -> Bridge {
    let model = outbound.codex_model();
    let server = Arc::new_cyclic(|weak: &Weak<CodexServer>| {
        let weak = weak.clone();
        CodexServer {
            proc: ChildProc::replay(Arc::new(move |frame: Value| {
                let response = outbound.try_exchange(frame.clone())?;
                if frame.get("method").is_some() && frame.get("id").is_some() {
                    weak.upgrade()
                        .unwrap()
                        .dispatch(json!({"id": frame["id"], "result": response}));
                }
                Ok(())
            })),
            next_id: AtomicI64::new(1),
            pending: Mutex::new(HashMap::new()),
            routes: Mutex::new(HashMap::new()),
            account_sink: Some(sink.clone()),
        }
    });
    server.routes.lock().unwrap().insert(
        id.into(),
        Arc::new(ThreadRoute {
            sink,
            state: Mutex::new(ThreadState::default()),
        }),
    );
    let opts = Arc::new(CodexThreadOpts {
        cwd: root.join("agents"),
        writable_roots: Vec::new(),
        model,
        reasoning_effort: None,
        instructions: String::new(),
        ephemeral: true,
    });
    let (reader, closing) = (server.clone(), server.clone());
    Bridge {
        handle: Handle::Codex(server, id.into(), opts),
        inbound: Box::new(move |frame| reader.dispatch(frame.clone())),
        eof: Box::new(move || closing.closed()),
    }
}
