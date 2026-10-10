//! Whether an answer from Canvas means the session it was sent with is dead.
//! Only these re-sign in (`ops/forward.rs`); a 403, a 404 or a 5xx never does.

use serde_json::Value;

/// True for a 401 that is not Canvas saying the signed-in user may not do
/// this (`"status": "unauthorized"`), or a redirect to the SSO host or to
/// Canvas's own `/login`. `canvas_origin` is where the request went, and
/// `sso_host` Okta's host.
pub fn session_rejected(
    status: u16,
    location: Option<&str>,
    body: &[u8],
    canvas_origin: &str,
    sso_host: &str,
) -> bool {
    match status {
        401 => !is_authorisation_failure(body),
        301 | 302 | 303 | 307 | 308 => {
            location.is_some_and(|l| leads_to_sign_in(l, canvas_origin, sso_host))
        }
        _ => false,
    }
}

/// Canvas answers 401 for both a missing login (`"status":
/// "unauthenticated"`) and a signed-in user who may not see the thing
/// (`"unauthorized"`). Only the first is a dead session; the second would
/// otherwise cost a sign-in for every forbidden course.
fn is_authorisation_failure(body: &[u8]) -> bool {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|v| v.get("status")?.as_str().map(|s| s == "unauthorized"))
        .unwrap_or(false)
}

fn leads_to_sign_in(location: &str, canvas_origin: &str, sso_host: &str) -> bool {
    let Ok(base) = url::Url::parse(canvas_origin) else {
        return false;
    };
    let Ok(target) = base.join(location.trim()) else {
        return false;
    };
    if target
        .host_str()
        .is_some_and(|h| h.eq_ignore_ascii_case(sso_host))
    {
        return true;
    }
    target.origin() == base.origin()
        && (target.path() == "/login" || target.path().starts_with("/login/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANVAS: &str = "https://canvas.lms.unimelb.edu.au";
    const SSO: &str = "sso.unimelb.edu.au";

    fn rejected(status: u16, location: Option<&str>, body: &[u8]) -> bool {
        session_rejected(status, location, body, CANVAS, SSO)
    }

    #[test]
    fn a_401_is_a_dead_session_unless_canvas_says_the_user_may_not() {
        assert!(rejected(401, None, b""));
        assert!(rejected(401, None, b"not json"));
        assert!(rejected(
            401,
            None,
            br#"{"status":"unauthenticated","errors":[{"message":"user authorization required"}]}"#
        ));
        assert!(!rejected(
            401,
            None,
            br#"{"status":"unauthorized","errors":[{"message":"user not authorized"}]}"#
        ));
    }

    #[test]
    fn a_redirect_to_the_sso_host_or_canvas_login_is_a_dead_session() {
        for location in [
            "https://sso.unimelb.edu.au/app/canvas/sso/saml",
            "HTTPS://SSO.UNIMELB.EDU.AU/x",
            "//sso.unimelb.edu.au/x",
            "/login",
            "/login/saml",
            "/login?redirect=1",
            "https://canvas.lms.unimelb.edu.au/login/canvas",
        ] {
            for status in [301, 302, 303, 307, 308] {
                assert!(rejected(status, Some(location), b""), "{status} {location}");
            }
        }
    }

    #[test]
    fn other_redirects_are_not() {
        for location in [
            "https://files.example-cdn.com/signed?token=abc",
            "/files/9/download?x=1",
            "/courses/1",
            "/loginx",
            "/api/login",
            "https://evil.example/login",
            "https://sso.unimelb.edu.au.evil.example/x",
            "https://canvas.lms.unimelb.edu.au:8443/login",
            "",
        ] {
            assert!(!rejected(302, Some(location), b""), "{location}");
        }
        assert!(!rejected(302, None, b""));
    }

    #[test]
    fn no_other_status_is_ever_a_dead_session() {
        for status in [
            200, 204, 304, 400, 402, 403, 404, 408, 409, 422, 429, 500, 502, 503,
        ] {
            assert!(
                !rejected(status, Some("https://sso.unimelb.edu.au/x"), b""),
                "{status}"
            );
        }
    }
}
