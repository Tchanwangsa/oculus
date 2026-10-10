use super::http::{extract_state_token, state_token_candidates};
use super::*;

/// A real token is ~40+ chars; the JS literal escapes `-` as `\x2D`.
const REAL_TOKEN: &str = "02.id.7Kx9pQ2mNvL4tR8wZ1yB3cD5fG6hJ0kM-aS-eU";

#[test]
fn unescapes_the_state_token_okta_embeds() {
    let html = format!(
        r#"<script>var config = {{"stateToken":"{}"}};</script>"#,
        REAL_TOKEN.replace('-', r"\x2D")
    );
    assert_eq!(extract_state_token(&html).unwrap(), REAL_TOKEN);
}

/// Inline script names `stateToken` before the config assigns it.
#[test]
fn skips_mentions_that_are_not_the_value() {
    let html = format!(
        r#"<script>
             if (stateToken) {{ render(stateToken); }}
             var x = {{"stateToken":""}};
             var config = {{"stateToken":"{REAL_TOKEN}"}};
           </script>"#
    );
    assert_eq!(extract_state_token(&html).unwrap(), REAL_TOKEN);
    assert_eq!(state_token_candidates(&html), vec![REAL_TOKEN.to_string()]);
}

#[test]
fn accepts_the_single_quoted_assignment_form() {
    let html = format!("<script>var stateToken = '{REAL_TOKEN}';</script>");
    assert_eq!(extract_state_token(&html).unwrap(), REAL_TOKEN);
}

#[test]
fn no_state_token_is_not_a_panic() {
    assert!(extract_state_token("<html><body>maintenance</body></html>").is_none());
}

fn select_rem(options: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "name": "select-authenticator-authenticate",
        "href": "https://sso.unimelb.edu.au/idp/idx/challenge",
        "value": [{ "name": "authenticator", "type": "object", "options": options }]
    })
}

fn option(label: &str, id: &str, method: &str) -> serde_json::Value {
    serde_json::json!({
        "label": label,
        "value": { "form": { "value": [
            { "name": "id", "value": id },
            { "name": "methodType", "value": method }
        ]}}
    })
}

#[test]
fn picks_password_then_google_authenticator() {
    let rem = select_rem(serde_json::json!([
        option("Password", "aut_pw", "password"),
        option("Okta Verify", "aut_ov", "otp"),
        option("Google Authenticator", "aut_ga", "otp"),
    ]));

    assert_eq!(
        select_payload(&rem, Factor::Password).unwrap()["id"],
        "aut_pw"
    );

    // Google Authenticator wins over Okta Verify's TOTP (different seed).
    let totp = select_payload(&rem, Factor::Totp).unwrap();
    assert_eq!(totp["id"], "aut_ga");
    assert_eq!(totp["methodType"], "otp");
}

#[test]
fn a_push_only_account_reports_what_it_was_offered() {
    let rem = select_rem(serde_json::json!([
        option("Get a push notification", "aut_push", "push"),
        option("Security Key or Biometric", "aut_wa", "webauthn"),
    ]));
    assert!(select_payload(&rem, Factor::Totp).is_none());
    assert_eq!(
        option_labels(&rem),
        vec!["Get a push notification", "Security Key or Biometric"]
    );
}

fn challenge_state(key: &str, methods: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "currentAuthenticator": { "value": {
            "key": key,
            "methods": methods.iter().map(|m| serde_json::json!({"type": m})).collect::<Vec<_>>()
        }}
    })
}

/// Answered on what Okta says it is challenging, not on flow position.
#[test]
fn answers_the_factor_okta_says_it_is_challenging() {
    let pw = challenge_state("okta_password", &["password"]);
    assert_eq!(challenged_factor(&pw, false), Some(Factor::Password));
    // Already answered — do not resend it, move on to the second factor.
    assert_eq!(challenged_factor(&pw, true), None);

    let ga = challenge_state("google_otp", &["otp"]);
    assert_eq!(challenged_factor(&ga, true), Some(Factor::Totp));
}

#[test]
fn a_push_challenge_is_not_answerable() {
    let push = challenge_state("okta_verify", &["push"]);
    assert_eq!(challenged_factor(&push, true), None);
    // Okta Verify also advertises totp, but its seed is not ours.
    let ov_totp = challenge_state("okta_verify", &["totp", "push"]);
    assert_eq!(challenged_factor(&ov_totp, true), None);
    let key = challenge_state("webauthn", &["webauthn"]);
    assert_eq!(challenged_factor(&key, true), None);
}

