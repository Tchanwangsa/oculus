use super::*;

#[test]
fn a_test_origin_must_be_loopback_http() {
    let dir = Scratch::new("okta-origins");
    for bad in [
        "https://127.0.0.1:1",
        "http://example.com:1",
        "http://127.0.0.1",
        "http://127.0.0.1:x",
        "http://localhost.evil.test:1",
    ] {
        let built = state_with(&dir, None, Box::new(NoLegacy));
        assert!(built.with_origins(Some(bad), None).is_err(), "{bad}");
        let built = state_with(&dir, None, Box::new(NoLegacy));
        assert!(built.with_origins(None, Some(bad)).is_err(), "{bad}");
    }
    let built = state_with(&dir, None, Box::new(NoLegacy));
    assert!(built.canvas_base.is_none() && built.sso_base.is_none());
}
