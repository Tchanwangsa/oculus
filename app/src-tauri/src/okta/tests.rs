use super::credentials::{clear_credentials_in, credential_status_in, store_credentials_in};
use super::login::{resume_in, sign_in_in};
use super::*;
use crate::credentials::KeydError;
use crate::test_support::{FakeKeyd, Scratch};
use keyd_core::okta::outcome_to_wire;
use serde_json::{json, Value};

const TRIGGERS: [(Trigger, &str); 4] = [
    (Trigger::Manual, "manual"),
    (Trigger::Startup, "startup"),
    (Trigger::Browser, "browser"),
    (Trigger::Forward, "forward"),
];

/// Every `LoginError` variant, as a sign-in can end in it.
fn every_login_error() -> Vec<LoginError> {
    vec![
        LoginError::NotConfigured,
        LoginError::SignedOut,
        LoginError::UnreadableCredentials("OSStatus -128".into()),
        LoginError::BadPassword("Password is incorrect".into()),
        LoginError::BadTotp("Invalid code".into()),
        LoginError::UnsupportedFactor(vec!["Okta Verify".into(), "Security Key".into()]),
        LoginError::Locked("Too many attempts".into()),
        LoginError::Network("dns error".into()),
        LoginError::Unexpected("identify, enroll-authenticator".into()),
        LoginError::Broker("oculus-keyd refused this program (no role)".into()),
        LoginError::Waiting(125),
        LoginError::Paused("Okta rejected the password: x".into()),
    ]
}

