//! `sign_in` end to end against a scripted Canvas and Okta. One loopback
//! server plays both: Canvas is `127.0.0.1` and Okta is `localhost`, two
//! hosts for the jar to keep apart, told apart by the request's `Host`.

use std::sync::{Arc, Mutex};

use super::*;
use crate::session::Kind;
use crate::test_support::okta_fake::{script, COOKIE, PASSWORD, SEED, USERNAME};
use crate::test_support::{FakeOrigin, Scratch, TestClock};

/// 2005-03-18 01:58:31 UTC: 29 s from the next code, so no test waits for one.
const T0: u64 = 1_111_111_111;
/// RFC 6238's code for `SEED` at `T0` (8 digits there: 14050471).
const CODE: &str = "050471";

fn at_t0() -> u64 {
    T0
}

/// The credentials on file, until the password is cleared.
struct Store {
    held: Mutex<Option<(String, String)>>,
}

impl Store {
    fn with_password(password: &str) -> Store {
        Store {
            held: Mutex::new(Some((USERNAME.to_string(), password.to_string()))),
        }
    }

    fn password_cleared(&self) -> bool {
        self.held.lock().unwrap().is_none()
    }
}

impl CredentialStore for Store {
    fn load(&self) -> Result<Option<Credentials>, String> {
        Ok(self
            .held
            .lock()
            .unwrap()
            .as_ref()
            .map(|(username, password)| Credentials {
                username: username.clone(),
                password: password.clone(),
                totp_secret: SEED.to_string(),
            }))
    }

    fn clear_password(&self) -> Result<(), String> {
        *self.held.lock().unwrap() = None;
        Ok(())
    }
}

/// The sessions a sign-in saved, in order; it can be told to refuse a kind.
#[derive(Default)]
struct Sessions {
    saved: Mutex<Vec<(Kind, String)>>,
    refuse: Option<Kind>,
}

impl Sessions {
    fn get(&self, kind: Kind) -> Option<String> {
        let saved = self.saved.lock().unwrap();
        saved
            .iter()
            .rfind(|(k, _)| *k == kind)
            .map(|(_, v)| v.clone())
    }

    fn is_empty(&self) -> bool {
        self.saved.lock().unwrap().is_empty()
    }
}

impl SessionStore for Sessions {
    fn put(&self, kind: Kind, value: &str) -> Result<(), String> {
        if self.refuse == Some(kind) {
            return Err("the vault is full".to_string());
        }
        self.saved.lock().unwrap().push((kind, value.to_string()));
        Ok(())
    }
}

fn env<'a>(dir: &Scratch, origin: &str, store: &'a Store, sessions: &'a Sessions) -> Env<'a> {
    let port = origin.rsplit(':').next().unwrap();
    let mut env = Env::new(&dir.0, &format!("http://127.0.0.1:{port}"), store, sessions);
    env.sso_base = format!("http://localhost:{port}");
    env.now = Arc::new(at_t0);
    env
}

fn requests(fake: &FakeOrigin) -> Vec<String> {
    fake.hits()
        .iter()
        .map(|h| format!("{} {}", h.method, h.path.split('?').next().unwrap()))
        .collect()
}

