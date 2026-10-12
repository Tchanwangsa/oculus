use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::vault::Vault;
use crate::webcrypto::WebCryptoKey;

const DEV: &str = "com.apple.WebKit.WebCrypto.master+app";
const BUNDLED: &str = "com.apple.WebKit.WebCrypto.master+com.tchan.oculus";
/// What WebKit's items hold: the base64 of 16 bytes, as text.
const WEBKIT_DEV: &str = "Zm9vYmFyZm9vYmFyZm9vYg==";
const WEBKIT_BUNDLED: &str = "/////////////////////w==";

fn app_named(application: &str) -> Caller {
    Caller {
        role: Role::App,
        application: Some(application.to_string()),
        ..Caller::default()
    }
}

fn state_over(dir: &Scratch, items: OldItems) -> State {
    State::new(
        BUILD,
        dir.0.clone(),
        Box::new(StaticKey(key())),
        Box::new(items),
    )
}

fn no_items() -> OldItems {
    OldItems::new(vec![], Reads::default())
}

fn item(text: &str) -> Result<Option<Vec<u8>>, KeyError> {
    Ok(Some(text.as_bytes().to_vec()))
}

fn ask(state: &State, caller: &Caller, app: &str) -> Result<Reply, OpError> {
    state.dispatch(caller, "webcrypto_key", &json!({"app": app}), b"")
}

/// The key and where it came from, as the dev app.
fn dev_key(state: &State) -> (String, String) {
    let reply = ask(state, &app_named("app"), "app").unwrap();
    (
        reply.header["key"].as_str().unwrap().to_string(),
        reply.header["source"].as_str().unwrap().to_string(),
    )
}

fn vault_of(dir: &Scratch) -> Vault {
    Vault::new(crate::paths::vault(&dir.0), key())
}

#[test]
fn a_key_in_the_vault_is_served_without_reading_the_keychain() {
    let dir = Scratch::new("webcrypto-vault");
    vault_of(&dir)
        .store(names::WEBCRYPTO_DEV, "AAAAAAAAAAAAAAAAAAAAAA==")
        .unwrap();
    let items = no_items().webkit_item(DEV, item(WEBKIT_DEV));
    let reads = items.webkit_reads();
    let state = state_over(&dir, items);
    assert_eq!(
        dev_key(&state),
        ("AAAAAAAAAAAAAAAAAAAAAA==".into(), "vault".into())
    );
    assert_eq!(reads.load(Ordering::SeqCst), 0);
}

#[test]
fn webkits_item_is_copied_in_once_and_left_in_the_keychain() {
    let dir = Scratch::new("webcrypto-import");
    let items = no_items().webkit_item(DEV, item(WEBKIT_DEV));
    let (reads, removed) = (items.webkit_reads(), items.removed());
    let state = state_over(&dir, items);

    let reply = ask(&state, &app_named("app"), "app").unwrap();
    assert_eq!(
        reply.header,
        json!({"key": WEBKIT_DEV, "source": "keychain"})
    );
    assert_eq!(reply.note.as_deref(), Some("app=app source=keychain"));
    assert_eq!(
        vault_of(&dir).get(names::WEBCRYPTO_DEV).unwrap().as_deref(),
        Some(WEBKIT_DEV)
    );
    assert_eq!(dev_key(&state), (WEBKIT_DEV.into(), "vault".into()));
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert!(removed.lock().unwrap().is_empty());
}

#[test]
fn a_new_key_is_made_only_when_webkit_has_no_item() {
    let dir = Scratch::new("webcrypto-mint");
    let state = state_over(&dir, no_items());
    let (minted, source) = dev_key(&state);
    assert_eq!(source, "minted");
    assert!(WebCryptoKey::from_text(&minted).is_some());
    assert_ne!(minted, WEBKIT_DEV);
    assert_eq!(dev_key(&state), (minted.clone(), "vault".into()));

    let fresh = state_over(&dir, no_items().webkit_item(DEV, item(WEBKIT_DEV)));
    assert_eq!(dev_key(&fresh), (minted, "vault".into()), "the vault wins");
}

