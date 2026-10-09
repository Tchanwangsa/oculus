use super::seed::sent_to;
use super::*;

#[test]
fn sign_out_clears_what_cookies_for_url_would_send() {
    assert!(sent_to("canvas.lms.unimelb.edu.au", CANVAS_HOST));
    assert!(sent_to(".unimelb.edu.au", CANVAS_HOST));
    assert!(sent_to(".sso.unimelb.edu.au", crate::auth::okta::SSO_HOST));
    assert!(!sent_to("library.unimelb.edu.au", CANVAS_HOST));
    assert!(!sent_to("lms.unimelb.edu.au.evil.com", CANVAS_HOST));
    assert!(!sent_to("edstem.org", crate::auth::okta::SSO_HOST));
}
