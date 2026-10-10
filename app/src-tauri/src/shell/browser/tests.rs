use super::seed::sent_to;
#[cfg(target_os = "macos")]
use super::seed::{sessions_from, sso_changed};
use super::*;
#[cfg(target_os = "macos")]
use std::sync::Mutex;

#[test]
fn sign_out_clears_what_cookies_for_url_would_send() {
    assert!(sent_to("canvas.lms.unimelb.edu.au", CANVAS_HOST));
    assert!(sent_to(".unimelb.edu.au", CANVAS_HOST));
    assert!(sent_to(".sso.unimelb.edu.au", crate::auth::okta::SSO_HOST));
    assert!(!sent_to("library.unimelb.edu.au", CANVAS_HOST));
    assert!(!sent_to("lms.unimelb.edu.au.evil.com", CANVAS_HOST));
    assert!(!sent_to("edstem.org", crate::auth::okta::SSO_HOST));
}

#[cfg(target_os = "macos")]
#[test]
fn okta_replaces_the_jar_only_when_the_stored_header_changed() {
    let mut seeded = None;
    assert!(
        !sso_changed(&mut seeded, None),
        "nothing stored, nothing to replace"
    );
    assert!(
        sso_changed(&mut seeded, Some("sid=a")),
        "the first seed of a run"
    );
    assert!(!sso_changed(&mut seeded, Some("sid=a")), "the same header");
    assert!(
        sso_changed(&mut seeded, Some("sid=b")),
        "a headless sign-in wrote a new one"
    );
    assert!(sso_changed(&mut seeded, None), "the session was dropped");
    assert!(!sso_changed(&mut seeded, None));
}

#[cfg(target_os = "macos")]
#[test]
fn sessions_come_from_keyd_and_a_missing_keyd_or_session_seeds_nothing() {
    use crate::providers::credentials::Credentialed;
    use crate::test_support::{FakeKeyd, Scratch};
    use serde_json::json;

    let dir = Scratch::new("seed-sessions");
    let held = std::sync::Arc::new(Mutex::new(
        json!({"canvas": "canvas_session=a", "sso": "sid=1"}),
    ));
    let served = held.clone();
    let keyd = FakeKeyd::start(&dir, move |_, _| (served.lock().unwrap().clone(), vec![]));
    let client = Credentialed::at(&dir);
    let seeded = Mutex::new(None);

    let sessions = sessions_from(&client, &seeded);
    assert_eq!(
        sessions,
        vec![
            (CANVAS_HOST, "canvas_session=a".to_string(), true),
            (crate::auth::okta::SSO_HOST, "sid=1".to_string(), true),
        ]
    );
    let sessions = sessions_from(&client, &seeded);
    assert!(sessions[0].2, "Canvas always replaces");
    assert!(!sessions[1].2, "Okta was seeded already");

    *held.lock().unwrap() = json!({"canvas": "canvas_session=a", "sso": null});
    let sessions = sessions_from(&client, &seeded);
    assert_eq!(sessions.len(), 1, "no Okta session, none seeded");
    assert_eq!(keyd.ops(), ["session_get", "session_get", "session_get"]);

    let empty = Scratch::new("seed-sessions-absent");
    let none = Mutex::new(None);
    assert!(sessions_from(&Credentialed::at(&empty), &none).is_empty());
    assert_eq!(
        *none.lock().unwrap(),
        None,
        "a keyd that cannot be asked changes nothing"
    );
}
