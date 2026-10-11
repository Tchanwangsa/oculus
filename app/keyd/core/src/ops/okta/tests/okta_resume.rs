use super::*;

const LOCKED_OUT: &str =
    r#"{"last":1000,"failures":3,"paused":"locked","credentials_paused":true,"manual_failures":3}"#;

fn plant_record(dir: &Scratch, text: &str) {
    let record = crate::paths::sign_in_record(&dir.0);
    std::fs::create_dir_all(record.parent().unwrap()).unwrap();
    std::fs::write(record, text).unwrap();
}

#[test]
fn the_app_resumes_the_attempt_guard_and_the_cli_may_not() {
    let dir = Scratch::new("okta-resume-op");
    let fake = fake();
    let state = state(&dir, &fake);
    save_good(&state);
    plant_record(&dir, LOCKED_OUT);

    for role in [Role::Cli, Role::Unknown] {
        let err = op(&state, &as_role(role), "okta_resume", json!({})).unwrap_err();
        assert_eq!(err.kind, "caller", "{role:?}");
    }
    assert_eq!(
        std::fs::read_to_string(crate::paths::sign_in_record(&dir.0)).unwrap(),
        LOCKED_OUT,
        "a refused caller changed nothing"
    );
    assert!(matches!(
        ensure(&state, "startup").unwrap(),
        Err(LoginError::Paused(_))
    ));

    let reply = op(&state, &as_role(Role::App), "okta_resume", json!({})).unwrap();
    assert_eq!(reply, json!({"resumed": true}));
    assert_eq!(ensure(&state, "startup").unwrap(), Ok(()));
}

#[test]
fn resuming_repairs_a_damaged_record_and_reports_one_it_cannot_replace() {
    let dir = Scratch::new("okta-resume-repair");
    let fake = fake();
    let state = state(&dir, &fake);
    save_good(&state);
    plant_record(&dir, "");
    assert!(matches!(
        ensure(&state, "startup").unwrap(),
        Err(LoginError::Paused(_))
    ));
    op(&state, &as_role(Role::App), "okta_resume", json!({})).unwrap();
    assert_eq!(ensure(&state, "startup").unwrap(), Ok(()));

    // A directory where the record belongs cannot be replaced.
    let dir = Scratch::new("okta-resume-fails");
    let state = state_with(&dir, None, Box::new(NoLegacy));
    std::fs::create_dir_all(crate::paths::sign_in_record(&dir.0)).unwrap();
    let err = op(&state, &as_role(Role::App), "okta_resume", json!({})).unwrap_err();
    assert_eq!(err.kind, "record");
    assert!(err.detail.contains("sign-in.json"), "{}", err.detail);
}
