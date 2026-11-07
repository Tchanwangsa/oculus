use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::harness::child::ChildProc;
use crate::harness::manager::Handle;
use crate::harness::testkit::{outbound::Wire, replay::Bridge};
use crate::harness::Sink;

use super::{session::ClaudeSession, translate::Translator};

pub(in crate::harness) fn bridge(wire: Wire, sink: Sink) -> Bridge {
    let session = Arc::new(ClaudeSession {
        proc: ChildProc::replay(wire),
        request_ids: AtomicU64::new(1),
        interrupting: Arc::default(),
        expecting: Arc::default(),
        pending: Mutex::new(HashMap::new()),
    });
    let translator = Rc::new(RefCell::new(Translator {
        interrupting: session.interrupting.clone(),
        expecting: session.expecting.clone(),
        ..Default::default()
    }));
    let (reader, closer, closing, eof_sink) = (
        translator.clone(),
        translator,
        session.clone(),
        sink.clone(),
    );
    Bridge {
        handle: Handle::Claude(session),
        inbound: Box::new(move |frame: &Value| {
            let mut translator = reader.borrow_mut();
            // Disable optional transcript-file discovery, without changing frames.
            translator.cwd.clear();
            for event in translator.translate(frame) {
                sink(event);
            }
            translator.cwd.clear();
        }),
        eof: Box::new(move || closing.closed(&mut closer.borrow_mut(), &eof_sink)),
    }
}
