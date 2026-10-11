use super::*;

#[test]
fn status_names_the_username_and_what_is_on_file_but_no_secret() {
    let dir = Scratch::new("okta-status");
    let fake = fake();
    let state = state(&dir, &fake);
    let empty = OktaStatus {
        username: None,
        has_password: false,
        has_totp: false,
    };
    assert_eq!(status(&state), empty);

    save_good(&state);
    let header = op(&state, &cli(), "okta_status", json!({})).unwrap();
    assert_eq!(
        header,
        json!({"username": USERNAME, "has_password": true, "has_totp": true})
    );
    let shown = header.to_string();
    assert!(!shown.contains(PASSWORD) && !shown.contains(SEED));

    vault_of(&dir).remove(names::OKTA_PASSWORD).unwrap();
    assert_eq!(
        status(&state),
        OktaStatus {
            username: Some(USERNAME.into()),
            has_password: false,
            has_totp: true,
        }
    );
}

#[test]
fn forget_removes_all_three_and_says_whether_any_was_there() {
    let dir = Scratch::new("okta-forget");
    let fake = fake();
    let state = state(&dir, &fake);
    save_good(&state);
    let forget = || op(&state, &cli(), "okta_forget", json!({})).unwrap();
    assert_eq!(forget(), json!({"existed": true, "legacy": "absent"}));
    assert_eq!(forget(), json!({"existed": false, "legacy": "absent"}));
    for name in names::OKTA {
        assert_eq!(stored(&dir, name), None);
        assert!(marked(&dir, name), "{name} keeps its marker");
    }
    assert!(matches!(
        ensure(&state, "manual").unwrap(),
        Err(LoginError::NotConfigured)
    ));
    // Forgetting one value alone still counts.
    save_good(&state);
    vault_of(&dir).remove(names::OKTA_USERNAME).unwrap();
    vault_of(&dir).remove(names::OKTA_PASSWORD).unwrap();
    assert_eq!(forget(), json!({"existed": true, "legacy": "absent"}));
}
