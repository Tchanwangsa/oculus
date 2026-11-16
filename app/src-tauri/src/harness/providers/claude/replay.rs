use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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
        turns: Arc::default(),
        pending: Mutex::new(HashMap::new()),
        background: Arc::default(),
        one_off: false,
        callbacks: Mutex::new(HashSet::new()),
        steering: Arc::default(),
    });
    let translator = Rc::new(RefCell::new(Translator {
        interrupting: session.interrupting.clone(),
        expecting: session.expecting.clone(),
        turns: session.turns.clone(),
        background_count: session.background.clone(),
        steering: session.steering.clone(),
        ..Default::default()
    }));
    let (reader, closer, reading, closing, eof_sink) = (
        translator.clone(),
        translator,
        session.clone(),
        session.clone(),
        sink.clone(),
    );
    Bridge {
        handle: Handle::Claude(session),
        inbound: Box::new(move |frame: &Value| {
            let mut translator = reader.borrow_mut();
            // Disable optional transcript-file discovery, without changing frames.
            translator.cwd.clear();
            reading.line(frame, &mut translator, &sink);
            translator.cwd.clear();
        }),
        eof: Box::new(move || closing.closed(&mut closer.borrow_mut(), &eof_sink)),
    }
}