fn entries(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn keyd_error(kind: &str, detail: &str) -> (Value, Vec<u8>) {
    (json!({"error": kind, "detail": detail}), vec![])
}

#[test]
fn the_status_is_keyds_and_keeps_the_json_shape_settings_reads() {
    let dir = Scratch::new("okta-status");
    let keyd = FakeKeyd::start(&dir, |_, _| {
        (
            json!({"username": "s1234567", "has_password": true, "has_totp": false}),
            vec![],
        )
    });
    let status = credential_status_in(&Credentialed::at(&dir), || panic!("keychain")).unwrap();
    assert_eq!(
        serde_json::to_value(status).unwrap(),
        json!({"username": "s1234567", "has_password": true, "has_totp": false})
    );
    assert_eq!(keyd.requests()[0].0, json!({"op": "okta_status"}));
    assert_eq!(keyd.ops(), ["okta_status"]);
}

#[test]
fn an_unreadable_status_names_the_keychain_and_any_other_refusal_says_it_could_not_check() {
    for (kind, detail) in [
        ("keychain", "OSStatus -128"),
        ("vault", "vault.bin is damaged"),
        ("caller", "no role"),
    ] {
        let dir = Scratch::new("okta-status-err");
        let keyd = FakeKeyd::start(&dir, move |_, _| keyd_error(kind, detail));
        let err = credential_status_in(&Credentialed::at(&dir), || panic!("keychain")).unwrap_err();
        assert!(err.contains(detail), "{err}");
        if kind == "keychain" {
            assert_eq!(
                err,
                LoginError::UnreadableCredentials(detail.into()).to_string()
            );
        } else {
            assert!(
                err.starts_with("Could not check the saved sign-in"),
                "{err}"
            );
        }
        assert_eq!(keyd.requests().len(), 1);
    }
}

#[test]
fn a_save_sends_the_values_as_typed_and_keyd_does_the_validating() {
    let dir = Scratch::new("okta-save");
    let keyd = FakeKeyd::start(&dir, |_, _| (json!({"saved": true}), vec![]));
    store_credentials_in(
        &Credentialed::at(&dir),
        " s1234567 ",
        "pw",
        "GEZD GEZD",
        || panic!("keychain"),
    )
    .unwrap();
    assert_eq!(
        keyd.requests()[0].0,
        json!({
            "op": "okta_save",
            "username": " s1234567 ",
            "password": "pw",
            "totp_secret": "GEZD GEZD",
        })
    );
    assert_eq!(keyd.requests().len(), 1);
}

#[test]
fn keyds_validation_message_reaches_the_caller_as_written() {
    for message in [
        "Username is required.",
        "Password is required.",
        "That does not look like a TOTP setup key: it may only contain the letters A–Z and the digits 2–7.",
    ] {
        let dir = Scratch::new("okta-save-invalid");
        let keyd = FakeKeyd::start(&dir, move |_, _| keyd_error("request", message));
        let err =
            store_credentials_in(&Credentialed::at(&dir), "", "", "1", || panic!("keychain"))
                .unwrap_err();
        assert_eq!(err, message);
        assert_eq!(keyd.requests().len(), 1, "the app sent it without a check");
    }
}

#[test]
fn forget_asks_keyd_once_and_a_refusal_surfaces() {
    let dir = Scratch::new("okta-forget");
    let keyd = FakeKeyd::start(&dir, |_, _| (json!({"existed": true}), vec![]));
    clear_credentials_in(&Credentialed::at(&dir), || panic!("keychain")).unwrap();
    assert_eq!(keyd.requests()[0].0, json!({"op": "okta_forget"}));

    let dir = Scratch::new("okta-forget-refused");
    let keyd = FakeKeyd::start(&dir, |_, _| keyd_error("vault", "vault.bin is damaged"));
    let err = clear_credentials_in(&Credentialed::at(&dir), || panic!("keychain")).unwrap_err();
    assert!(err.contains("damaged"), "{err}");
    assert_eq!(keyd.requests().len(), 1);
}

#[test]
fn a_save_or_forget_keyd_refuses_never_reaches_the_keychain() {
    for kind in ["keychain", "caller", "upstream", "vault"] {
        let dir = Scratch::new("okta-write-refused");
        let keyd = FakeKeyd::start(&dir, move |_, _| keyd_error(kind, "refused"));
        let broker = Credentialed::at(&dir);
        let err =
            store_credentials_in(&broker, "u", "p", "GEZD", || panic!("keychain")).unwrap_err();
        assert!(err.contains("refused"), "{kind}: {err}");
        clear_credentials_in(&broker, || panic!("keychain")).unwrap_err();
        assert_eq!(keyd.ops(), ["okta_save", "okta_forget"], "{kind}");
    }
}

#[test]
fn the_sign_in_sends_the_trigger_and_returns_keyds_outcome() {
    let dir = Scratch::new("okta-sign-in");
    let keyd = FakeKeyd::start(&dir, |_, _| (outcome_to_wire(&Ok(String::new())), vec![]));
    let broker = Credentialed::at(&dir);
    for (trigger, _) in TRIGGERS {
        sign_in_in(&broker, trigger).unwrap();
    }
    let sent: Vec<Value> = keyd.requests().into_iter().map(|(h, _)| h).collect();
    let want: Vec<Value> = TRIGGERS
        .iter()
        .map(|(_, name)| json!({"op": "ensure_signed_in", "trigger": name}))
        .collect();
    assert_eq!(sent, want);
}

#[test]
fn every_login_error_keyd_reports_comes_back_the_same() {
    for error in every_login_error() {
        let dir = Scratch::new("okta-outcome");
        let wire = outcome_to_wire(&Err(error.clone()));
        let keyd = FakeKeyd::start(&dir, move |_, _| (wire.clone(), vec![]));
        let got = sign_in_in(&Credentialed::at(&dir), Trigger::Manual).unwrap_err();
        assert_eq!(got, error);
        assert_eq!(got.to_string(), error.to_string());
        assert_eq!(keyd.requests().len(), 1);
    }
}

#[test]
fn a_keyd_failure_is_surfaced_and_never_becomes_a_second_attempt() {
    for (kind, want) in [
        (
            "keychain",
            LoginError::UnreadableCredentials("OSStatus -128".into()),
        ),
        (
            "caller",
            LoginError::Broker(KeydError::Caller("OSStatus -128".into()).to_string()),
        ),
        (
            "vault",
            LoginError::Broker(KeydError::Vault("OSStatus -128".into()).to_string()),
        ),
        (
            "upstream",
            LoginError::Broker(KeydError::Upstream("OSStatus -128".into()).to_string()),
        ),
        (
            "teapot",
            LoginError::Broker(KeydError::Broken("teapot: OSStatus -128".into()).to_string()),
        ),
    ] {
        let dir = Scratch::new("okta-no-second-route");
        let keyd = FakeKeyd::start(&dir, move |_, _| keyd_error(kind, "OSStatus -128"));
        let before = entries(&dir);
        let got = sign_in_in(&Credentialed::at(&dir), Trigger::Startup).unwrap_err();
        assert_eq!(got, want, "{kind}");
        assert!(!got.to_string().contains("Unexpected"), "{kind}: {got}");
        assert_eq!(keyd.requests().len(), 1, "{kind}: exactly one request");
        assert_eq!(entries(&dir), before, "{kind}: nothing written");
    }
}

#[test]
fn a_resume_goes_to_keyd_and_only_an_absent_keyd_resets_the_record_here() {
    let dir = Scratch::new("okta-resume");
    let keyd = FakeKeyd::start(&dir, |_, _| (json!({"resumed": true}), vec![]));
    resume_in(&Credentialed::at(&dir), || panic!("in-process"));
    assert_eq!(keyd.requests()[0].0, json!({"op": "okta_resume"}));
    assert_eq!(keyd.requests().len(), 1);

    for kind in ["caller", "record", "vault", "keychain"] {
        let dir = Scratch::new("okta-resume-refused");
        let keyd = FakeKeyd::start(&dir, move |_, _| keyd_error(kind, "refused"));
        resume_in(&Credentialed::at(&dir), || panic!("in-process {kind}"));
        assert_eq!(keyd.requests().len(), 1, "{kind}");
        assert!(
            !keyd_core::paths::sign_in_record(&dir).exists(),
            "{kind}: the app wrote the record"
        );
    }

    let dir = Scratch::new("okta-resume-absent");
    let ran = std::cell::Cell::new(false);
    resume_in(&Credentialed::at(&dir), || {
        ran.set(true);
        Ok(())
    });
    assert!(ran.get());
}

#[test]
fn only_an_absent_keyd_runs_the_credential_fallbacks_and_never_a_sign_in() {
    let dir = Scratch::new("okta-absent");
    let broker = Credentialed::at(&dir);
    let status = credential_status_in(&broker, || {
        Ok(CredentialStatus {
            username: Some("fallback".into()),
            has_password: false,
            has_totp: false,
        })
    })
    .unwrap();
    assert_eq!(status.username.as_deref(), Some("fallback"));
    let ran = std::cell::Cell::new(0);
    store_credentials_in(&broker, "u", "p", "GEZD", || {
        ran.set(ran.get() + 1);
        Ok(())
    })
    .unwrap();
    clear_credentials_in(&broker, || {
        ran.set(ran.get() + 1);
        Err("denied".into())
    })
    .unwrap_err();
    assert_eq!(ran.get(), 2);
    // No sign-in without keyd: the session would be one only keyd can use.
    let Err(LoginError::Broker(why)) = sign_in_in(&broker, Trigger::Manual) else {
        panic!("a sign-in with keyd absent is a broker error");
    };
    assert!(why.starts_with("oculus-keyd is not running"), "{why}");
    assert!(LoginError::Broker(why)
        .to_string()
        .contains("only through it"));
}
