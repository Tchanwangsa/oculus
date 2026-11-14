use std::sync::Arc;

use serde_json::Value;

use crate::harness::manager::Handle;
use crate::harness::{Harness, OneSink, Provider, SendOptions, Sinks};

use super::{guard::assert_scratch, rig::Rig, scenario::Scenario};

pub(in crate::harness) struct Bridge {
    pub handle: Handle,
    pub inbound: Box<dyn FnMut(&Value)>,
    /// The transport closing, through the bridge's production EOF path.
    pub eof: Box<dyn FnMut()>,
}

pub(super) struct Replay {
    pub envelopes: Vec<Value>,
    pub snapshots: Vec<Vec<Value>>,
    pub items: Vec<Value>,
    pub reopened: Vec<Value>,
    pub outbound: Vec<Value>,
    pub busy: bool,
    pub schema: Vec<String>,
    /// Run, attempt, binding and receipt rows for the thread (`snapshot::ledger`).
    pub ledger: Value,
    pub repeat_envelope: bool,
    /// The app restarted after the stream (startup reconciliation ran).
    pub restarted: bool,
    pub load_race: bool,
    /// Continuations the consumer dispatched (`Scenario::continuation`).
    pub continuations: Vec<SendOptions>,
}

/// A whole recording through the [`Rig`]: its request, every frame, then the
/// transport closing (or, for a `restart` scenario, the app starting again).
pub(super) fn run(scenario: Scenario) -> Replay {
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.resume();
    rig.close();
    rig.finish()
}

#[test]
#[should_panic(expected = "unrecorded provider spawn in replay")]
fn replay_manager_never_spawns_an_unrecorded_handle() {
    let scratch = super::guard::scratch("turn-no-spawn");
    let harness = Harness::replay(assert_scratch(&scratch, &scratch));
    let sinks: Arc<dyn Sinks> = Arc::new(OneSink(Arc::new(|_| {})));
    let _ = harness.send(
        1,
        Provider::Claude,
        None,
        &SendOptions::default(),
        "unexpected",
        &sinks,
    );
}