/// When Okta describes no authenticator, fall back on flow position.
#[test]
fn an_undescribed_challenge_falls_back_to_flow_position() {
    let bare = serde_json::json!({});
    assert_eq!(challenged_factor(&bare, false), Some(Factor::Password));
    assert_eq!(challenged_factor(&bare, true), Some(Factor::Totp));
}

#[test]
fn a_wrong_code_is_a_totp_error_not_a_password_error() {
    let state = serde_json::json!({
        "messages": { "value": [{ "class": "ERROR", "message": "Invalid code. Try again." }] }
    });
    assert!(matches!(
        check_messages(&state, Some(Factor::Totp)),
        Err(LoginError::BadTotp(_))
    ));
    assert!(matches!(
        check_messages(&state, Some(Factor::Password)),
        Err(LoginError::BadPassword(_))
    ));
}

#[test]
fn a_lockout_outranks_the_factor_it_was_reported_on() {
    let state = serde_json::json!({
        "messages": { "value": [{ "class": "ERROR", "message": "Your account is locked." }] }
    });
    assert!(matches!(
        check_messages(&state, Some(Factor::Totp)),
        Err(LoginError::Locked(_))
    ));
}

#[test]
fn informational_messages_are_not_failures() {
    let state = serde_json::json!({
        "messages": { "value": [{ "class": "INFO", "message": "Verify with your password" }] }
    });
    assert!(check_messages(&state, None).is_ok());
}

#[test]
fn finds_the_assertion_form_among_decoys() {
    let html = r#"
        <form action="/search"><input name="q" value=""/></form>
        <form method="post" action="https://canvas.lms.unimelb.edu.au/login/saml">
          <input type="hidden" name="SAMLResponse" value="PHNhbWw+"/>
          <input type="hidden" name="RelayState" value="rs123"/>
        </form>"#;
    let (action, fields) = parse_saml_form(html).unwrap();
    assert_eq!(action, "https://canvas.lms.unimelb.edu.au/login/saml");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0], ("SAMLResponse".into(), "PHNhbWw+".into()));
}

use crate::test_support::{Answer, FakeOrigin, Hit};

/// `/start` redirects to `/next` setting two cookies; `/next` answers
/// plain text whatever `Accept-Encoding` asked for.
fn setting_origin() -> FakeOrigin {
    FakeOrigin::start(|hit: &Hit| match hit.path.as_str() {
        "/start" => Answer {
            status: 302,
            headers: vec![
                ("Location", "/next".to_string()),
                ("Set-Cookie", "a=1; Path=/; HttpOnly".to_string()),
                ("Set-Cookie", "b=2; Path=/other".to_string()),
            ],
            body: Vec::new(),
        },
        _ => Answer {
            status: 200,
            headers: Vec::new(),
            body: b"landed".to_vec(),
        },
    })
}

fn cookie_headers(hit: &Hit) -> Vec<&str> {
    hit.headers
        .iter()
        .filter(|(k, _)| k == "cookie")
        .map(|(_, v)| v.as_str())
        .collect()
}

#[test]
fn a_set_cookie_is_replayed_only_by_the_jar_whatever_ureq_features_are_on() {
    let origin = setting_origin();
    let mut jar = Jar::default();
    let (landed, body) = walk(&mut jar, &format!("{}/start", origin.origin), 5).unwrap();
    assert_eq!(landed.path(), "/next");
    assert_eq!(body, "landed");

    let hits = origin.hits();
    assert_eq!(hits.len(), 2);
    assert!(cookie_headers(&hits[0]).is_empty());
    // One header, the jar's. A ureq cookie store would add a second.
    assert_eq!(cookie_headers(&hits[1]), ["a=1; b=2"]);

    // A later walk with an empty jar carries nothing from the first.
    walk(&mut Jar::default(), &format!("{}/next", origin.origin), 5).unwrap();
    assert!(cookie_headers(&origin.hits()[2]).is_empty());
}

#[test]
fn every_request_asks_for_an_uncompressed_answer() {
    let origin = setting_origin();
    walk(&mut Jar::default(), &format!("{}/start", origin.origin), 5).unwrap();
    let cookie = "canvas_session=x";
    let url = format!("{}/next", origin.origin);
    assert!(request("GET", &url).set("Cookie", cookie).call().is_ok());
    for hit in origin.hits() {
        assert_eq!(hit.header("accept-encoding"), Some("identity"));
    }
}