#[test]
fn a_refused_or_failed_read_makes_no_key_and_stores_nothing() {
    for (n, error) in [
        KeyError::Refused("reading x: OSStatus -128".into()),
        KeyError::Refused("reading x: OSStatus -25308".into()),
        KeyError::Platform("reading x: OSStatus -25291".into()),
    ]
    .into_iter()
    .enumerate()
    {
        let dir = Scratch::new(&format!("webcrypto-refused-{n}"));
        let items = no_items().webkit_item(DEV, Err(error.clone()));
        let reads = items.webkit_reads();
        let state = state_over(&dir, items);
        for _ in 0..2 {
            let err = ask(&state, &app_named("app"), "app").unwrap_err();
            assert_eq!(err.kind, "keychain", "{error:?}");
        }
        assert_eq!(reads.load(Ordering::SeqCst), 2, "a refusal is not cached");
        assert_eq!(vault_of(&dir).get(names::WEBCRYPTO_DEV).unwrap(), None);
    }
}

#[test]
fn an_item_that_is_not_16_bytes_of_base64_is_never_copied_or_replaced() {
    let long = "Zm9vYmFyZm9vYmFyZm9vYmFyZm9vYmFyZm9vYmFyZm9vYmFy";
    for (n, data) in [
        b"not base64 at all".to_vec(),
        long.as_bytes().to_vec(),
        b"Zm9vYmFyZm9vYmFyZm9vYg==\n".to_vec(),
        vec![0xff; 24],
        Vec::new(),
    ]
    .into_iter()
    .enumerate()
    {
        let dir = Scratch::new(&format!("webcrypto-bad-{n}"));
        let state = state_over(&dir, no_items().webkit_item(DEV, Ok(Some(data.clone()))));
        let err = ask(&state, &app_named("app"), "app").unwrap_err();
        assert_eq!(err.kind, "keychain");
        let text = String::from_utf8_lossy(&data);
        assert!(
            text.is_empty() || !err.detail.contains(text.trim()),
            "{}",
            err.detail
        );
        assert_eq!(vault_of(&dir).get(names::WEBCRYPTO_DEV).unwrap(), None);
    }
}

#[test]
fn a_damaged_vault_entry_is_never_served_or_replaced() {
    let dir = Scratch::new("webcrypto-damaged");
    vault_of(&dir)
        .store(names::WEBCRYPTO_DEV, "Zm9vYmFy")
        .unwrap();
    let items = no_items().webkit_item(DEV, item(WEBKIT_DEV));
    let reads = items.webkit_reads();
    let state = state_over(&dir, items);
    let err = ask(&state, &app_named("app"), "app").unwrap_err();
    assert_eq!(err.kind, "vault");
    assert!(!err.detail.contains("Zm9vYmFy"));
    assert_eq!(reads.load(Ordering::SeqCst), 0);
    assert_eq!(
        vault_of(&dir).get(names::WEBCRYPTO_DEV).unwrap().as_deref(),
        Some("Zm9vYmFy")
    );
}

#[test]
fn only_the_app_is_served_and_the_vault_is_not_opened_for_anyone_else() {
    let dir = Scratch::new("webcrypto-roles");
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let state = State::new(
        BUILD,
        dir.0.clone(),
        Box::new(Counting(reads.clone())),
        Box::new(no_items().webkit_item(DEV, item(WEBKIT_DEV))),
    );
    for role in [Role::Cli, Role::Unknown] {
        let caller = Caller {
            application: Some("app".into()),
            ..as_role(role)
        };
        let err = ask(&state, &caller, "app").unwrap_err();
        assert_eq!(err.kind, "caller", "{role:?}");
    }
    let err = ask(&state, &as_role(Role::App), "app").unwrap_err();
    assert_eq!(err.kind, "caller", "an app the peer check could not name");
    assert_eq!(reads.load(Ordering::SeqCst), 0);
}