fn log_lines(dir: &Scratch) -> Vec<String> {
    std::fs::read_to_string(paths::sign_in_log(&dir.0))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn the_scripted_seed_gives_the_rfc_code() {
    assert_eq!(totp_code(SEED, T0).unwrap(), CODE);
}

#[test]
fn a_manual_sign_in_walks_canvas_to_okta_and_back_and_saves_both_sessions() {
    let dir = Scratch::new("sign-in-ok");
    let fake = FakeOrigin::start(script(|p| p == CODE));
    let store = Store::with_password(PASSWORD);
    let sessions = Sessions::default();

    let env = env(&dir, &fake.origin, &store, &sessions);
    let cookie = sign_in(&env, Trigger::Manual, Role::App).unwrap();
    assert_eq!(cookie, COOKIE);

    assert_eq!(
        requests(&fake),
        [
            "GET /login/saml",
            "GET /app/canvas/saml",
            "POST /idp/idx/introspect",
            "POST /idp/idx/identify",
            "POST /idp/idx/challenge/answer",
            "POST /idp/idx/challenge/answer",
            "GET /login/token/redirect",
            "GET /app/canvas/saml",
            "POST /login/saml",
            "GET /",
            "GET /api/v1/users/self",
        ]
    );
    // Okta's cookies never reach Canvas, nor Canvas's Okta.
    let hits = fake.hits();
    let cookie_of = |i: usize| hits[i].header("cookie").unwrap_or("").to_string();
    assert_eq!(cookie_of(8), "canvas_session=anonymous");
    assert_eq!(cookie_of(7), "JSESSIONID=js1; sid=sess1");
    assert_eq!(cookie_of(10), COOKIE);
    assert_eq!(
        hits[8].header("content-type"),
        Some("application/x-www-form-urlencoded")
    );

    assert_eq!(sessions.get(Kind::Canvas).as_deref(), Some(COOKIE));
    assert_eq!(
        sessions.get(Kind::Sso).as_deref(),
        Some("JSESSIONID=js1; sid=sess1")
    );
    assert_eq!(sessions.get(Kind::Ed), None);
    assert!(
        !paths::cookie(&dir.0).exists() && !paths::sso_cookie(&dir.0).exists(),
        "a session goes to the store, never to a file"
    );
    assert!(markers::is_authenticated(&dir.0));

    assert_eq!(log_lines(&dir), ["2005-03-18T01:58:31Z manual: signed in"]);
    let record = std::fs::read_to_string(paths::sign_in_record(&dir.0)).unwrap();
    assert!(record.contains(&format!(r#""last":{T0}"#)) && record.contains(r#""failures":0"#));
    assert!(!store.password_cleared());
}

#[test]
fn a_rejected_password_is_reported_logged_and_forgotten_without_saving_a_session() {
    let dir = Scratch::new("sign-in-bad-password");
    let fake = FakeOrigin::start(script(|p| p == CODE));
    let store = Store::with_password("not-the-password");
    let sessions = Sessions::default();

    let result = sign_in(
        &env(&dir, &fake.origin, &store, &sessions),
        Trigger::Manual,
        Role::App,
    );
    let Err(LoginError::BadPassword(why)) = result else {
        panic!("expected BadPassword, got {result:?}");
    };
    assert_eq!(why, "Password is incorrect");
    assert!(store.password_cleared());

    assert!(sessions.is_empty());
    assert!(!markers::is_authenticated(&dir.0));
    assert_eq!(
        log_lines(&dir),
        ["2005-03-18T01:58:31Z manual: failed — Okta rejected the password: Password is incorrect"]
    );
    let record = std::fs::read_to_string(paths::sign_in_record(&dir.0)).unwrap();
    assert!(record.contains(r#""failures":1"#) && record.contains(r#""paused":"Okta rejected"#));
    assert_eq!(
        requests(&fake).last().unwrap(),
        "POST /idp/idx/challenge/answer"
    );
}

#[test]
fn an_automatic_sign_in_goes_nowhere_when_the_attempt_record_is_out_of_reach() {
    let dir = Scratch::new("sign-in-closed");
    let fake = FakeOrigin::start(script(|p| p == CODE));
    let store = Store::with_password(PASSWORD);
    let sessions = Sessions::default();
    std::fs::create_dir_all(paths::sign_in_record(&dir.0)).unwrap();

    let result = sign_in(
        &env(&dir, &fake.origin, &store, &sessions),
        Trigger::Startup,
        Role::App,
    );
    assert!(matches!(result, Err(LoginError::Paused(_))), "{result:?}");
    assert!(fake.hits().is_empty(), "no request without a record");
    assert!(!store.password_cleared());
}

#[test]
fn a_sign_in_after_a_recent_attempt_waits_without_a_request() {
    let dir = Scratch::new("sign-in-waits");
    let next_code = totp_code(SEED, T0 + 60).unwrap();
    let fake = FakeOrigin::start(script(move |p| p == CODE || p == next_code));
    let store = Store::with_password(PASSWORD);
    let sessions = Sessions::default();
    let clock = TestClock::at(T0);
    let mut env = env(&dir, &fake.origin, &store, &sessions);
    env.now = clock.clock();

    sign_in(&env, Trigger::Startup, Role::App).unwrap();
    let before = fake.hits().len();
    let result = sign_in(&env, Trigger::Browser, Role::App);
    assert!(
        matches!(result, Err(LoginError::Waiting(600))),
        "{result:?}"
    );
    // A person skips that wait, but not the minute between any two attempts.
    for role in [Role::App, Role::Cli] {
        let result = sign_in(&env, Trigger::Manual, role);
        assert!(matches!(result, Err(LoginError::Waiting(60))), "{result:?}");
    }
    assert_eq!(fake.hits().len(), before);

    clock.advance(60);
    sign_in(&env, Trigger::Manual, Role::Cli).unwrap();
    assert_eq!(
        log_lines(&dir),
        [
            "2005-03-18T01:58:31Z app startup: signed in",
            "2005-03-18T01:59:31Z manual: signed in",
        ]
    );
}

#[test]
fn a_lockout_stops_the_cli_before_any_request_and_the_app_may_try_again() {
    let dir = Scratch::new("sign-in-lockout-roles");
    let next_code = totp_code(SEED, T0 + 120).unwrap();
    let fake = FakeOrigin::start(script(move |p| p == CODE || p == next_code));
    let store = Store::with_password(PASSWORD);
    let sessions = Sessions::default();
    let clock = TestClock::at(T0 - 3600);
    let mut env = env(&dir, &fake.origin, &store, &sessions);
    env.now = clock.clock();
    let record = paths::sign_in_record(&dir.0);
    std::fs::create_dir_all(record.parent().unwrap()).unwrap();
    std::fs::write(
        &record,
        format!(
            r#"{{"last":{},"failures":1,"paused":"The account is locked or blocked: x","credentials_paused":true}}"#,
            T0 - 7200
        ),
    )
    .unwrap();

    clock.advance(3600);
    for trigger in [Trigger::Manual, Trigger::Startup] {
        let result = sign_in(&env, trigger, Role::Cli);
        assert!(matches!(result, Err(LoginError::Paused(_))), "{result:?}");
    }
    assert!(fake.hits().is_empty(), "the CLI made no request");
    assert!(log_lines(&dir).is_empty(), "a refusal is not an attempt");

    sign_in(&env, Trigger::Manual, Role::App).unwrap();
    assert!(!fake.hits().is_empty());
    // The app's success cleared the pause for the CLI as well.
    clock.advance(120);
    sign_in(&env, Trigger::Manual, Role::Cli).unwrap();
}

#[test]
fn a_signed_out_app_or_missing_credentials_stop_before_any_request() {
    let dir = Scratch::new("sign-in-stops");
    let fake = FakeOrigin::start(script(|p| p == CODE));
    let store = Store::with_password(PASSWORD);
    let sessions = Sessions::default();
    let env = env(&dir, &fake.origin, &store, &sessions);

    let marker = paths::signed_out(&env.data_dir);
    std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
    std::fs::write(&marker, b"1").unwrap();
    for trigger in [Trigger::Startup, Trigger::Browser, Trigger::Forward] {
        assert!(matches!(
            sign_in(&env, trigger, Role::App),
            Err(LoginError::SignedOut)
        ));
    }

    store.clear_password().unwrap();
    assert!(matches!(
        sign_in(&env, Trigger::Manual, Role::App),
        Err(LoginError::NotConfigured)
    ));
    assert!(fake.hits().is_empty());
}

#[test]
fn credentials_are_trimmed_and_validated_before_they_are_saved() {
    let creds = validate_credentials("  s1234567\t", "  pw with spaces ", " gezd gnbv ").unwrap();
    assert_eq!(creds.username, "s1234567");
    assert_eq!(
        creds.password, "  pw with spaces ",
        "a password is kept as typed"
    );
    assert_eq!(creds.totp_secret, "gezdgnbv");

    let refusal = |u, p, t| validate_credentials(u, p, t).err().unwrap();
    assert_eq!(refusal(" ", "pw", SEED), "Username is required.");
    assert_eq!(refusal("u", "", SEED), "Password is required.");
    assert!(refusal("u", "pw", " ").starts_with("That does not look like a TOTP setup key:"));
    assert!(refusal("u", "pw", "GEZD1").starts_with("That does not look like a TOTP setup key:"));
}

#[test]
fn a_refused_setup_key_is_never_echoed_not_even_one_character() {
    // Characters the message's own words do not contain.
    let message = validate_credentials("u", "pw", "GEZD!").err().unwrap();
    for key in [
        "1",
        "!",
        "8",
        "9",
        "0",
        "@",
        "é",
        "☃",
        "GEZD1",
        "ZZZZ\u{7f}",
    ] {
        let message = validate_credentials("u", "pw", key).err().unwrap();
        for c in key.chars().filter(|c| !c.is_alphabetic() || !c.is_ascii()) {
            assert!(!message.contains(c), "{c:?} echoed in {message}");
        }
    }
    assert_eq!(message, validate_credentials("u", "pw", "9").err().unwrap());
    assert!(message.contains("A–Z") && message.contains("2–7"));
}

#[test]
fn a_keyd_failure_and_an_unreadable_credential_are_described_as_what_they_are() {
    let broker =
        LoginError::Broker("oculus-keyd refused this program (no role)".into()).to_string();
    assert!(
        broker.contains("oculus-keyd") && broker.contains("no role"),
        "{broker}"
    );
    assert!(!broker.contains("Unexpected sign-in step"), "{broker}");

    let unreadable = LoginError::UnreadableCredentials("OSStatus -128".into()).to_string();
    assert!(unreadable.contains("OSStatus -128") && unreadable.contains("not missing"));
    assert!(
        unreadable.contains("oculus-keyd") && unreadable.contains("this program"),
        "{unreadable}"
    );
}

#[test]
fn any_trigger_that_signs_in_marks_the_app_authenticated_and_a_manual_one_lifts_the_sign_out() {
    for trigger in [
        Trigger::Startup,
        Trigger::Browser,
        Trigger::Forward,
        Trigger::Manual,
    ] {
        let dir = Scratch::new("sign-in-marks");
        let fake = FakeOrigin::start(script(|p| p == CODE));
        let (store, sessions) = (Store::with_password(PASSWORD), Sessions::default());
        let env = env(&dir, &fake.origin, &store, &sessions);
        assert!(!markers::is_authenticated(&dir.0));
        sign_in(&env, trigger, Role::Cli).unwrap();
        assert!(markers::is_authenticated(&dir.0), "{trigger:?}");
    }

    let dir = Scratch::new("sign-in-lifts-sign-out");
    let fake = FakeOrigin::start(script(|p| p == CODE));
    let (store, sessions) = (Store::with_password(PASSWORD), Sessions::default());
    let env = env(&dir, &fake.origin, &store, &sessions);
    markers::mark_signed_out(&dir.0).unwrap();
    sign_in(&env, Trigger::Manual, Role::App).unwrap();
    assert!(markers::is_authenticated(&dir.0) && !markers::is_signed_out(&dir.0));
}

#[test]
fn a_session_that_cannot_be_saved_is_a_failed_sign_in_and_an_okta_one_is_not() {
    let dir = Scratch::new("sign-in-store-refuses");
    let fake = FakeOrigin::start(script(|p| p == CODE));
    let store = Store::with_password(PASSWORD);
    let sessions = Sessions {
        refuse: Some(Kind::Canvas),
        ..Sessions::default()
    };
    let result = sign_in(
        &env(&dir, &fake.origin, &store, &sessions),
        Trigger::Manual,
        Role::App,
    );
    let Err(LoginError::Unexpected(why)) = result else {
        panic!("{result:?}");
    };
    assert!(why.contains("could not save the session cookie"), "{why}");
    assert!(sessions.is_empty() && !markers::is_authenticated(&dir.0));

    let dir = Scratch::new("sign-in-sso-refused");
    let fake = FakeOrigin::start(script(|p| p == CODE));
    let sessions = Sessions {
        refuse: Some(Kind::Sso),
        ..Sessions::default()
    };
    sign_in(
        &env(&dir, &fake.origin, &store, &sessions),
        Trigger::Manual,
        Role::App,
    )
    .unwrap();
    assert_eq!(sessions.get(Kind::Canvas).as_deref(), Some(COOKIE));
    assert!(markers::is_authenticated(&dir.0));
}

#[test]
fn with_nowhere_to_keep_a_session_the_sign_in_fails_and_marks_nothing() {
    let dir = Scratch::new("sign-in-no-sessions");
    let fake = FakeOrigin::start(script(|p| p == CODE));
    let store = Store::with_password(PASSWORD);
    let unused = Sessions::default();
    let mut env = env(&dir, &fake.origin, &store, &unused);
    env.sessions = &NoSessions;
    let result = sign_in(&env, Trigger::Manual, Role::App);
    assert!(
        matches!(result, Err(LoginError::Unexpected(_))),
        "{result:?}"
    );
    assert!(!markers::is_authenticated(&dir.0));
}
