use super::*;

#[test]
fn a_sign_in_through_dispatch_saves_both_sessions_in_the_vault_and_replies_signed_in() {
    let dir = Scratch::new("okta-ensure");
    let fake = fake();
    let state = state(&dir, &fake);
    save_good(&state);

    let reply = state
        .dispatch(
            &cli(),
            "ensure_signed_in",
            &json!({"trigger": "manual"}),
            b"",
        )
        .unwrap();
    assert_eq!(reply.header, json!({"result": "signed_in"}));
    assert_eq!(reply.note.as_deref(), Some("result=signed_in"));
    assert_eq!(stored(&dir, "session.canvas").as_deref(), Some(COOKIE));
    assert_eq!(
        stored(&dir, "session.sso").as_deref(),
        Some("JSESSIONID=js1; sid=sess1")
    );
    assert_eq!(state.session_generation(), 2, "both sessions are counted");
    assert!(
        !crate::paths::cookie(&dir.0).exists() && !crate::paths::sso_cookie(&dir.0).exists(),
        "no cookie file"
    );
    assert!(crate::paths::authenticated(&dir.0).exists());
    assert!(!reply.header.to_string().contains("canvas_session"));
    assert_eq!(requests(&fake), 11, "one flow");
    let log = std::fs::read_to_string(crate::paths::sign_in_log(&dir.0)).unwrap();
    assert!(log.trim_end().ends_with("manual: signed in"), "{log}");
    for secret in [PASSWORD, SEED] {
        assert!(!log.contains(secret));
    }
}

#[test]
fn an_unknown_or_missing_trigger_is_a_request_error() {
    let dir = Scratch::new("okta-trigger");
    let fake = fake();
    let state = state(&dir, &fake);
    for req in [
        json!({}),
        json!({"trigger": "Forward"}),
        json!({"trigger": 1}),
        json!({"trigger": "app startup"}),
    ] {
        let err = op(&state, &cli(), "ensure_signed_in", req).unwrap_err();
        assert_eq!(err.kind, "request");
    }
    assert_eq!(requests(&fake), 0);
}

#[test]
fn a_bad_password_is_an_outcome_that_clears_only_the_vaults_password() {
    let dir = Scratch::new("okta-bad-password");
    let fake = fake();
    let state = state(&dir, &fake);
    save(&state, USERNAME, "not-the-password", SEED).unwrap();

    let reply = state
        .dispatch(
            &cli(),
            "ensure_signed_in",
            &json!({"trigger": "manual"}),
            b"",
        )
        .unwrap();
    assert_eq!(
        reply.header,
        json!({"result": "error", "code": "bad_password", "detail": "Password is incorrect"})
    );
    assert_eq!(
        reply.note.as_deref(),
        Some("result=error code=bad_password")
    );
    assert_eq!(stored(&dir, names::OKTA_PASSWORD), None);
    assert!(
        marked(&dir, names::OKTA_PASSWORD),
        "the marker stays, so no re-import"
    );
    assert_eq!(
        stored(&dir, names::OKTA_USERNAME).as_deref(),
        Some(USERNAME)
    );
    assert_eq!(stored(&dir, names::OKTA_TOTP_SECRET).as_deref(), Some(SEED));
    assert_eq!(stored(&dir, "session.canvas"), None);
    assert_eq!(
        status(&state),
        OktaStatus {
            username: Some(USERNAME.into()),
            has_password: false,
            has_totp: true,
        }
    );
    // With no password there is nothing to replay.
    let before = requests(&fake);
    assert!(matches!(
        ensure(&state, "manual").unwrap(),
        Err(LoginError::NotConfigured)
    ));
    assert_eq!(requests(&fake), before);
}

#[test]
fn the_second_automatic_attempt_waits_and_a_manual_one_waits_a_minute() {
    let dir = Scratch::new("okta-guard");
    let fake = fake();
    let (state, clock) = clocked(&dir, &fake);
    save_good(&state);

    assert_eq!(ensure(&state, "startup").unwrap(), Ok(()));
    let before = requests(&fake);
    assert_eq!(
        ensure(&state, "browser").unwrap(),
        Err(LoginError::Waiting(600))
    );
    assert_eq!(
        ensure(&state, "manual").unwrap(),
        Err(LoginError::Waiting(60))
    );
    assert_eq!(requests(&fake), before, "the wait costs no request");

    clock.advance(60);
    assert_eq!(ensure(&state, "manual").unwrap(), Ok(()));
    assert!(requests(&fake) > before);
    assert_eq!(
        ensure(&state, "manual").unwrap(),
        Err(LoginError::Waiting(60))
    );
}

#[test]
fn a_manual_request_from_the_cli_cannot_lift_a_lockout_but_the_apps_can() {
    let dir = Scratch::new("okta-lockout-roles");
    let fake = fake();
    let (state, clock) = clocked(&dir, &fake);
    save_good(&state);
    let record = crate::paths::sign_in_record(&dir.0);
    std::fs::write(
        &record,
        format!(
            r#"{{"last":{},"failures":1,"paused":"The account is locked or blocked: x","credentials_paused":true,"manual_failures":0}}"#,
            T0 - 7200
        ),
    )
    .unwrap();

    for trigger in ["manual", "startup", "browser"] {
        let Err(LoginError::Paused(_)) = ensure_as(&state, Role::Cli, trigger).unwrap() else {
            panic!("the CLI's {trigger} request was not paused");
        };
    }
    assert_eq!(requests(&fake), 0, "the CLI cost Okta nothing");
    assert!(ensure_as(&state, Role::App, "startup").unwrap().is_err());

    assert_eq!(ensure_as(&state, Role::App, "manual").unwrap(), Ok(()));
    // The app's success lifted the pause for everyone.
    clock.advance(60);
    assert_eq!(ensure_as(&state, Role::Cli, "manual").unwrap(), Ok(()));
}

#[test]
fn three_failed_manual_requests_hold_the_next_one_until_credentials_are_saved() {
    let dir = Scratch::new("okta-three");
    let fake = fake();
    let (state, clock) = clocked(&dir, &fake);
    // A well-formed seed that is not the account's: every code is wrong.
    save(&state, USERNAME, PASSWORD, "JBSWY3DPEHPK3PXP").unwrap();
    for _ in 0..3 {
        let Err(LoginError::BadTotp(_)) = ensure(&state, "manual").unwrap() else {
            panic!("expected a rejected code");
        };
        clock.advance(60);
    }
    let before = requests(&fake);
    let Err(LoginError::Waiting(secs)) = ensure(&state, "manual").unwrap() else {
        panic!("a fourth manual attempt ran");
    };
    assert_eq!(secs, 6 * 3600 - 60);
    assert_eq!(requests(&fake), before);

    save_good(&state);
    assert_eq!(ensure(&state, "manual").unwrap(), Ok(()));
}

#[test]
fn a_signed_out_marker_stops_an_automatic_attempt_but_not_a_manual_one() {
    let dir = Scratch::new("okta-signed-out");
    let fake = fake();
    let state = state(&dir, &fake);
    save_good(&state);
    let marker = crate::paths::signed_out(&dir.0);
    std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
    std::fs::write(&marker, b"1").unwrap();
    assert!(matches!(
        ensure(&state, "startup").unwrap(),
        Err(LoginError::SignedOut)
    ));
    assert_eq!(requests(&fake), 0);
    assert!(ensure(&state, "manual").unwrap().is_ok());
}
