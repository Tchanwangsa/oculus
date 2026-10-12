use super::*;

fn app() -> Caller {
    as_role(Role::App)
}

fn get_as(state: &State, caller: &Caller, secret: &str) -> Result<Value, OpError> {
    state
        .dispatch(caller, "get", &json!({"secret": secret}), b"")
        .map(|r| r.header)
}

#[test]
fn the_app_reads_back_the_spotify_credentials() {
    let dir = Scratch::new("get-app");
    let state = state_in(&dir);
    assert_eq!(
        get_as(&state, &app(), names::MUSIC_SPOTIFY).unwrap()["value"],
        Value::Null
    );
    call(
        &state,
        "store",
        json!({"secret": "music.spotify", "value": "{\"blob\":1}"}),
    )
    .unwrap();
    assert_eq!(
        get_as(&state, &app(), names::MUSIC_SPOTIFY).unwrap()["value"],
        "{\"blob\":1}"
    );
    assert_eq!(
        call(&state, "delete", json!({"secret": "music.spotify"})).unwrap()["existed"],
        true
    );
    assert_eq!(
        get_as(&state, &app(), names::MUSIC_SPOTIFY).unwrap()["value"],
        Value::Null
    );
}

#[test]
fn the_cli_and_an_unknown_caller_are_refused_before_the_vault_is_read() {
    let dir = Scratch::new("get-roles");
    let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let state = State::new(
        BUILD,
        dir.0.clone(),
        Box::new(Counting(reads.clone())),
        Box::new(NoLegacy),
    );
    for role in [Role::Cli, Role::Unknown] {
        let err = get_as(&state, &as_role(role), names::MUSIC_SPOTIFY).unwrap_err();
        assert_eq!(err.kind, "caller", "{role:?}");
    }
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn the_app_cannot_read_back_any_other_name() {
    let dir = Scratch::new("get-other");
    let state = state_in(&dir);
    call(
        &state,
        "store",
        json!({"secret": "voyage", "value": "pa-SECRET"}),
    )
    .unwrap();
    for name in names::KNOWN.iter().filter(|n| **n != names::MUSIC_SPOTIFY) {
        let err = get_as(&state, &app(), name).unwrap_err();
        assert_eq!(err.kind, "request", "{name}");
        assert!(!err.detail.contains("pa-SECRET"));
    }
    let err = get_as(&state, &app(), "keyd.imported.voyage").unwrap_err();
    assert_eq!(err.kind, "request");
}

#[test]
fn a_sign_out_keeps_the_spotify_credentials() {
    let dir = Scratch::new("get-sign-out");
    let state = state_in(&dir);
    call(
        &state,
        "store",
        json!({"secret": "music.spotify", "value": "blob"}),
    )
    .unwrap();
    call(&state, "sign_out", json!({})).unwrap();
    call(&state, "session_clear", json!({})).unwrap();
    assert_eq!(
        get_as(&state, &app(), names::MUSIC_SPOTIFY).unwrap()["value"],
        "blob"
    );
}
