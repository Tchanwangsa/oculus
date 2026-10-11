use super::*;

fn good_requests() -> Vec<(&'static str, Value)> {
    vec![
        (
            "okta_save",
            json!({"username": USERNAME, "password": PASSWORD, "totp_secret": SEED}),
        ),
        ("okta_forget", json!({})),
        ("okta_status", json!({})),
        ("ensure_signed_in", json!({"trigger": "manual"})),
    ]
}

#[test]
fn an_unknown_role_is_refused_by_every_okta_op_and_nothing_runs() {
    let dir = Scratch::new("okta-role");
    let fake = fake();
    let state = state(&dir, &fake);
    for (name, req) in good_requests() {
        let err = op(&state, &as_role(Role::Unknown), name, req).unwrap_err();
        assert_eq!(err.kind, "caller", "{name}");
        assert!(!err.detail.contains(PASSWORD), "{name}");
    }
    assert!(
        !crate::paths::vault(&dir.0).exists(),
        "no op touched the vault"
    );
    assert!(!crate::paths::sign_in_record(&dir.0).exists());
    assert_eq!(requests(&fake), 0);
}

#[test]
fn the_app_and_the_cli_are_both_admitted() {
    for role in [Role::App, Role::Cli] {
        let dir = Scratch::new("okta-admit");
        let fake = fake();
        let state = state(&dir, &fake);
        for (name, req) in good_requests() {
            // A manual sign-in reaches the scripted servers and succeeds.
            op(&state, &as_role(role), name, req)
                .unwrap_or_else(|e| panic!("{role:?} {name}: {e:?}"));
        }
    }
}

#[test]
fn store_and_delete_refuse_the_okta_names_and_has_still_answers() {
    let dir = Scratch::new("okta-generic");
    let fake = fake();
    let state = state(&dir, &fake);
    for name in names::OKTA {
        let err = op(
            &state,
            &cli(),
            "store",
            json!({"secret": name, "value": "x"}),
        )
        .unwrap_err();
        assert_eq!(err.kind, "request", "{name}");
        assert!(err.detail.contains("okta_save"), "{}", err.detail);
        let err = op(&state, &cli(), "delete", json!({"secret": name})).unwrap_err();
        assert_eq!(err.kind, "request", "{name}");
        assert!(err.detail.contains("okta_forget"), "{}", err.detail);
        assert_eq!(
            op(&state, &cli(), "has", json!({"secret": name})).unwrap(),
            json!({"has": false})
        );
    }
    assert!(
        !crate::paths::vault(&dir.0).exists()
            || vault_of(&dir)
                .load()
                .unwrap()
                .names()
                .all(|n| n.starts_with("keyd.")),
        "no okta value was written"
    );
    save_good(&state);
    assert_eq!(
        op(&state, &cli(), "has", json!({"secret": "okta.password"})).unwrap(),
        json!({"has": true})
    );
    // A caller of no role may not even ask whether a value exists.
    for name in ["okta.password", "voyage"] {
        let err = op(
            &state,
            &as_role(Role::Unknown),
            "has",
            json!({"secret": name}),
        )
        .unwrap_err();
        assert_eq!(err.kind, "caller", "{name}");
    }
}
