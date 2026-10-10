//! A scripted Canvas and Okta on one loopback server, for the sign-in's tests
//! and keyd's. Canvas is `127.0.0.1` and Okta is `localhost`, two hosts for
//! the jar to keep apart, told apart by the request's `Host`.

use super::{Answer, Hit};

/// base32 of the RFC 6238 test secret `12345678901234567890`.
pub const SEED: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
pub const USERNAME: &str = "s1234567";
pub const PASSWORD: &str = "hunter2";
pub const STATE_TOKEN: &str = "02.id.7Kx9pQ2mNvL4tR8wZ1yB3cD5fG6hJ0kM-aS-eU";
/// What a signed-in Canvas session is.
pub const COOKIE: &str = "_csrf_token=csrf1; canvas_session=real";

/// For a sign-in that runs on the real clock: the code for this 30 s window
/// or the last.
pub fn current_code(passcode: &str) -> bool {
    let now = crate::clock::now_secs();
    [now, now - 30]
        .iter()
        .any(|t| crate::okta::totp_code(SEED, *t).is_ok_and(|code| code == passcode))
}

/// The instant a test clock pins: 2005-03-18 01:58:31 UTC, 29 s before the
/// next code, so no sign-in under it waits for a fresh one.
pub const T0: u64 = 1_111_111_111;

/// For a sign-in whose clock starts at `T0` and is moved in whole windows
/// (30 s): the code of any window in the next day.
pub fn code_from_t0(passcode: &str) -> bool {
    (0..3000).any(|k| crate::okta::totp_code(SEED, T0 + 30 * k).is_ok_and(|code| code == passcode))
}

pub fn answer(status: u16, headers: &[(&'static str, &str)], body: &str) -> Answer {
    Answer {
        status,
        headers: headers.iter().map(|(k, v)| (*k, v.to_string())).collect(),
        body: body.as_bytes().to_vec(),
    }
}

pub fn json(status: u16, headers: &[(&'static str, &str)], body: serde_json::Value) -> Answer {
    answer(status, headers, &body.to_string())
}

/// What Okta says to each request. Anything it was not expecting is a 404,
/// so the sign-in under test fails rather than the fake.
pub fn script(
    passcode_ok: impl Fn(&str) -> bool + Send + 'static,
) -> impl Fn(&Hit) -> Answer + Send + 'static {
    move |hit| respond(hit, &passcode_ok)
}

fn respond(hit: &Hit, passcode_ok: &dyn Fn(&str) -> bool) -> Answer {
    let host = hit.header("host").unwrap_or("");
    let (name, port) = host.split_once(':').unwrap_or((host, ""));
    let canvas = format!("http://127.0.0.1:{port}");
    let sso = format!("http://localhost:{port}");
    let cookie = hit.header("cookie").unwrap_or("");
    let path = hit.path.split('?').next().unwrap_or("");
    let body = String::from_utf8_lossy(&hit.body).to_string();
    let sent: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let passcode = sent["credentials"]["passcode"].as_str().unwrap_or("");

    match (name, hit.method.as_str(), path) {
        ("127.0.0.1", "GET", "/login/saml") => answer(
            302,
            &[
                ("Location", &format!("{sso}/app/canvas/saml")),
                ("Set-Cookie", "canvas_session=anonymous; Path=/"),
            ],
            "",
        ),
        ("127.0.0.1", "POST", "/login/saml") if body.contains("SAMLResponse=PHNhbWw%2B") => answer(
            302,
            &[
                ("Location", &format!("{canvas}/")),
                ("Set-Cookie", "canvas_session=real; Path=/"),
                ("Set-Cookie", "_csrf_token=csrf1; Path=/"),
            ],
            "",
        ),
        ("127.0.0.1", "GET", "/") => answer(200, &[], "dashboard"),
        ("127.0.0.1", "GET", "/api/v1/users/self") if cookie == COOKIE => {
            json(200, &[], serde_json::json!({ "name": "Test Student" }))
        }
        ("127.0.0.1", "GET", "/api/v1/users/self") => answer(401, &[], "{}"),

        // The SAML app URL: the sign-in page until Okta has a session, then
        // the assertion form that posts to Canvas.
        ("localhost", "GET", "/app/canvas/saml") if cookie.contains("sid=sess1") => answer(
            200,
            &[],
            &format!(
                r#"<form method="post" action="{canvas}/login/saml">
                     <input type="hidden" name="SAMLResponse" value="PHNhbWw+"/>
                     <input type="hidden" name="RelayState" value="rs123"/>
                   </form>"#
            ),
        ),
        ("localhost", "GET", "/app/canvas/saml") => answer(
            200,
            &[],
            &format!(
                r#"<script>var config = {{"stateToken":"{}"}};</script>"#,
                STATE_TOKEN.replace('-', r"\x2D")
            ),
        ),
        ("localhost", "POST", "/idp/idx/introspect") if sent["stateToken"] == STATE_TOKEN => json(
            200,
            &[("Set-Cookie", "JSESSIONID=js1; Path=/")],
            serde_json::json!({
                "stateHandle": "sh1",
                "remediation": { "value": [{
                    "name": "identify",
                    "href": format!("{sso}/idp/idx/identify"),
                    "value": [{ "name": "identifier" }]
                }]}
            }),
        ),
        ("localhost", "POST", "/idp/idx/identify") if sent["identifier"] == USERNAME => json(
            200,
            &[],
            serde_json::json!({
                "stateHandle": "sh1",
                "currentAuthenticator": { "value": {
                    "key": "okta_password", "methods": [{ "type": "password" }]
                }},
                "remediation": { "value": [
                    { "name": "select-authenticator-authenticate", "value": [] },
                    { "name": "challenge-authenticator",
                      "href": format!("{sso}/idp/idx/challenge/answer") }
                ]}
            }),
        ),
        ("localhost", "POST", "/idp/idx/challenge/answer") if passcode == PASSWORD => json(
            200,
            &[],
            serde_json::json!({
                "stateHandle": "sh1",
                "currentAuthenticator": { "value": {
                    "key": "google_otp", "methods": [{ "type": "otp" }]
                }},
                "remediation": { "value": [{
                    "name": "challenge-authenticator",
                    "href": format!("{sso}/idp/idx/challenge/answer")
                }]}
            }),
        ),
        ("localhost", "POST", "/idp/idx/challenge/answer") if passcode_ok(passcode) => json(
            200,
            &[],
            serde_json::json!({
                "stateHandle": "sh1",
                "success": { "href": format!("{sso}/login/token/redirect?stateToken=t") }
            }),
        ),
        ("localhost", "POST", "/idp/idx/challenge/answer") => json(
            401,
            &[],
            serde_json::json!({
                "messages": { "value": [{ "class": "ERROR", "message": "Password is incorrect" }] }
            }),
        ),
        ("localhost", "GET", "/login/token/redirect") => {
            answer(200, &[("Set-Cookie", "sid=sess1; Path=/")], "")
        }
        _ => answer(404, &[], "unexpected"),
    }
}
