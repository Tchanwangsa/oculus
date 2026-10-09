use super::attempts::{admit, settle, wait_after, AttemptRecord, Trigger};
use super::flow::parse_saml_form;
use super::http::{extract_state_token, state_token_candidates, Jar, LoginError};
use super::remediation::{
    challenged_factor, check_messages, option_labels, select_payload, Factor,
};
use super::totp::{base32_decode, totp_at};

#[test]
fn automatic_attempts_back_off_and_manual_ones_do_not() {
    let mut r = AttemptRecord::default();
    assert!(admit(&mut r, Trigger::Startup, 1_000_000).is_ok());
    settle(&mut r, &Err(LoginError::BadTotp(String::new())));
    assert!(matches!(
        admit(&mut r, Trigger::Browser, 1_000_599),
        Err(LoginError::Waiting(1))
    ));
    assert!(admit(&mut r, Trigger::KeepAlive, 1_000_600).is_ok());
    settle(&mut r, &Err(LoginError::Unexpected(String::new())));
    // Two failures in a row: an hour.
    assert!(matches!(
        admit(&mut r, Trigger::KeepAlive, 1_003_000),
        Err(LoginError::Waiting(_))
    ));
    assert!(admit(&mut r, Trigger::Manual, 1_003_000).is_ok());
    settle(&mut r, &Err(LoginError::BadTotp(String::new())));
    assert_eq!(wait_after(r.failures), 6 * 3600);
    settle(&mut r, &Ok(String::new()));
    assert_eq!(r.failures, 0);
}

#[test]
fn a_network_failure_waits_without_counting() {
    let mut r = AttemptRecord::default();
    admit(&mut r, Trigger::Startup, 5_000).unwrap();
    settle(&mut r, &Err(LoginError::Network(String::new())));
    assert_eq!(r.failures, 0);
    assert!(matches!(
        admit(&mut r, Trigger::Startup, 5_100),
        Err(LoginError::Waiting(500))
    ));
}

#[test]
fn a_lockout_pauses_automatic_sign_in_until_a_manual_success() {
    let mut r = AttemptRecord::default();
    admit(&mut r, Trigger::KeepAlive, 10_000).unwrap();
    settle(&mut r, &Err(LoginError::Locked("Too many attempts".into())));
    assert!(matches!(
        admit(&mut r, Trigger::KeepAlive, 1_000_000),
        Err(LoginError::Paused(_))
    ));
    assert!(admit(&mut r, Trigger::Manual, 1_000_000).is_ok());
    settle(&mut r, &Ok(String::new()));
    assert!(r.paused.is_none());
    assert!(admit(&mut r, Trigger::KeepAlive, 1_000_600).is_ok());
}

/// RFC 4226 appendix D, the canonical HOTP vectors.
#[test]
fn matches_the_rfc_4226_hotp_vectors() {
    let secret = b"12345678901234567890";
    let expected = [
        "755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583", "399871",
        "520489",
    ];
    for (counter, want) in expected.iter().enumerate() {
        // TOTP with step 1 at time == counter is exactly HOTP(counter).
        assert_eq!(
            &totp_at(secret, counter as u64, 1, 6),
            want,
            "counter {counter}"
        );
    }
}

/// RFC 6238 appendix B, the SHA-1 rows.
#[test]
fn matches_the_rfc_6238_totp_vectors() {
    let secret = b"12345678901234567890";
    for (time, want) in [
        (59u64, "94287082"),
        (1_111_111_109, "07081804"),
        (1_111_111_111, "14050471"),
        (1_234_567_890, "89005924"),
        (2_000_000_000, "69279037"),
    ] {
        assert_eq!(totp_at(secret, time, 30, 8), want, "t={time}");
    }
}

#[test]
fn decodes_base32_the_way_authenticator_apps_write_it() {
    assert_eq!(
        base32_decode("GEZDGNBVGY3TQOJQ").unwrap(),
        b"12345678901234567890"[..10].to_vec()
    );
    // Okta shows the setup key in spaced, lowercase groups.
    assert_eq!(
        base32_decode("gezd gnbv gy3t qojq").unwrap(),
        base32_decode("GEZDGNBVGY3TQOJQ").unwrap()
    );
    assert!(base32_decode("not-valid-1890").is_err());
    assert!(base32_decode("").is_err());
}

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

#[test]
fn cookies_are_filed_per_host() {
    let mut jar = Jar::default();
    jar.0
        .entry("a.example".into())
        .or_default()
        .insert("sid".into(), "1".into());
    jar.0
        .entry("b.example".into())
        .or_default()
        .insert("other".into(), "2".into());
    assert_eq!(jar.header("a.example"), "sid=1");
    assert!(!jar.has("b.example", "sid"));
    assert_eq!(jar.header("nowhere.example"), "");
}
