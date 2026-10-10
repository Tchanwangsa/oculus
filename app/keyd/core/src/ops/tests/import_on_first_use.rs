use super::forward_op::{answer, forward};
use super::*;

fn with_items(
    dir: &Scratch,
    items: Vec<(
        (&'static str, &'static str),
        Result<Option<String>, KeyError>,
    )>,
) -> (State, Reads) {
    let reads = Reads::default();
    let state = State::new(
        BUILD,
        dir.0.clone(),
        Box::new(StaticKey(key())),
        Box::new(OldItems::new(items, reads.clone())),
    );
    (state, reads)
}

/// One old Voyage item.
fn with_old(dir: &Scratch, old: Result<Option<String>, KeyError>) -> (State, Reads) {
    with_items(dir, vec![(("com.tchan.oculus.voyage", "voyage"), old)])
}

#[test]
fn the_old_item_is_imported_once_and_a_delete_is_not_undone() {
    let dir = Scratch::new("import");
    let (state, reads) = with_old(&dir, Ok(Some("pa-old".into())));
    assert_eq!(
        call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
        true
    );
    assert_eq!(
        call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
        true
    );
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
    let v = Vault::new(crate::paths::vault(&dir.0), key());
    assert_eq!(v.get("voyage").unwrap().as_deref(), Some("pa-old"));

    assert_eq!(
        call(&state, "delete", json!({"secret": "voyage"})).unwrap()["existed"],
        true
    );
    assert_eq!(
        call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
        false,
        "the old item stays gone"
    );

    // A fresh keyd over the same vault agrees.
    let (again, reads) = with_old(&dir, Ok(Some("pa-old".into())));
    assert_eq!(
        call(&again, "has", json!({"secret": "voyage"})).unwrap()["has"],
        false
    );
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(
        call(&again, "has", json!({"secret": "keyd.imported.voyage"})).is_err(),
        "the marker is no secret"
    );
}

#[test]
fn a_stored_key_outranks_the_old_item_and_skips_its_read() {
    let dir = Scratch::new("import-store");
    let (state, reads) = with_old(&dir, Ok(Some("pa-old".into())));
    call(
        &state,
        "store",
        json!({"secret": "voyage", "value": "pa-new"}),
    )
    .unwrap();
    assert_eq!(
        call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
        true
    );
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(
        Vault::new(crate::paths::vault(&dir.0), key())
            .get("voyage")
            .unwrap()
            .as_deref(),
        Some("pa-new")
    );
}

#[test]
fn no_old_item_is_imported_as_absent_and_not_read_again() {
    let dir = Scratch::new("import-none");
    let (state, reads) = with_old(&dir, Ok(None));
    assert_eq!(
        call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
        false
    );
    assert_eq!(
        call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
        false
    );
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn a_refused_old_item_is_a_keychain_error_and_is_tried_again() {
    let dir = Scratch::new("import-refused");
    let (state, reads) = with_old(
        &dir,
        Err(KeyError::Refused(
            "reading com.tchan.oculus.voyage/voyage: OSStatus -128".into(),
        )),
    );
    let err = call(&state, "has", json!({"secret": "voyage"})).unwrap_err();
    assert_eq!(err.kind, "keychain");
    assert!(err.detail.contains("-128"), "{}", err.detail);
    assert!(call(&state, "has", json!({"secret": "voyage"})).is_err());
    assert_eq!(
        reads.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "a refusal is not recorded as imported"
    );
}

#[test]
fn forward_imports_the_old_item_first() {
    let origin = FakeOrigin::start(|_| answer(200, vec![], b"{}"));
    let dir = Scratch::new("import-forward");
    let (state, _) = with_old(&dir, Ok(Some("pa-old".into())));
    let state = state.with_routes(
        Routes::compiled()
            .with_origin("voyage", &origin.origin)
            .unwrap(),
    );
    assert_eq!(forward(&state, b"{}").unwrap().header["status"], 200);
    assert_eq!(
        origin.hits()[0].header("authorization"),
        Some("Bearer pa-old")
    );
}

#[test]
fn mineru_and_groqs_old_items_are_imported_once() {
    let dir = Scratch::new("import-mineru-groq");
    let (state, reads) = with_items(
        &dir,
        vec![
            (
                ("com.tchan.oculus.mineru", "mineru"),
                Ok(Some("mineru-old".into())),
            ),
            (
                ("com.tchan.oculus.groq", "groq"),
                Ok(Some("gsk_old".into())),
            ),
        ],
    );
    for name in ["mineru", "groq", "mineru", "groq"] {
        assert_eq!(
            call(&state, "has", json!({"secret": name})).unwrap()["has"],
            true,
            "{name}"
        );
    }
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 2);
    let v = Vault::new(crate::paths::vault(&dir.0), key());
    assert_eq!(v.get("mineru").unwrap().as_deref(), Some("mineru-old"));
    assert_eq!(v.get("groq").unwrap().as_deref(), Some("gsk_old"));
}
