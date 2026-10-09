//! `sign_in` end to end against a scripted Canvas and Okta. One loopback
//! server plays both: Canvas is `127.0.0.1` and Okta is `localhost`, two
//! hosts for the jar to keep apart, told apart by the request's `Host`.

use std::sync::Mutex;

use super::*;
use crate::test_support::okta_fake::{script, COOKIE, PASSWORD, SEED, USERNAME};
use crate::test_support::{FakeOrigin, Scratch};

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

fn env<'a>(dir: &Scratch, origin: &str, store: &'a Store) -> Env<'a> {
    let port = origin.rsplit(':').next().unwrap();
    let mut env = Env::new(&dir.0, &format!("http://127.0.0.1:{port}"), store);
    env.sso_base = format!("http://localhost:{port}");
    env.now = at_t0;
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

    let cookie = sign_in(&env(&dir, &fake.origin, &store), Trigger::Manual).unwrap();
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

    let session = paths::cookie(&dir.0);
    let sso = paths::sso_cookie(&dir.0);
    assert_eq!(std::fs::read_to_string(&session).unwrap(), COOKIE);
    assert_eq!(
        std::fs::read_to_string(&sso).unwrap(),
        "JSESSIONID=js1; sid=sess1"
    );
    #[cfg(unix)]
    {
        assert!(crate::platform::files::is_owner_only(&session).unwrap());
        assert!(crate::platform::files::is_owner_only(&sso).unwrap());
    }

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

    let result = sign_in(&env(&dir, &fake.origin, &store), Trigger::Manual);
    let Err(LoginError::BadPassword(why)) = result else {
        panic!("expected BadPassword, got {result:?}");
    };
    assert_eq!(why, "Password is incorrect");
    assert!(store.password_cleared());

    assert!(!paths::cookie(&dir.0).exists());
    assert!(!paths::sso_cookie(&dir.0).exists());
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
    std::fs::create_dir_all(paths::sign_in_record(&dir.0)).unwrap();

    let result = sign_in(&env(&dir, &fake.origin, &store), Trigger::KeepAlive);
    assert!(matches!(result, Err(LoginError::Paused(_))), "{result:?}");
    assert!(fake.hits().is_empty(), "no request without a record");
    assert!(!store.password_cleared());
}

#[test]
fn an_automatic_sign_in_after_a_recent_attempt_waits_without_a_request() {
    let dir = Scratch::new("sign-in-waits");
    let fake = FakeOrigin::start(script(|p| p == CODE));
    let store = Store::with_password(PASSWORD);
    let env = env(&dir, &fake.origin, &store);

    sign_in(&env, Trigger::Startup).unwrap();
    let before = fake.hits().len();
    let result = sign_in(&env, Trigger::Browser);
    assert!(
        matches!(result, Err(LoginError::Waiting(600))),
        "{result:?}"
    );
    assert_eq!(fake.hits().len(), before);
    // The wait does not apply to a person.
    sign_in(&env, Trigger::Manual).unwrap();
    assert_eq!(
        log_lines(&dir),
        [
            "2005-03-18T01:58:31Z app startup: signed in",
            "2005-03-18T01:58:31Z manual: signed in",
        ]
    );
}

#[test]
fn a_signed_out_app_or_missing_credentials_stop_before_any_request() {
    let dir = Scratch::new("sign-in-stops");
    let fake = FakeOrigin::start(script(|p| p == CODE));
    let store = Store::with_password(PASSWORD);
    let env = env(&dir, &fake.origin, &store);

    let marker = paths::signed_out(&env.data_dir);
    std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
    std::fs::write(&marker, b"1").unwrap();
    assert!(matches!(
        sign_in(&env, Trigger::KeepAlive),
        Err(LoginError::SignedOut)
    ));

    store.clear_password().unwrap();
    assert!(matches!(
        sign_in(&env, Trigger::Manual),
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
    assert_eq!(
        refusal("u", "pw", " "),
        "That does not look like a TOTP setup key: secret is empty"
    );
    assert!(refusal("u", "pw", "GEZD1").starts_with("That does not look like a TOTP setup key:"));
}
