use super::testing::{as_role, call, cli, key, state_in};
use super::*;
use crate::test_support::{Answer, FakeOrigin, OldItems, Reads, Scratch, BUILD};
use crate::vault::{NoLegacy, StaticKey};

mod forward_op;
mod import_on_first_use;

struct Counting(std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl KeySource for Counting {
    fn get_or_create(&self) -> Result<MasterKey, KeyError> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(20));
        Ok(key())
    }
}

struct Refusing;

impl KeySource for Refusing {
    fn get_or_create(&self) -> Result<MasterKey, KeyError> {
        Err(KeyError::Refused("user cancelled".into()))
    }
}

#[test]
fn ping_never_reads_the_master_key() {
    let dir = Scratch::new("ping");
    let state = State::new(BUILD, dir.0.clone(), Box::new(Refusing), Box::new(NoLegacy));
    let reply = call(&state, "ping", json!({"op": "ping"})).unwrap();
    assert_eq!(reply["source_hash"], BUILD.source_hash);
    assert_eq!(reply["source_hash"].as_str().unwrap().len(), 64);
    assert_eq!(reply["pid"], std::process::id());
    assert_eq!(reply["version"], BUILD.version);
}

#[test]
fn a_refused_master_key_is_a_keychain_error_and_is_retried() {
    let dir = Scratch::new("refused");
    let state = State::new(BUILD, dir.0.clone(), Box::new(Refusing), Box::new(NoLegacy));
    let err = state
        .dispatch(&cli(), "has", &json!({"secret": "voyage"}), b"")
        .unwrap_err();
    assert_eq!(err.kind, "keychain");
    assert_eq!(err.to_json()["error"], "keychain");
    assert!(
        state.master.lock().unwrap().is_none(),
        "a refusal is not cached"
    );
}

#[test]
fn the_master_key_is_read_once_across_racing_connections() {
    let dir = Scratch::new("single-flight");
    let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let state = std::sync::Arc::new(State::new(
        BUILD,
        dir.0.clone(),
        Box::new(Counting(reads.clone())),
        Box::new(NoLegacy),
    ));
    let threads: Vec<_> = (0..6)
        .map(|_| {
            let s = state.clone();
            std::thread::spawn(move || {
                s.dispatch(&cli(), "has", &json!({"secret": "groq"}), b"")
                    .unwrap()
                    .header
            })
        })
        .collect();
    for t in threads {
        assert_eq!(t.join().unwrap()["has"], false);
    }
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn store_has_delete_and_never_echo_a_value() {
    let dir = Scratch::new("ops");
    let state = state_in(&dir);
    let stored = call(
        &state,
        "store",
        json!({"secret": "voyage", "value": "pa-SECRET"}),
    )
    .unwrap();
    assert!(!stored.to_string().contains("pa-SECRET"));
    assert_eq!(
        call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
        true
    );
    assert_eq!(
        call(&state, "delete", json!({"secret": "voyage"})).unwrap()["existed"],
        true
    );
    assert_eq!(
        call(&state, "delete", json!({"secret": "voyage"})).unwrap()["existed"],
        false
    );
    assert_eq!(
        call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
        false
    );
}

/// One well-formed request for every op there is.
fn every_op() -> Vec<(&'static str, Value)> {
    vec![
        ("has", json!({"secret": "voyage"})),
        ("store", json!({"secret": "voyage", "value": "pa-x"})),
        ("delete", json!({"secret": "voyage"})),
        (
            "forward",
            json!({"secret": "voyage", "method": "POST", "path": "/v1/x"}),
        ),
        (
            "okta_save",
            json!({"username": "u", "password": "p", "totp_secret": "GEZD"}),
        ),
        ("okta_forget", json!({})),
        ("okta_status", json!({})),
        ("ensure_signed_in", json!({"trigger": "manual"})),
        ("okta_resume", json!({})),
        ("session_get", json!({})),
        (
            "session_put",
            json!({"kind": "canvas", "value": "canvas_session=x"}),
        ),
        ("session_clear", json!({})),
        ("session_status", json!({})),
        ("session_mark", json!({"authenticated": true})),
        ("sign_out", json!({})),
        ("a_future_op", json!({})),
    ]
}

#[test]
fn every_op_but_ping_refuses_a_caller_of_no_role_before_anything_runs() {
    let dir = Scratch::new("no-role");
    let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let state = State::new(
        BUILD,
        dir.0.clone(),
        Box::new(Counting(reads.clone())),
        Box::new(NoLegacy),
    );
    let nobody = as_role(Role::Unknown);
    for (op, req) in every_op() {
        let err = state.dispatch(&nobody, op, &req, b"").unwrap_err();
        assert_eq!(err.kind, "caller", "{op}");
        assert!(!err.detail.contains(op), "{op}: {}", err.detail);
    }
    // Not even a body is read for it.
    assert_eq!(
        state
            .dispatch(&nobody, "has", &json!({"secret": "voyage"}), b"x")
            .unwrap_err()
            .kind,
        "caller"
    );
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(!crate::paths::vault(&dir.0).exists());
    assert!(!crate::paths::sign_in_record(&dir.0).exists());

    // ping is the one op that answers everyone, for diagnostics.
    let ping = state.dispatch(&nobody, "ping", &json!({}), b"").unwrap();
    assert_eq!(ping.header["version"], BUILD.version);
}

#[test]
fn the_app_and_the_cli_get_past_the_role_gate_for_every_op() {
    for role in [Role::App, Role::Cli] {
        let dir = Scratch::new("with-role");
        let state = state_in(&dir);
        for (op, req) in every_op() {
            // Whatever else the op says, it is not a refusal of the caller,
            // except that lifting a lockout pause and reading a cookie
            // back are the app's alone.
            let refused = matches!(
                state.dispatch(&as_role(role), op, &req, b""),
                Err(OpError { kind: "caller", .. })
            );
            assert_eq!(
                refused,
                role == Role::Cli && matches!(op, "okta_resume" | "session_get"),
                "{role:?} {op}"
            );
        }
    }
}

#[test]
fn malformed_requests_are_request_errors() {
    let dir = Scratch::new("bad");
    let state = state_in(&dir);
    for (op, req, body) in [
        ("get", json!({"secret": "voyage"}), &b""[..]),
        ("has", json!({}), b""),
        ("has", json!({"secret": "session.nope"}), b""),
        ("store", json!({"secret": "voyage"}), b""),
        ("store", json!({"secret": "voyage", "value": ""}), b""),
        ("store", json!({"secret": "voyage", "value": 5}), b""),
        ("ping", json!({}), b"x"),
    ] {
        let err = state.dispatch(&cli(), op, &req, body).unwrap_err();
        assert_eq!(err.kind, "request", "{op} {req}");
    }
    assert!(!crate::paths::vault(&dir.0).exists(), "nothing was written");
}
