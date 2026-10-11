use super::*;

#[test]
fn automatic_attempts_back_off_and_manual_ones_skip_the_backoff() {
    let mut r = AttemptRecord::default();
    attempt(&mut r, Trigger::Startup, T, bad_totp());
    assert!(matches!(
        admit(&mut r, Trigger::Browser, APP, T + 599),
        Err(LoginError::Waiting(1))
    ));
    attempt(
        &mut r,
        Trigger::Startup,
        T + 600,
        Err(LoginError::Unexpected(String::new())),
    );
    // Two failures in a row: an hour.
    assert!(matches!(
        admit(&mut r, Trigger::Startup, APP, T + 3000),
        Err(LoginError::Waiting(_))
    ));
    attempt(&mut r, Trigger::Manual, T + 3000, bad_totp());
    assert_eq!(wait_after(r.failures), 6 * 3600);
    settle(&mut r, Trigger::Manual, &ok());
    assert_eq!(r.failures, 0);
}

#[test]
fn a_network_failure_waits_without_counting() {
    let mut r = AttemptRecord::default();
    attempt(
        &mut r,
        Trigger::Startup,
        5_000,
        Err(LoginError::Network(String::new())),
    );
    assert_eq!((r.failures, r.manual_failures), (0, 0));
    assert!(matches!(
        admit(&mut r, Trigger::Startup, APP, 5_100),
        Err(LoginError::Waiting(500))
    ));
}

#[test]
fn a_lockout_pauses_automatic_sign_in_until_a_manual_success() {
    let mut r = AttemptRecord::default();
    attempt(
        &mut r,
        Trigger::Startup,
        10_000,
        Err(LoginError::Locked("Too many attempts".into())),
    );
    assert!(matches!(
        admit(&mut r, Trigger::Startup, APP, T),
        Err(LoginError::Paused(_))
    ));
    attempt(&mut r, Trigger::Manual, T, ok());
    assert!(r.paused.is_none());
    assert!(admit(&mut r, Trigger::Startup, APP, T + 600).is_ok());
}

// (a) Spacing.

#[test]
fn no_two_attempts_of_any_trigger_start_within_a_minute() {
    for first in ALL {
        for second in ALL {
            let mut r = AttemptRecord::default();
            attempt(&mut r, first, T, ok());
            let Err(LoginError::Waiting(secs)) = admit(&mut r, second, APP, T + 59) else {
                panic!("{second:?} right after {first:?} was admitted");
            };
            // A manual one is held for the minute; an automatic one for ten.
            assert_eq!(secs, if second == Trigger::Manual { 1 } else { 541 });
            assert_eq!(r.last, T, "a refusal is not an attempt");
            assert!(admit(&mut r, Trigger::Manual, APP, T + 60).is_ok());
        }
    }
}

#[test]
fn a_forward_attempt_is_automatic_in_every_rule() {
    // The back-off after a failure.
    let mut r = AttemptRecord::default();
    attempt(&mut r, Trigger::Forward, T, bad_totp());
    assert!(matches!(
        admit(&mut r, Trigger::Forward, APP, T + 599),
        Err(LoginError::Waiting(1))
    ));
    assert!(admit(&mut r, Trigger::Forward, CLI, T + 600).is_ok());

    // A success is waited out too.
    let mut r = AttemptRecord::default();
    attempt(&mut r, Trigger::Forward, T, ok());
    assert!(matches!(
        admit(&mut r, Trigger::Forward, APP, T + 59),
        Err(LoginError::Waiting(541))
    ));

    // A pause stops it for either role, and a manual attempt from the app lifts it.
    let mut r = AttemptRecord::default();
    attempt(
        &mut r,
        Trigger::Forward,
        T,
        Err(LoginError::Locked("Too many attempts".into())),
    );
    for role in [APP, CLI] {
        assert!(matches!(
            admit(&mut r, Trigger::Forward, role, T + 7 * 3600),
            Err(LoginError::Paused(_))
        ));
    }
    attempt(&mut r, Trigger::Manual, T + 7 * 3600, ok());
    assert!(admit(&mut r, Trigger::Forward, CLI, T + 8 * 3600).is_ok());

    // A damaged record refuses it.
    let mut r = AttemptRecord {
        damaged: Some("is damaged".into()),
        ..AttemptRecord::default()
    };
    assert!(matches!(
        admit(&mut r, Trigger::Forward, APP, T),
        Err(LoginError::Paused(_))
    ));
}

#[test]
fn back_to_back_manual_attempts_are_held_a_minute_for_either_role() {
    for role in [APP, CLI, Role::Unknown] {
        let mut r = AttemptRecord::default();
        admit(&mut r, Trigger::Manual, role, T).unwrap();
        for t in [T, T + 1, T + 30, T + 59] {
            assert!(matches!(
                admit(&mut r, Trigger::Manual, role, t),
                Err(LoginError::Waiting(_))
            ));
        }
        assert_eq!(r.last, T);
        assert!(admit(&mut r, Trigger::Manual, role, T + 60).is_ok());
    }
}

// (b) Three failed manual attempts in a row.

