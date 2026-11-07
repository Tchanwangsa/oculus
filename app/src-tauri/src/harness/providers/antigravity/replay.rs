use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::harness::child::ChildProc;
use crate::harness::manager::Handle;
use crate::harness::testkit::{outbound::Wire, replay::Bridge};
use crate::harness::Sink;

use super::{session::AntigravitySession, translate::Translator};

pub(in crate::harness) fn bridge(wire: Wire, sink: Sink) -> Bridge {
    let session = Arc::new(AntigravitySession {
        proc: ChildProc::replay(wire),
        pending_brief: Mutex::new(None),
        interrupting: Arc::default(),
        expecting: Arc::default(),
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
        handle: Handle::Antigravity(session),
        inbound: Box::new(move |frame| {
            for event in reader.borrow_mut().translate(frame) {
                sink(event);
            }
        }),
        eof: Box::new(move || closing.closed(&mut closer.borrow_mut(), &eof_sink)),
    }
}
