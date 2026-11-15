use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::json;

use crate::harness::child::ChildProc;
use crate::harness::manager::Handle;
use crate::harness::testkit::{outbound::Outbound, replay::Bridge};
use crate::harness::Sink;

use super::{
    server::OpencodeServer,
    session::{SessionRoute, SessionState},
};

pub(in crate::harness) fn bridge(outbound: Outbound, sink: Sink, id: &str, root: &Path) -> Bridge {
    let pipe = outbound.clone();
    let server = Arc::new(OpencodeServer {
        proc: ChildProc::replay(Arc::new(move |frame| pipe.try_exchange(frame).map(|_| ()))),
        replay: Some(Arc::new(move |method, path, body| {
            outbound.exchange(json!({"method": method, "path": path, "body": body}))
        })),
        base: "replay://no-network".into(),
        directory: root.join("agents"),
        api: ureq::AgentBuilder::new().build(),
        stream: ureq::AgentBuilder::new().build(),
        routes: Mutex::new(HashMap::new()),
        default_sink: Some(sink.clone()),
        raw_log: None,
    });
    server.route(
        id,
        SessionRoute {
            sink,
            agent: "oculus",
            model: Mutex::new(None),
            variant: None,
            state: Mutex::new(SessionState::default()),
        },
    );
    let (reader, closing) = (server.clone(), server.clone());
    Bridge {
        handle: Handle::Opencode(server, id.into()),
        inbound: Box::new(move |frame| reader.dispatch(frame)),
        eof: Box::new(move || closing.on_exit()),
    }
}
