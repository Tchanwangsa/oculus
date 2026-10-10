use super::*;

const SERVICE: &str = "com.oculus.unimelb-sso";

fn old_items(password: Result<Option<String>, KeyError>) -> (Box<OldItems>, Reads) {
    let reads = Reads::default();
    let items = OldItems::new(
        vec![
            ((SERVICE, "username"), Ok(Some(USERNAME.to_string()))),
            ((SERVICE, "password"), password),
            ((SERVICE, "totp_secret"), Ok(Some(SEED.to_string()))),
        ],
        reads.clone(),
    );
    (Box::new(items), reads)
}

#[test]
fn status_imports_the_three_old_items_once_and_marks_them() {
    let dir = Scratch::new("okta-import-status");
    let (items, reads) = old_items(Ok(Some(PASSWORD.into())));
    let state = state_with(&dir, None, items);
    assert_eq!(
        status(&state),
        OktaStatus {
            username: Some(USERNAME.into()),
            has_password: true,
            has_totp: true,
        }
    );
    assert_eq!(
        reads.load(Ordering::SeqCst),
        3,
        "one read, so one prompt, per item"
    );
    status(&state);
    assert_eq!(reads.load(Ordering::SeqCst), 3);
    assert_eq!(
        stored(&dir, names::OKTA_PASSWORD).as_deref(),
        Some(PASSWORD)
    );
    for name in names::OKTA {
        assert!(marked(&dir, name), "{name}");
    }
}

#[test]
fn a_forget_is_never_undone_by_a_later_import_even_in_a_new_keyd() {
    let dir = Scratch::new("okta-import-forget");
    let (items, _) = old_items(Ok(Some(PASSWORD.into())));
    let state = state_with(&dir, None, items);
    status(&state);
    op(&state, &cli(), "okta_forget", json!({})).unwrap();

    let (items, reads) = old_items(Ok(Some(PASSWORD.into())));
    let fresh = state_with(&dir, None, items);
    assert_eq!(status(&fresh).username, None);
    assert_eq!(
        reads.load(Ordering::SeqCst),
        0,
        "the old items are not even read"
    );
    assert!(matches!(
        ensure(&fresh, "manual").unwrap(),
        Err(LoginError::NotConfigured)
    ));
}

#[test]
fn forgetting_before_any_import_stops_the_import() {
    let dir = Scratch::new("okta-forget-first");
    let (items, reads) = old_items(Ok(Some(PASSWORD.into())));
    let state = state_with(&dir, None, items);
    op(&state, &cli(), "okta_forget", json!({})).unwrap();
    assert_eq!(status(&state).username, None);
    assert_eq!(reads.load(Ordering::SeqCst), 0);
}

#[test]
fn a_save_outranks_the_old_items_and_skips_their_read() {
    let dir = Scratch::new("okta-import-save");
    let (items, reads) = old_items(Ok(Some("the-old-password".into())));
    let state = state_with(&dir, None, items);
    save(&state, "new-user", "new-pw", SEED).unwrap();
    assert_eq!(status(&state).username.as_deref(), Some("new-user"));
    assert_eq!(
        stored(&dir, names::OKTA_PASSWORD).as_deref(),
        Some("new-pw")
    );
    assert_eq!(reads.load(Ordering::SeqCst), 0);
}

#[test]
fn ensure_signed_in_imports_the_old_items_then_signs_in_with_them() {
    let dir = Scratch::new("okta-import-ensure");
    let fake = fake();
    let (items, reads) = old_items(Ok(Some(PASSWORD.into())));
    let state = state_with(&dir, Some(&fake), items);
    assert_eq!(ensure(&state, "startup").unwrap(), Ok(()));
    assert_eq!(reads.load(Ordering::SeqCst), 3);
    assert_eq!(
        stored(&dir, names::OKTA_USERNAME).as_deref(),
        Some(USERNAME)
    );
}

#[test]
fn a_refused_old_item_is_a_keychain_error_for_status_and_unreadable_for_a_sign_in() {
    let dir = Scratch::new("okta-import-refused");
    let fake = fake();
    let refused = Err(KeyError::Refused("reading password: OSStatus -128".into()));
    let (items, reads) = old_items(refused);
    let state = state_with(&dir, Some(&fake), items);

    let err = op(&state, &cli(), "okta_status", json!({})).unwrap_err();
    assert_eq!(err.kind, "keychain");
    assert!(err.detail.contains("-128"), "{}", err.detail);

    let Err(LoginError::UnreadableCredentials(why)) = ensure(&state, "manual").unwrap() else {
        panic!("a refused read is unreadable, never not-configured");
    };
    assert!(why.contains("-128"), "{why}");
    assert_eq!(requests(&fake), 0);
    assert!(
        reads.load(Ordering::SeqCst) >= 2,
        "a refusal is retried, not recorded"
    );
    // No attempt was made, so none was recorded.
    assert!(!crate::paths::sign_in_record(&dir.0).exists());
}
