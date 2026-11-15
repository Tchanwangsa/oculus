use keyd_core::okta::LoginError;
use serde_json::json;

use crate::sources::canvas::{CanvasError, SessionProbe};

use super::{answer, refused, rig};

fn paused() -> LoginError {
    LoginError::Paused("a rejected password".into())
}

#[test]
fn a_401_whose_sign_in_was_refused_is_expired_with_the_reason() {
    let r = rig(|_, _| refused(401, &[], b"{}", paused()));
    let err = r.canvas.get("/api/v1/users/self").unwrap_err();
    assert_eq!(err, CanvasError::Expired(Some(paused())));
    assert!(
        err.to_string().contains("Automatic sign-in is paused"),
        "{err}"
    );
}

#[test]
fn a_probe_of_an_expired_session_is_rejected_with_the_sign_in_reason() {
    let r = rig(|_, _| refused(401, &[], b"{}", paused()));
    match r.canvas.probe() {
        SessionProbe::Rejected(why) => {
            assert!(why.contains("Automatic sign-in is paused"), "{why}")
        }
        _ => panic!("expected Rejected"),
    }
}

#[test]
fn a_missing_session_whose_sign_in_failed_is_expired() {
    let r = rig(|_, _| {
        (
            json!({"error": "missing", "detail": "no canvas session is stored",
                   "signin": {"result": "error", "code": "signed_out"}}),
            vec![],
        )
    });
    assert_eq!(
        r.canvas.get("/x").unwrap_err(),
        CanvasError::Expired(Some(LoginError::SignedOut))
    );
    assert!(matches!(r.canvas.probe(), SessionProbe::Rejected(_)));
}

#[test]
fn a_401_after_a_sign_in_that_worked_is_still_expired() {
    let r = rig(|_, _| answer(401, &[], br#"{"status":"unauthenticated"}"#));
    assert_eq!(r.canvas.get("/x").unwrap_err(), CanvasError::Expired(None));
}

#[test]
fn a_401_that_says_the_user_may_not_is_an_answer_not_an_expiry() {
    let r = rig(|_, _| answer(401, &[], br#"{"status":"unauthorized","errors":[]}"#));
    assert_eq!(r.canvas.get("/api/v1/courses/1/x").unwrap().status, 401);
    assert!(r.canvas.expired().is_none());
}

#[test]
fn a_redirect_to_the_sso_host_or_canvas_login_is_expired() {
    for location in [
        "https://sso.unimelb.edu.au/app/canvas/sso/saml",
        "/login/saml",
    ] {
        let r = rig(move |_, _| answer(302, &[("location", location)], b""));
        assert_eq!(
            r.canvas.get("/api/v1/users/self").unwrap_err(),
            CanvasError::Expired(None),
            "{location}"
        );
    }
}

#[test]
fn after_the_first_expiry_every_request_fails_without_asking_keyd_again() {
    let r = rig(|_, _| refused(401, &[], b"{}", paused()));
    r.canvas.get("/a").unwrap_err();
    assert_eq!(r.forwards().len(), 1);
    let again = r.canvas.get("/b").unwrap_err();
    assert_eq!(again, CanvasError::Expired(Some(paused())));
    assert_eq!(r.forwards().len(), 1);
    assert_eq!(
        r.canvas.expired(),
        Some(CanvasError::Expired(Some(paused())))
    );
}

/// Canvas's body for a course an expired session can no longer see.
const GONE: &[u8] = br#"{"errors":[{"message":"The specified resource does not exist."}]}"#;

#[test]
fn an_api_404_checks_the_session_and_is_sent_again_once_keyd_signed_in() {
    let tabs = std::sync::atomic::AtomicUsize::new(0);
    let r = rig(move |h, _| match h["path"].as_str().unwrap() {
        "/api/v1/courses/5/tabs" if tabs.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 => {
            answer(404, &[], GONE)
        }
        "/api/v1/courses/5/tabs" => answer(200, &[], b"[]"),
        _ => answer(200, &[], br#"{"name":"A"}"#),
    });
    assert_eq!(
        r.canvas.get_json("/api/v1/courses/5/tabs").unwrap(),
        json!([])
    );
    assert_eq!(
        r.paths(),
        [
            "/api/v1/courses/5/tabs",
            "/api/v1/users/self",
            "/api/v1/courses/5/tabs"
        ]
    );
}

#[test]
fn a_404_on_a_live_session_is_the_answer_and_checks_the_session_once() {
    let r = rig(|h, _| match h["path"].as_str().unwrap() {
        "/api/v1/users/self" => answer(200, &[], br#"{"name":"A"}"#),
        _ => answer(404, &[], GONE),
    });
    assert_eq!(r.canvas.get("/api/v1/courses/5/tabs").unwrap().status, 404);
    assert_eq!(r.canvas.get("/api/v1/courses/6/tabs").unwrap().status, 404);
    assert_eq!(r.canvas.get("/courses/6/pages/x").unwrap().status, 404);
    assert_eq!(
        r.paths(),
        [
            "/api/v1/courses/5/tabs",
            "/api/v1/users/self",
            "/api/v1/courses/5/tabs",
            "/api/v1/courses/6/tabs",
            "/courses/6/pages/x"
        ]
    );
}

#[test]
fn an_api_404_whose_session_check_could_not_sign_in_is_expired() {
    let r = rig(|h, _| match h["path"].as_str().unwrap() {
        "/api/v1/users/self" => refused(401, &[], b"{}", paused()),
        _ => answer(404, &[], GONE),
    });
    assert_eq!(
        r.canvas.get_json("/api/v1/courses/5/tabs").unwrap_err(),
        CanvasError::Expired(Some(paused()))
    );
    assert!(r.canvas.expired().is_some());
}
