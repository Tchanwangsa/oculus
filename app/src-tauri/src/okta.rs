//! The app's half of the headless University of Melbourne SSO sign-in.
//!
//! The flow itself (Okta's IDX state machine, the SAML round trip, the
//! attempt guard) is `keyd_core::okta`. With `oculus-keyd` installed every
//! call here goes through it and the credentials live in its vault; only when
//! the client says `KeydError::Absent` do the keychain and an in-process run
//! of the flow answer. Any other keyd error surfaces and starts no second
//! route, so a refusal can never become a second attempt against Okta.
//!
//! Password and seed are kept together, so to anything running as this user
//! the second factor is not a second factor — the same deliberate trade as a
//! password manager holding TOTP.

use crate::credentials::{Credentialed, KeydError};
pub use keyd_core::okta::{totp_now, LoginError, Trigger, SSO_HOST};
use keyd_core::okta::{validate_credentials, CredentialStore, Credentials, Env};
pub use keyd_core::platform::Role;

fn broker() -> Credentialed {
    Credentialed::at(&crate::paths::data_dir())
}

// ── The keychain fallback ────────────────────────────────────────────────────

const KEYCHAIN_SERVICE: &str = "com.oculus.unimelb-sso";

fn secret(account: &str) -> crate::credentials::Secret<'_> {
    crate::credentials::Secret::new(KEYCHAIN_SERVICE, account)
}

/// `Err` when the keychain refused the read, as opposed to holding nothing.
fn read(account: &str) -> Result<Option<String>, String> {
    Ok(secret(account).fetch()?.filter(|s| !s.is_empty()))
}

fn write(account: &str, value: &str) -> Result<(), String> {
    secret(account).write(value)
}

fn erase(account: &str) -> Result<(), String> {
    secret(account).delete()
}

/// The keychain, as the sign-in's credential store.
struct Keychain;

impl CredentialStore for Keychain {
    /// `Ok(None)` when any piece is missing; a refused read is `Err`, never
    /// "not set up".
    fn load(&self) -> Result<Option<Credentials>, String> {
        let Some(username) = read("username")? else {
            return Ok(None);
        };
        let Some(password) = read("password")? else {
            return Ok(None);
        };
        let Some(totp_secret) = read("totp_secret")? else {
            return Ok(None);
        };
        Ok(Some(Credentials {
            username,
            password,
            totp_secret,
        }))
    }

    fn clear_password(&self) -> Result<(), String> {
        clear_password()
    }
}

/// Which pieces are on file, for the settings UI; values never leave keyd or
/// the keychain.
#[derive(Debug, serde::Serialize)]
pub struct CredentialStatus {
    pub username: Option<String>,
    pub has_password: bool,
    pub has_totp: bool,
}

fn keychain_status() -> Result<CredentialStatus, String> {
    let unreadable = |e| LoginError::UnreadableCredentials(e).to_string();
    Ok(CredentialStatus {
        username: read("username").map_err(unreadable)?,
        has_password: read("password").map_err(unreadable)?.is_some(),
        has_totp: read("totp_secret").map_err(unreadable)?.is_some(),
    })
}

/// Validates with `keyd_core::okta::validate_credentials`, the check keyd
/// applies too, then saves all three. A new save lifts the attempt guard.
fn keychain_store(username: &str, password: &str, totp_secret: &str) -> Result<(), String> {
    let creds = validate_credentials(username, password, totp_secret)?;
    write("username", &creds.username)?;
    write("password", &creds.password)?;
    write("totp_secret", &creds.totp_secret)?;
    if let Err(why) = keyd_core::okta::resume_automatic_sign_in(&crate::paths::data_dir()) {
        eprintln!("[oculus] the attempt guard was not cleared: {why}");
    }
    Ok(())
}

fn keychain_forget() -> Result<(), String> {
    erase("username")?;
    erase("password")?;
    erase("totp_secret")?;
    Ok(())
}

