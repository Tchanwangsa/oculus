use super::*;

/// Slows the first request of a sign-in, so a second caller arrives while
/// the first is running.
fn slow(
    inner: impl Fn(&crate::test_support::Hit) -> crate::test_support::Answer + Send + 'static,
) -> FakeOrigin {
    FakeOrigin::start(move |hit| {
        if hit.method == "GET" && hit.path == "/login/saml" {
            std::thread::sleep(Duration::from_millis(600));
        }
        inner(hit)
    })
}

fn wait_for_a_request(fake: &FakeOrigin) {
    let started = std::time::Instant::now();
    while fake.hits().is_empty() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "no request came"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_caller_arriving_mid_sign_in_waits_and_gets_its_outcome_and_one_flow_runs() {
    let dir = Scratch::new("okta-flight");
    let fake = slow(script(code_from_t0));
    let state = Arc::new(state(&dir, &fake));
    save_good(&state);

    let first = {
        let s = state.clone();
        std::thread::spawn(move || ensure(&s, "startup").unwrap())
    };
    wait_for_a_request(&fake);
    // Without single flight this would be Waiting(600).
    let second = ensure(&state, "browser").unwrap();
    let first = first.join().unwrap();

    assert_eq!(first, Ok(()));
    assert_eq!(second, first);
    assert_eq!(requests(&fake), 11, "exactly one flow's worth of requests");
    let log = std::fs::read_to_string(crate::paths::sign_in_log(&dir.0)).unwrap();
    assert_eq!(log.lines().count(), 1, "{log}");

    // Arriving after it finished, a caller meets the guard.
    assert!(matches!(
        ensure(&state, "browser").unwrap(),
        Err(LoginError::Waiting(_))
    ));
    assert_eq!(requests(&fake), 11);
}

#[test]
fn waiters_share_a_failure_and_only_a_later_caller_sees_the_guards_wait() {
    let dir = Scratch::new("okta-flight-fail");
    let fake = slow(|_| answer(404, &[], "unexpected"));
    let state = Arc::new(state(&dir, &fake));
    save_good(&state);

    let spawn = |trigger: &'static str| {
        let s = state.clone();
        std::thread::spawn(move || ensure(&s, trigger).unwrap())
    };
    let first = spawn("startup");
    wait_for_a_request(&fake);
    let waiters = [spawn("browser"), spawn("startup")];
    let first = first.join().unwrap();
    let first_error = first.clone().unwrap_err();
    assert!(
        !matches!(first_error, LoginError::Waiting(_)),
        "{first_error}"
    );
    for waiter in waiters {
        assert_eq!(waiter.join().unwrap(), first);
    }
    let attempts = requests(&fake);

    assert!(matches!(
        ensure(&state, "browser").unwrap(),
        Err(LoginError::Waiting(_))
    ));
    assert_eq!(requests(&fake), attempts);
}

#[test]
fn a_sign_out_waits_for_the_sign_in_running_and_leaves_nothing_signed_in() {
    let dir = Scratch::new("okta-flight-sign-out");
    let fake = slow(script(code_from_t0));
    let state = Arc::new(state(&dir, &fake));
    save_good(&state);

    let first = {
        let s = state.clone();
        std::thread::spawn(move || ensure(&s, "startup").unwrap())
    };
    wait_for_a_request(&fake);
    op(&state, &cli(), "sign_out", json!({})).unwrap();
    assert_eq!(first.join().unwrap(), Ok(()));

    let held = op(&state, &cli(), "session_status", json!({})).unwrap();
    assert_eq!(held["canvas"], false, "{held}");
    assert_eq!(held["sso"], false, "{held}");
    assert_eq!(held["authenticated"], false, "{held}");
    assert_eq!(held["signed_out"], true, "{held}");
    assert!(matches!(
        ensure(&state, "browser").unwrap(),
        Err(LoginError::SignedOut)
    ));
}

#[test]
fn a_caller_arriving_during_a_sign_out_is_told_it_is_signed_out() {
    let flight = Arc::new(Flight::default());
    let (started, begun) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let out = {
        let f = flight.clone();
        std::thread::spawn(move || {
            f.exclusively(|| {
                started.send(()).unwrap();
                released.recv().unwrap();
            })
        })
    };
    begun.recv().unwrap();
    let waiter = {
        let f = flight.clone();
        std::thread::spawn(move || f.run(|| Ok("never".to_string())))
    };
    std::thread::sleep(Duration::from_millis(100));
    release.send(()).unwrap();
    out.join().unwrap();
    assert_eq!(waiter.join().unwrap(), Err(LoginError::SignedOut));
}

#[test]
fn a_sign_in_that_panics_fails_its_waiters_instead_of_hanging_them() {
    let flight = Arc::new(Flight::default());
    let first = {
        let f = flight.clone();
        std::thread::spawn(move || {
            f.run(|| {
                std::thread::sleep(Duration::from_millis(300));
                panic!("boom")
            })
        })
    };
    std::thread::sleep(Duration::from_millis(100));
    let waiter = flight.run(|| Ok("never".to_string()));
    assert!(
        matches!(waiter, Err(LoginError::Unexpected(_))),
        "{waiter:?}"
    );
    assert!(first.join().is_err());
    // The flight is usable again.
    assert_eq!(
        flight.run(|| Ok("again".to_string())),
        Ok("again".to_string())
    );
}
