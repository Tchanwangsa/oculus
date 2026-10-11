use super::*;

#[test]
fn save_refuses_each_invalid_input_with_the_shared_message_and_writes_nothing() {
    let dir = Scratch::new("okta-invalid");
    let fake = fake();
    let state = state(&dir, &fake);
    const BAD_SEED: &str = "That does not look like a TOTP setup key: it may only contain the letters A–Z and the digits 2–7.";
    let cases = [
        (("  ", PASSWORD, SEED), "Username is required."),
        ((USERNAME, "", SEED), "Password is required."),
        ((USERNAME, PASSWORD, ""), BAD_SEED),
        ((USERNAME, PASSWORD, "   "), BAD_SEED),
        ((USERNAME, PASSWORD, "GEZD!NBV"), BAD_SEED),
        ((USERNAME, PASSWORD, "GEZD1NBV"), BAD_SEED),
    ];
    for ((u, p, t), message) in cases {
        let err = save(&state, u, p, t).unwrap_err();
        assert_eq!(
            (err.kind, err.detail.as_str()),
            ("request", message),
            "{u:?} {t:?}"
        );
        assert!(!err.detail.contains(PASSWORD));
    }
    for req in [
        json!({}),
        json!({"username": USERNAME, "password": PASSWORD}),
        json!({"username": USERNAME, "password": 5, "totp_secret": SEED}),
    ] {
        let err = op(&state, &cli(), "okta_save", req).unwrap_err();
        assert_eq!(err.kind, "request");
        assert!(
            err.detail.starts_with("okta_save needs a string"),
            "{}",
            err.detail
        );
    }
    assert!(!crate::paths::vault(&dir.0).exists(), "nothing was written");
}

#[test]
fn a_rejected_save_leaves_what_was_saved_before_untouched() {
    let dir = Scratch::new("okta-atomic");
    let fake = fake();
    let state = state(&dir, &fake);
    save_good(&state);
    assert!(save(&state, "someone-else", PASSWORD, "not base32 !").is_err());
    assert_eq!(
        stored(&dir, names::OKTA_USERNAME).as_deref(),
        Some(USERNAME)
    );
    assert_eq!(stored(&dir, names::OKTA_TOTP_SECRET).as_deref(), Some(SEED));
}

#[test]
fn a_save_writes_all_three_and_their_markers_in_one_vault_update() {
    let dir = Scratch::new("okta-save");
    let fake = fake();
    let state = state(&dir, &fake);
    let reply = save(
        &state,
        "  s1234567 ",
        PASSWORD,
        "gezd gnbv gy3t qojq gezd gnbv gy3t qojq",
    )
    .unwrap();
    assert_eq!(reply, json!({"saved": true}));
    // Trimmed, and the seed with its spaces removed.
    assert_eq!(
        stored(&dir, names::OKTA_USERNAME).as_deref(),
        Some("s1234567")
    );
    assert_eq!(
        stored(&dir, names::OKTA_PASSWORD).as_deref(),
        Some(PASSWORD)
    );
    assert_eq!(
        stored(&dir, names::OKTA_TOTP_SECRET).as_deref(),
        Some("gezdgnbvgy3tqojqgezdgnbvgy3tqojq")
    );
    for name in names::OKTA {
        assert!(marked(&dir, name), "{name}");
    }
    let all = vault_of(&dir).load().unwrap();
    assert_eq!(all.names().count(), 6);
}

#[test]
fn saving_clears_the_attempt_guards_wait_and_pause() {
    let dir = Scratch::new("okta-resume");
    let fake = fake();
    let state = state(&dir, &fake);
    save_good(&state);
    let record = crate::paths::sign_in_record(&dir.0);
    // A lockout, and the last attempt just over a minute ago.
    std::fs::write(
        &record,
        format!(
            r#"{{"last":{},"failures":3,"paused":"locked","manual_failures":3}}"#,
            T0 - 61
        ),
    )
    .unwrap();
    assert!(matches!(
        ensure(&state, "startup").unwrap(),
        Err(LoginError::Paused(_))
    ));
    save_good(&state);
    let text = std::fs::read_to_string(&record).unwrap();
    assert!(
        text.contains(r#""failures":0"#)
            && text.contains(r#""paused":null"#)
            && text.contains(r#""manual_failures":0"#),
        "{text}"
    );
    // So the next automatic attempt runs.
    assert!(matches!(ensure(&state, "startup").unwrap(), Ok(_)));
}

#[test]
fn nothing_a_save_does_shows_a_value() {
    let dir = Scratch::new("okta-leak");
    let fake = fake();
    let state = state(&dir, &fake);
    let reply = state
        .dispatch(
            &cli(),
            "okta_save",
            &json!({"username": USERNAME, "password": PASSWORD, "totp_secret": SEED}),
            b"",
        )
        .unwrap();
    let shown = format!("{} {:?}", reply.header, reply.note);
    for secret in [PASSWORD, SEED] {
        assert!(!shown.contains(secret), "{shown}");
    }
    let sealed = std::fs::read(crate::paths::vault(&dir.0)).unwrap();
    assert!(!sealed
        .windows(PASSWORD.len())
        .any(|w| w == PASSWORD.as_bytes()));
}