/// Drop only the password, keeping username and seed — the response to
/// `LoginError::BadPassword`.
fn clear_password() -> Result<(), String> {
    erase("password")
}

// ── Saved credentials, through keyd ──────────────────────────────────────────

// Each `*_in` asks `broker` and runs its fallback only when keyd is absent.

pub fn credential_status() -> Result<CredentialStatus, String> {
    credential_status_in(&broker(), keychain_status)
}

fn credential_status_in(
    broker: &Credentialed,
    keychain: impl FnOnce() -> Result<CredentialStatus, String>,
) -> Result<CredentialStatus, String> {
    match broker.okta_status() {
        Ok(status) => Ok(CredentialStatus {
            username: status.username,
            has_password: status.has_password,
            has_totp: status.has_totp,
        }),
        Err(KeydError::Absent) => keychain(),
        Err(KeydError::Keychain(e)) => Err(LoginError::UnreadableCredentials(e).to_string()),
        Err(e) => Err(format!(
            "Could not check the saved sign-in credentials: {e}"
        )),
    }
}

/// Saves all three. keyd validates; its message for bad input is passed on
/// as written.
pub fn store_credentials(username: &str, password: &str, totp_secret: &str) -> Result<(), String> {
    store_credentials_in(&broker(), username, password, totp_secret, || {
        keychain_store(username, password, totp_secret)
    })
}

fn store_credentials_in(
    broker: &Credentialed,
    username: &str,
    password: &str,
    totp_secret: &str,
    keychain: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match broker.okta_save(username, password, totp_secret) {
        Err(KeydError::Absent) => keychain(),
        Err(KeydError::Request(message)) => Err(message),
        other => other.map_err(|e| e.to_string()),
    }
}

/// Forget everything. Called on explicit disconnect.
pub fn clear_credentials() -> Result<(), String> {
    clear_credentials_in(&broker(), keychain_forget)
}

fn clear_credentials_in(
    broker: &Credentialed,
    keychain: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match broker.okta_forget() {
        Err(KeydError::Absent) => keychain(),
        other => other.map(|_| ()).map_err(|e| e.to_string()),
    }
}

// ── The sign-in ──────────────────────────────────────────────────────────────

fn env(data_dir: &std::path::Path) -> Env<'static> {
    Env::new(data_dir, crate::paths::CANVAS_BASE, &Keychain)
}

/// Headless sign-in behind the attempt guard. keyd runs it and writes the
/// session files, taking `role` from the caller's identity; with keyd absent
/// it runs here with the keychain's credentials and the `role` the caller
/// states (`keyd_core::okta::sign_in`).
pub fn sign_in(data_dir: &std::path::Path, trigger: Trigger, role: Role) -> Result<(), LoginError> {
    sign_in_in(&Credentialed::at(data_dir), trigger, || {
        keyd_core::okta::sign_in(&env(data_dir), trigger, role).map(|_| ())
    })
}

fn sign_in_in(
    broker: &Credentialed,
    trigger: Trigger,
    in_process: impl FnOnce() -> Result<(), LoginError>,
) -> Result<(), LoginError> {
    match broker.ensure_signed_in(trigger) {
        Ok(outcome) => outcome,
        Err(KeydError::Absent) => in_process(),
        Err(KeydError::Keychain(e)) => Err(LoginError::UnreadableCredentials(e)),
        Err(e) => Err(LoginError::Broker(e.to_string())),
    }
}

/// Clears the attempt guard's failures, pause and wait after a person signed
/// in themselves. keyd does it, so the app never writes the record; only an
/// absent keyd lets this process reset it. A failure is logged: the sign-in
/// itself succeeded.
pub fn resume_automatic_sign_in(data_dir: &std::path::Path) {
    resume_in(&Credentialed::at(data_dir), || {
        keyd_core::okta::resume_automatic_sign_in(data_dir)
    });
}