#[test]
fn neither_build_can_have_the_others_key() {
    let dir = Scratch::new("webcrypto-builds");
    let items = no_items()
        .webkit_item(DEV, item(WEBKIT_DEV))
        .webkit_item(BUNDLED, item(WEBKIT_BUNDLED));
    let reads = items.webkit_reads();
    let state = state_over(&dir, items);
    let (dev, bundle) = (app_named("app"), app_named("com.tchan.oculus"));

    for (caller, app) in [(&dev, "com.tchan.oculus"), (&bundle, "app")] {
        assert_eq!(ask(&state, caller, app).unwrap_err().kind, "caller");
    }
    assert_eq!(reads.load(Ordering::SeqCst), 0);

    assert_eq!(ask(&state, &dev, "app").unwrap().header["key"], WEBKIT_DEV);
    assert_eq!(
        ask(&state, &bundle, "com.tchan.oculus").unwrap().header["key"],
        WEBKIT_BUNDLED
    );
    let vault = vault_of(&dir);
    assert_eq!(
        vault.get(names::WEBCRYPTO_DEV).unwrap().as_deref(),
        Some(WEBKIT_DEV)
    );
    assert_eq!(
        vault.get(names::WEBCRYPTO_BUNDLED).unwrap().as_deref(),
        Some(WEBKIT_BUNDLED)
    );

    // A build with no item gets a key of its own, not the other's.
    let dir = Scratch::new("webcrypto-builds-mint");
    let state = state_over(&dir, no_items().webkit_item(DEV, item(WEBKIT_DEV)));
    let minted = ask(&state, &bundle, "com.tchan.oculus").unwrap().header;
    assert_eq!(minted["source"], "minted");
    assert_ne!(minted["key"], WEBKIT_DEV);
    assert_eq!(dev_key(&state), (WEBKIT_DEV.into(), "keychain".into()));
}

#[test]
fn an_unknown_or_missing_application_is_a_bad_request() {
    let dir = Scratch::new("webcrypto-request");
    let state = state_over(&dir, no_items());
    for app in ["", "App", "Oculus", "com.tchan.oculus.keyd", "safari"] {
        let err = ask(&state, &app_named(app), app).unwrap_err();
        assert_eq!(err.kind, "request", "{app:?}");
    }
    let err = state
        .dispatch(&app_named("app"), "webcrypto_key", &json!({}), b"")
        .unwrap_err();
    assert_eq!(err.kind, "request");
}

#[test]
fn racing_first_requests_read_the_item_once_and_agree() {
    for webkit in [Some(WEBKIT_DEV), None] {
        let dir = Scratch::new("webcrypto-race");
        let mut items = no_items().slow_webkit(Duration::from_millis(30));
        if let Some(text) = webkit {
            items = items.webkit_item(DEV, item(text));
        }
        let reads = items.webkit_reads();
        let state = Arc::new(state_over(&dir, items));
        let threads: Vec<_> = (0..6)
            .map(|_| {
                let s = state.clone();
                std::thread::spawn(move || dev_key(&s).0)
            })
            .collect();
        let keys: Vec<String> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        assert!(keys.iter().all(|k| *k == keys[0]), "{webkit:?}");
        if let Some(text) = webkit {
            assert_eq!(keys[0], text);
        }
        assert_eq!(reads.load(Ordering::SeqCst), 1, "{webkit:?}");
        assert_eq!(
            vault_of(&dir).get(names::WEBCRYPTO_DEV).unwrap().as_deref(),
            Some(keys[0].as_str())
        );
    }
}

#[test]
fn the_generic_ops_never_read_or_write_webkits_keys() {
    let dir = Scratch::new("webcrypto-generic");
    let state = state_over(&dir, no_items());
    let (key, _) = dev_key(&state);
    for name in names::WEBCRYPTO_NAMES {
        for (op, req) in [
            (
                "store",
                json!({"secret": name, "value": "AAAAAAAAAAAAAAAAAAAAAA=="}),
            ),
            ("delete", json!({"secret": name})),
        ] {
            let err = call(&state, op, req).unwrap_err();
            assert_eq!(err.kind, "request", "{op} {name}");
            assert!(err.detail.contains("webcrypto_key"));
        }
        let err = state
            .dispatch(&app_named("app"), "get", &json!({"secret": name}), b"")
            .unwrap_err();
        assert_eq!(err.kind, "request", "get {name}");
        assert!(!err.detail.contains(&key));
    }
    assert_eq!(
        call(&state, "has", json!({"secret": names::WEBCRYPTO_DEV})).unwrap()["has"],
        true
    );
    call(&state, "sign_out", json!({})).unwrap();
    assert_eq!(
        dev_key(&state),
        (key, "vault".into()),
        "a sign-out keeps it"
    );
}
