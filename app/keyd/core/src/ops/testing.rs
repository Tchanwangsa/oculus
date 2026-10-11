//! What the ops' tests share: a state over a scratch dir and a fixed master
//! key, and a call as a given role.

use serde_json::Value;

use super::{OpError, State};
use crate::platform::{Caller, Role};
use crate::test_support::{Scratch, BUILD};
use crate::vault::{MasterKey, NoLegacy, StaticKey};

pub(super) fn as_role(role: Role) -> Caller {
    Caller {
        role,
        ..Caller::default()
    }
}

pub(super) fn cli() -> Caller {
    as_role(Role::Cli)
}

pub(super) fn key() -> MasterKey {
    MasterKey::from_bytes([9; 32])
}

pub(super) fn state_in(dir: &Scratch) -> State {
    State::new(
        BUILD,
        dir.0.clone(),
        Box::new(StaticKey(key())),
        Box::new(NoLegacy),
    )
}

/// The reply's header, for ops that answer without a body.
pub(super) fn call(state: &State, op: &str, req: Value) -> Result<Value, OpError> {
    state.dispatch(&cli(), op, &req, b"").map(|r| {
        assert!(r.body.is_empty());
        r.header
    })
}

/// An op with no arguments, as the CLI, replying with its header.
pub(super) fn op_as(state: &State, op: &str) -> Value {
    call(state, op, serde_json::json!({"op": op})).unwrap()
}
