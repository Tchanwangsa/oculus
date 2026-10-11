use super::seed::{on_university_hosts, sent_to};
#[cfg(target_os = "macos")]
use super::seed::{sessions_from, sso_changed};
#[cfg(target_os = "macos")]
use std::sync::Mutex;

const CANVAS_HOST: &str = "canvas.lms.unimelb.edu.au";
const SSO_HOST: &str = "sso.unimelb.edu.au";

#[test]
fn sign_out_clears_what_cookies_for_url_would_send() {
    let (canvas, sso) = (CANVAS_HOST, SSO_HOST);
    assert!(sent_to("canvas.lms.unimelb.edu.au", canvas));
    assert!(sent_to(".unimelb.edu.au", canvas));
    assert!(sent_to(".sso.unimelb.edu.au", sso));
    assert!(!sent_to("library.unimelb.edu.au", canvas));
    assert!(!sent_to("lms.unimelb.edu.au.evil.com", canvas));
    assert!(!sent_to("edstem.org", sso));
}

#[test]
fn sessions_are_seeded_only_on_the_universitys_hosts() {
    let unimelb = |h| on_university_hosts(h, CANVAS_HOST, Some(SSO_HOST));
    assert!(unimelb("canvas.lms.unimelb.edu.au"));
    assert!(unimelb("library.unimelb.edu.au"));
    assert!(!unimelb("unimelb.edu.au.evil.com"));
    assert!(!unimelb("monash.edu.au"));

    // A hosted Canvas shares nothing with its IdP but the two hosts.
    let rmit = |h| on_university_hosts(h, "rmit.instructure.com", Some("myapps.rmit.edu.au"));
    assert!(rmit("rmit.instructure.com"));
    assert!(rmit("myapps.rmit.edu.au"));
    assert!(!rmit("other.instructure.com"));
    assert!(!rmit("www.rmit.edu.au"));

    let native = |h| on_university_hosts(h, "canvas.foo.edu", None);
    assert!(native("canvas.foo.edu"));
    assert!(!native("www.foo.edu"));
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

    let sessions = sessions_from(&client, &seeded, CANVAS_HOST, Some(SSO_HOST));
    assert_eq!(
        sessions,
        vec![
            (
                CANVAS_HOST.to_string(),
                "canvas_session=a".to_string(),
                true
            ),
            (SSO_HOST.to_string(), "sid=1".to_string(), true),
        ]
    );
    let sessions = sessions_from(&client, &seeded, CANVAS_HOST, Some(SSO_HOST));
    assert!(sessions[0].2, "Canvas always replaces");
    assert!(!sessions[1].2, "Okta was seeded already");

    *held.lock().unwrap() = json!({"canvas": "canvas_session=a", "sso": null});
    let sessions = sessions_from(&client, &seeded, CANVAS_HOST, Some(SSO_HOST));
    assert_eq!(sessions.len(), 1, "no Okta session, none seeded");
    assert_eq!(keyd.ops(), ["session_get", "session_get", "session_get"]);

    let empty = Scratch::new("seed-sessions-absent");
    let none = Mutex::new(None);
    assert!(sessions_from(
        &Credentialed::at(&empty),
        &none,
        CANVAS_HOST,
        Some(SSO_HOST)
    )
    .is_empty());
    assert_eq!(
        *none.lock().unwrap(),
        None,
        "a keyd that cannot be asked changes nothing"
    );
}
