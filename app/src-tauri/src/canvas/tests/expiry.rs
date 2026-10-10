use keyd_core::okta::LoginError;
use serde_json::json;

use crate::canvas::{CanvasError, SessionProbe};

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
