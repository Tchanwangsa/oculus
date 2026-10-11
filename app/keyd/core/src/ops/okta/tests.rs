use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::okta::outcome_from_wire;
use crate::platform::Role;
use crate::test_support::okta_fake::{
    answer, code_from_t0, script, COOKIE, PASSWORD, SEED, T0, USERNAME,
};
use crate::test_support::{FakeOrigin, OldItems, Reads, Scratch, TestClock, BUILD};
use crate::vault::{KeyError, MasterKey, NoLegacy, StaticKey};

fn key() -> MasterKey {
    MasterKey::from_bytes([7; 32])
}

fn as_role(role: Role) -> Caller {
    Caller {
        role,
        ..Caller::default()
    }
}

fn cli() -> Caller {
    as_role(Role::Cli)
}

fn fake() -> FakeOrigin {
    FakeOrigin::start(script(code_from_t0))
}

/// A keyd whose sign-in clock reads `clock`.
fn state_on(
    dir: &Scratch,
    fake: Option<&FakeOrigin>,
    legacy: Box<dyn crate::vault::LegacySource>,
    clock: &TestClock,
) -> State {
    let state = State::new(BUILD, dir.0.clone(), Box::new(StaticKey(key())), legacy)
        .with_clock(clock.clock());
    let Some(fake) = fake else { return state };
    let port = fake.origin.rsplit(':').next().unwrap();
    state
        .with_origins(
            Some(&format!("http://127.0.0.1:{port}")),
            Some(&format!("http://localhost:{port}")),
        )
        .unwrap()
}

fn state_with(
    dir: &Scratch,
    fake: Option<&FakeOrigin>,
    legacy: Box<dyn crate::vault::LegacySource>,
) -> State {
    state_on(dir, fake, legacy, &TestClock::at(T0))
}

fn state(dir: &Scratch, fake: &FakeOrigin) -> State {
    state_with(dir, Some(fake), Box::new(NoLegacy))
}

/// A keyd on a clock the test moves.
fn clocked(dir: &Scratch, fake: &FakeOrigin) -> (State, TestClock) {
    let clock = TestClock::at(T0);
    (state_on(dir, Some(fake), Box::new(NoLegacy), &clock), clock)
}

fn op(state: &State, caller: &Caller, name: &str, req: Value) -> Result<Value, OpError> {
    state.dispatch(caller, name, &req, b"").map(|r| r.header)
}

fn save(state: &State, username: &str, password: &str, seed: &str) -> Result<Value, OpError> {
    op(
        state,
        &cli(),
        "okta_save",
        json!({"username": username, "password": password, "totp_secret": seed}),
    )
}

fn save_good(state: &State) {
    save(state, USERNAME, PASSWORD, SEED).unwrap();
}

fn ensure_as(state: &State, role: Role, trigger: &str) -> Result<Result<(), LoginError>, OpError> {
    let header = op(
        state,
        &as_role(role),
        "ensure_signed_in",
        json!({"trigger": trigger}),
    )?;
    Ok(outcome_from_wire(&header).unwrap_or_else(|| panic!("{header}")))
}

fn ensure(state: &State, trigger: &str) -> Result<Result<(), LoginError>, OpError> {
    ensure_as(state, Role::Cli, trigger)
}

fn status(state: &State) -> OktaStatus {
    OktaStatus::from_wire(&op(state, &cli(), "okta_status", json!({})).unwrap()).unwrap()
}

fn vault_of(dir: &Scratch) -> Vault {
    Vault::new(crate::paths::vault(&dir.0), key())
}

fn stored(dir: &Scratch, name: &str) -> Option<String> {
    vault_of(dir).get(name).unwrap()
}

fn marked(dir: &Scratch, name: &str) -> bool {
    vault_of(dir).has(&names::imported(name)).unwrap()
}

fn requests(fake: &FakeOrigin) -> usize {
    fake.hits().len()
}

mod ensure_signed_in;
mod import_on_first_use;
mod okta_resume;
mod okta_save;
mod origins;
mod single_flight;
mod status_and_forget;
mod who_may_ask;