fn resume_in(broker: &Credentialed, in_process: impl FnOnce() -> Result<(), String>) {
    let outcome = match broker.okta_resume() {
        Err(KeydError::Absent) => in_process(),
        other => other.map_err(|e| e.to_string()),
    };
    if let Err(why) = outcome {
        eprintln!("[oculus] the attempt guard was not cleared: {why}");
    }
}

/// What the sign-in page looks like from here, for when the flow fails.
pub fn diagnose() -> String {
    keyd_core::okta::diagnose(&env(&crate::paths::data_dir()))
}

// ── Tauri commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn okta_credential_status() -> Result<CredentialStatus, String> {
    credential_status()
}

#[tauri::command]
pub fn okta_save_credentials(
    username: String,
    password: String,
    totp_secret: String,
) -> Result<(), String> {
    store_credentials(&username, &password, &totp_secret)
}

#[tauri::command]
pub fn okta_clear_credentials() -> Result<(), String> {
    clear_credentials()
}

#[tauri::command]
pub async fn okta_sign_in(app: tauri::AppHandle) -> Result<String, String> {
    crate::blocking::run(move || run_sign_in(&app, &crate::paths::data_dir(), Trigger::Manual))
        .await
}

fn run_sign_in(
    app: &tauri::AppHandle,
    dir: &std::path::Path,
    trigger: Trigger,
) -> Result<String, String> {
    sign_in(dir, trigger, Role::App).map_err(|e| e.to_string())?;
    signed_in(app, dir)
}

/// End the headless sign-in the way every sign-in ends, returning the account
/// name.
fn signed_in(app: &tauri::AppHandle, dir: &std::path::Path) -> Result<String, String> {
    crate::auth::session_established(app, dir, crate::auth::Via::Headless);
    // The headless path works on this account, so a re-authenticating
    // LaunchAgent is worth installing.
    crate::keepalive::ensure_installed();
    crate::canvas::Canvas::open(dir).whoami()
}

/// Called when a probe finds the session dead: rebuild it silently if
/// automated sign-in is set up. `false` means ask the user; every reason but
/// "never set up" and "signed out" is logged, a keychain refusal included.
pub fn try_auto_recover(app: &tauri::AppHandle, trigger: Trigger) -> bool {
    let dir = crate::paths::data_dir();
    let outcome = match sign_in(&dir, trigger, Role::App) {
        Err(LoginError::NotConfigured | LoginError::SignedOut) => return false,
        Err(e) => Err(e.to_string()),
        Ok(()) => signed_in(app, &dir),
    };
    match outcome {
        Ok(name) => {
            eprintln!("[oculus] session rebuilt without a browser ({name})");
            true
        }
        Err(e) => {
            eprintln!("[oculus] automated re-sign-in failed: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FakeKeyd, Scratch};
    use keyd_core::okta::outcome_to_wire;
    use serde_json::{json, Value};

    const TRIGGERS: [(Trigger, &str); 4] = [
        (Trigger::Manual, "manual"),
        (Trigger::Startup, "startup"),
        (Trigger::KeepAlive, "keep-alive"),
        (Trigger::Browser, "browser"),
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
            let err =
                credential_status_in(&Credentialed::at(&dir), || panic!("keychain")).unwrap_err();
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
            sign_in_in(&broker, trigger, || panic!("in-process")).unwrap();
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
            let got = sign_in_in(&Credentialed::at(&dir), Trigger::Manual, || {
                panic!("in-process")
            })
            .unwrap_err();
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
            let got = sign_in_in(&Credentialed::at(&dir), Trigger::KeepAlive, || {
                panic!("in-process")
            })
            .unwrap_err();
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
    fn only_an_absent_keyd_runs_the_fallbacks() {
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
        let got = sign_in_in(&broker, Trigger::Manual, || Err(LoginError::NotConfigured));
        assert_eq!(got, Err(LoginError::NotConfigured));
    }
}