#[test]
fn three_failed_manual_attempts_hold_the_fourth_to_the_normal_backoff() {
    let mut r = AttemptRecord::default();
    for i in 0..2 {
        attempt(&mut r, Trigger::Manual, T + 60 * i, bad_totp());
    }
    // Two failures: the minute is all a manual attempt waits for.
    assert!(admit(&mut r, Trigger::Manual, APP, T + 120).is_ok());
    settle(&mut r, Trigger::Manual, &bad_totp());
    assert_eq!(r.manual_failures, 3);

    // Three failures: the 6 h wait automatic attempts have.
    let Err(LoginError::Waiting(secs)) = admit(&mut r, Trigger::Manual, APP, T + 180) else {
        panic!("a fourth manual attempt ran");
    };
    assert_eq!(secs, 6 * 3600 - 60);
    assert!(admit(&mut r, Trigger::Manual, APP, T + 120 + 6 * 3600 - 1).is_err());
    assert!(admit(&mut r, Trigger::Manual, APP, T + 120 + 6 * 3600).is_ok());
}

#[test]
fn a_success_or_saved_credentials_start_the_manual_count_over() {
    let held = || {
        let mut r = AttemptRecord::default();
        for i in 0..3 {
            attempt(&mut r, Trigger::Manual, T + 60 * i, bad_totp());
        }
        assert!(admit(&mut r, Trigger::Manual, APP, T + 600).is_err());
        r
    };

    let mut r = held();
    settle(&mut r, Trigger::Startup, &ok());
    assert_eq!(r.manual_failures, 0);
    assert!(admit(&mut r, Trigger::Manual, APP, T + 600).is_ok());

    let mut r = held();
    resume(&mut r);
    assert_eq!(r.manual_failures, 0);
    assert!(admit(&mut r, Trigger::Manual, APP, T + 600).is_ok());
}

#[test]
fn only_failed_manual_attempts_with_a_verdict_count_toward_the_three() {
    let mut r = AttemptRecord::default();
    attempt(&mut r, Trigger::Manual, T, bad_totp());
    attempt(
        &mut r,
        Trigger::Manual,
        T + 60,
        Err(LoginError::Network(String::new())),
    );
    attempt(&mut r, Trigger::Startup, T + 60 + 600, bad_totp());
    attempt(&mut r, Trigger::Browser, T + 60 + 600 + 3600, bad_totp());
    assert_eq!(r.manual_failures, 1);
    assert_eq!(r.failures, 3);
    // Automatic failures held back the automatic wait, not the manual count.
    assert!(admit(&mut r, Trigger::Manual, APP, T + 5000).is_ok());
}

#[test]
fn a_held_manual_attempt_stops_at_a_pause_like_an_automatic_one() {
    let mut r = AttemptRecord::default();
    for i in 0..3 {
        attempt(
            &mut r,
            Trigger::Manual,
            T + 60 * i,
            Err(LoginError::Locked("locked".into())),
        );
    }
    // The app could override the pause before; after three failures it cannot.
    assert!(matches!(
        admit(&mut r, Trigger::Manual, APP, T + 10 * 3600),
        Err(LoginError::Paused(_))
    ));
}

// (c) A lockout or a rejected password pauses the CLI out.

#[test]
fn a_lockout_or_a_rejected_password_lifts_only_for_a_manual_attempt_from_the_app() {
    for error in [
        LoginError::Locked("Too many attempts".into()),
        LoginError::BadPassword("Password is incorrect".into()),
    ] {
        let mut r = AttemptRecord::default();
        attempt(&mut r, Trigger::Startup, T, Err(error.clone()));
        for role in [CLI, Role::Unknown] {
            let Err(LoginError::Paused(why)) = admit(&mut r, Trigger::Manual, role, T + 120) else {
                panic!("{role:?} overrode {error}");
            };
            assert_eq!(why, error.to_string());
            assert_eq!(r.last, T, "a refusal is not an attempt");
        }
        assert!(admit(&mut r, Trigger::Manual, APP, T + 120).is_ok());
        // The app's attempt succeeds, and the pause is gone for everyone.
        settle(&mut r, Trigger::Manual, &ok());
        assert!(admit(&mut r, Trigger::Manual, CLI, T + 180).is_ok());
    }
}

#[test]
fn a_pause_for_a_factor_it_cannot_answer_lifts_for_a_manual_attempt_from_either() {
    let mut r = AttemptRecord::default();
    attempt(
        &mut r,
        Trigger::Startup,
        T,
        Err(LoginError::UnsupportedFactor(vec![])),
    );
    assert!(r.paused.is_some() && !r.credentials_paused);
    assert!(matches!(
        admit(&mut r, Trigger::Startup, APP, T + 3 * 3600),
        Err(LoginError::Paused(_))
    ));
    assert!(admit(&mut r, Trigger::Manual, CLI, T + 120).is_ok());
}

#[test]
fn saving_credentials_lifts_a_lockout_pause_for_everyone_but_keeps_the_spacing() {
    let mut r = AttemptRecord::default();
    attempt(
        &mut r,
        Trigger::Manual,
        T,
        Err(LoginError::Locked("locked".into())),
    );
    resume(&mut r);
    assert_eq!((r.failures, r.manual_failures), (0, 0));
    assert!(r.paused.is_none() && !r.credentials_paused);
    // The attempt a minute ago still counts for the spacing, for every trigger.
    for trigger in [Trigger::Manual, Trigger::Startup] {
        assert!(matches!(
            admit(&mut r, trigger, CLI, T + 59),
            Err(LoginError::Waiting(1))
        ));
    }
    assert!(admit(&mut r, Trigger::Startup, CLI, T + 60).is_ok());
}
