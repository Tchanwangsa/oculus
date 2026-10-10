use keyd_core::okta::LoginError;

use super::{refused, rig};

#[test]
fn a_calendar_fetch_does_not_turn_a_dead_session_into_an_empty_calendar() {
    let r = rig(|_, _| refused(401, &[], b"{}", LoginError::SignedOut));
    let err = crate::calendar::fetch(&r.canvas, 1).unwrap_err();
    assert!(err.contains("Signed out"), "{err}");
}
