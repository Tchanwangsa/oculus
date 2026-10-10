//! The session ops: the Canvas cookie, the Okta cookie and Ed's token, which
//! keyd holds in the vault (`crate::session`).
//!
//! `forward` is how a session is used. These ops are how one arrives (a
//! browser or login window handing over what it signed in with, an Ed token
//! the user pasted), is dropped, or is reported present. `session_get` is the
//! one op whose reply carries a cookie, and only the app may ask: it seeds
//! WebKit's cookie store for the in-app browser. The role check is
//! `require_role`'s. Only `session_mark` and `sign_out` touch the
//! authenticated flag and the signed-out marker; none touches the sign-in
//! attempt record. The first of these ops after keyd starts imports the
//! sessions an earlier version kept in files (`legacy.rs`).

use serde_json::{json, Value};

use super::{OpError, Reply, State};
use crate::session::{check_value, markers, store, Kind};

fn kind_of(text: &Value) -> Result<Kind, OpError> {
    text.as_str().and_then(Kind::parse).ok_or_else(|| {
        OpError::new(
            "request",
            "the session kind must be \"canvas\", \"sso\" or \"ed\"",
        )
    })
}

impl State {
    /// `{"canvas": cookie|null, "sso": cookie|null}`. Ed's token is never
    /// returned: nothing outside keyd needs it.
    pub(super) fn session_get(&self) -> Result<Reply, OpError> {
        let vault = self.vault()?;
        self.import_sessions(&vault)?;
        let canvas = store::get(&vault, Kind::Canvas)?;
        let sso = store::get(&vault, Kind::Sso)?;
        Ok(json!({"canvas": canvas, "sso": sso}).into())
    }

    /// Replaces the whole session `kind`. `value` rides in the header line.
    pub(super) fn session_put(&self, req: &Value) -> Result<Reply, OpError> {
        let kind = kind_of(req.get("kind").unwrap_or(&Value::Null))?;
        let value = req
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| OpError::new("request", "session_put needs a string \"value\""))?;
        check_value(value).map_err(|why| OpError::new("request", why))?;
        let vault = self.vault()?;
        self.import_sessions(&vault)?;
        store::put(&vault, &self.generation, kind, value)?;
        Ok(json!({"stored": true}).into())
    }

    /// Drops the sessions in `kinds` (all three when it is absent). The reply
    /// names the ones that were held.
    pub(super) fn session_clear(&self, req: &Value) -> Result<Reply, OpError> {
        let kinds = match req.get("kinds") {
            None | Some(Value::Null) => Kind::ALL.to_vec(),
            Some(Value::Array(list)) => list.iter().map(kind_of).collect::<Result<_, _>>()?,
            Some(_) => return Err(OpError::new("request", "kinds must be a list")),
        };
        let vault = self.vault()?;
        self.import_sessions(&vault)?;
        let cleared = store::clear(&vault, &self.generation, &kinds)?;
        let cleared: Vec<&str> = cleared.into_iter().map(Kind::wire).collect();
        Ok(json!({"cleared": cleared}).into())
    }

    /// Which sessions are held, and whether the app believes it is signed in
    /// (`authenticated`) or was signed out and not since (`signed_out`):
    /// booleans, never a value. `generation` counts the changes to a session
    /// since keyd started (`session::Generation`), so a caller can tell that
    /// something replaced one.
    pub(super) fn session_status(&self) -> Result<Reply, OpError> {
        let vault = self.vault()?;
        self.import_sessions(&vault)?;
        let held = store::held(&vault)?;
        Ok(json!({
            "canvas": held.contains(&Kind::Canvas),
            "sso": held.contains(&Kind::Sso),
            "ed": held.contains(&Kind::Ed),
            "authenticated": markers::is_authenticated(&self.data_dir),
            "signed_out": markers::is_signed_out(&self.data_dir),
            "generation": self.generation.get(),
        })
        .into())
    }

    /// `{"authenticated": bool}`: a person signed in (true: sets the flag and
    /// lifts the signed-out marker) or the app found its session dead (false:
    /// clears the flag only). The sessions themselves are `session_put`'s.
    pub(super) fn session_mark(&self, req: &Value) -> Result<Reply, OpError> {
        let on = req
            .get("authenticated")
            .and_then(Value::as_bool)
            .ok_or_else(|| {
                OpError::new("request", "session_mark needs a boolean \"authenticated\"")
            })?;
        if on {
            markers::mark_authenticated(&self.data_dir);
        } else {
            markers::clear_authenticated(&self.data_dir)
                .map_err(|e| OpError::new("record", format!("the authenticated flag: {e}")))?;
        }
        Ok(json!({"authenticated": on}).into())
    }

    /// Drops all three sessions and the authenticated flag, and stands every
    /// automatic sign-in down until a session is established again. The attempt
    /// record stays: forgetting a lockout pause would let automatic sign-in
    /// resume. `{"had": bool}` says whether there was anything to drop.
    pub(super) fn sign_out(&self) -> Result<Reply, OpError> {
        let vault = self.vault()?;
        self.import_sessions(&vault)?;
        let cleared = store::clear(&vault, &self.generation, &Kind::ALL)?;
        let flagged = markers::clear_authenticated(&self.data_dir)
            .map_err(|e| OpError::new("record", format!("the authenticated flag: {e}")))?;
        markers::mark_signed_out(&self.data_dir)
            .map_err(|e| OpError::new("record", format!("the signed-out marker: {e}")))?;
        Ok(json!({"had": flagged || !cleared.is_empty()}).into())
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{as_role, call, cli, key, state_in};
    use super::*;
    use crate::names;
    use crate::platform::Role;
    use crate::test_support::{Scratch, BUILD};
    use crate::vault::{KeyError, KeySource, MasterKey, NoLegacy, Vault};

    const CANVAS: &str = "canvas_session=SECRET-CANVAS==; _csrf_token=SECRET-CSRF";

    fn put(state: &State, kind: &str, value: &str) -> Result<Value, OpError> {
        call(state, "session_put", json!({"kind": kind, "value": value}))
    }

    fn vault(dir: &Scratch) -> Vault {
        Vault::new(crate::paths::vault(&dir.0), key())
    }

    #[test]
    fn put_status_get_and_clear_round_trip() {
        let dir = Scratch::new("session-ops");
        let state = state_in(&dir);
        let none = call(&state, "session_status", json!({})).unwrap();
        assert_eq!(
            none,
            json!({"canvas": false, "sso": false, "ed": false, "authenticated": false, "signed_out": false, "generation": 0})
        );

        put(&state, "canvas", CANVAS).unwrap();
        put(&state, "sso", "sid=SECRET-SSO").unwrap();
        put(&state, "ed", "SECRET.ED.JWT").unwrap();
        let status = call(&state, "session_status", json!({})).unwrap();
        assert_eq!(
            status,
            json!({"canvas": true, "sso": true, "ed": true, "authenticated": false, "signed_out": false, "generation": 3})
        );

        let app = as_role(Role::App);
        let got = state
            .dispatch(&app, "session_get", &json!({}), b"")
            .unwrap()
            .header;
        assert_eq!(got, json!({"canvas": CANVAS, "sso": "sid=SECRET-SSO"}));

        // A put replaces the whole value.
        put(&state, "canvas", "canvas_session=NEXT").unwrap();
        let got = state
            .dispatch(&app, "session_get", &json!({}), b"")
            .unwrap()
            .header;
        assert_eq!(got["canvas"], "canvas_session=NEXT");

        let cleared = call(&state, "session_clear", json!({"kinds": ["sso", "ed"]})).unwrap();
        assert_eq!(cleared, json!({"cleared": ["sso", "ed"]}));
        let status = call(&state, "session_status", json!({})).unwrap();
        assert_eq!(status["canvas"], true);
        assert_eq!(
            (status["sso"].clone(), status["ed"].clone()),
            (json!(false), json!(false))
        );

        let cleared = call(&state, "session_clear", json!({})).unwrap();
        assert_eq!(cleared, json!({"cleared": ["canvas"]}));
        let got = state
            .dispatch(&app, "session_get", &json!({}), b"")
            .unwrap()
            .header;
        assert_eq!(got, json!({"canvas": null, "sso": null}));
    }

    #[test]
    fn only_the_app_may_read_a_cookie_back_and_no_other_reply_carries_one() {
        let dir = Scratch::new("session-roles");
        let state = state_in(&dir);
        put(&state, "canvas", CANVAS).unwrap();
        put(&state, "ed", "SECRET.ED.JWT").unwrap();

        for role in [Role::Cli, Role::Unknown] {
            let err = state
                .dispatch(&as_role(role), "session_get", &json!({}), b"")
                .unwrap_err();
            assert_eq!(err.kind, "caller", "{role:?}");
            assert!(!err.detail.contains("SECRET"), "{}", err.detail);
        }
        assert!(state
            .dispatch(&as_role(Role::App), "session_get", &json!({}), b"")
            .is_ok());

        for (op, req) in [
            ("session_status", json!({})),
            ("session_put", json!({"kind": "canvas", "value": CANVAS})),
            ("has", json!({"secret": "session.canvas"})),
            ("session_clear", json!({"kinds": ["sso"]})),
        ] {
            for role in [Role::App, Role::Cli] {
                let reply = state.dispatch(&as_role(role), op, &req, b"").unwrap();
                let shown = format!("{:?}", reply);
                assert!(!shown.contains("SECRET"), "{op}: {shown}");
            }
        }
        let (a, b) = (
            call(&state, "has", json!({"secret": "session.canvas"})).unwrap(),
            call(&state, "has", json!({"secret": "session.ed"})).unwrap(),
        );
        assert_eq!(
            (a["has"].as_bool(), b["has"].as_bool()),
            (Some(true), Some(true))
        );
    }

    #[test]
    fn the_generic_ops_never_write_a_session() {
        let dir = Scratch::new("session-generic");
        let state = state_in(&dir);
        put(&state, "canvas", CANVAS).unwrap();
        for name in names::SESSIONS {
            let stored = state.dispatch(
                &cli(),
                "store",
                &json!({"secret": name, "value": "x=forged"}),
                b"",
            );
            assert_eq!(stored.unwrap_err().kind, "request", "{name}");
            let deleted = state.dispatch(&cli(), "delete", &json!({"secret": name}), b"");
            assert_eq!(deleted.unwrap_err().kind, "request", "{name}");
        }
        assert_eq!(
            vault(&dir).get("session.canvas").unwrap().as_deref(),
            Some(CANVAS)
        );
        // The Okta names stay refused too.
        for name in names::OKTA {
            assert!(state
                .dispatch(&cli(), "delete", &json!({"secret": name}), b"")
                .is_err());
        }
    }

    #[test]
    fn a_bad_put_is_refused_before_the_master_key_is_read() {
        struct Refusing;
        impl KeySource for Refusing {
            fn get_or_create(&self) -> Result<MasterKey, KeyError> {
                Err(KeyError::Refused("no".into()))
            }
        }
        let dir = Scratch::new("session-bad");
        let state = State::new(BUILD, dir.0.clone(), Box::new(Refusing), Box::new(NoLegacy));
        for req in [
            json!({"kind": "canvas"}),
            json!({"kind": "canvas", "value": 5}),
            json!({"kind": "canvas", "value": ""}),
            json!({"kind": "canvas", "value": "a=1\r\nHost: evil"}),
            json!({"kind": "canvas", "value": "a=\u{fc}"}),
            json!({"kind": "canvas", "value": "a".repeat(crate::session::MAX_VALUE + 1)}),
            json!({"kind": "session.canvas", "value": "a=1"}),
            json!({"kind": "Canvas", "value": "a=1"}),
            json!({"kind": null, "value": "a=1"}),
            json!({"value": "a=1"}),
        ] {
            let err = state
                .dispatch(&cli(), "session_put", &req, b"")
                .unwrap_err();
            assert_eq!(err.kind, "request", "{req}");
            assert!(!err.detail.contains("evil"), "{}", err.detail);
        }
        for req in [
            json!({"kinds": "canvas"}),
            json!({"kinds": ["canvas", "nope"]}),
            json!({"kinds": [5]}),
        ] {
            let err = state
                .dispatch(&cli(), "session_clear", &req, b"")
                .unwrap_err();
            assert_eq!(err.kind, "request", "{req}");
        }
        assert!(!crate::paths::vault(&dir.0).exists());
    }

    #[test]
    fn every_change_bumps_the_generation_and_a_read_does_not() {
        let dir = Scratch::new("session-generation");
        let state = state_in(&dir);
        assert_eq!(state.session_generation(), 0);
        put(&state, "canvas", CANVAS).unwrap();
        assert_eq!(state.session_generation(), 1);
        put(&state, "ed", "t").unwrap();
        assert_eq!(state.session_generation(), 2);
        call(&state, "session_status", json!({})).unwrap();
        state
            .dispatch(&as_role(Role::App), "session_get", &json!({}), b"")
            .unwrap();
        call(&state, "has", json!({"secret": "session.canvas"})).unwrap();
        assert_eq!(state.session_generation(), 2);
        call(&state, "session_clear", json!({"kinds": ["ed"]})).unwrap();
        assert_eq!(state.session_generation(), 3);
        assert!(put(&state, "canvas", "").is_err());
        assert_eq!(
            state.session_generation(),
            3,
            "a refused put changes nothing"
        );
    }

    #[test]
    fn session_mark_sets_and_clears_the_flag_and_a_sign_in_by_a_person_lifts_the_sign_out() {
        let dir = Scratch::new("session-mark");
        let state = state_in(&dir);
        let status = |s: &State| {
            let v = call(s, "session_status", json!({})).unwrap();
            (v["authenticated"].clone(), v["signed_out"].clone())
        };
        assert_eq!(status(&state), (json!(false), json!(false)));
        assert_eq!(
            call(&state, "session_mark", json!({"authenticated": true})).unwrap(),
            json!({"authenticated": true})
        );
        assert_eq!(status(&state), (json!(true), json!(false)));
        call(&state, "session_mark", json!({"authenticated": false})).unwrap();
        assert_eq!(status(&state), (json!(false), json!(false)));
        // Clearing a flag that is not there is not an error.
        call(&state, "session_mark", json!({"authenticated": false})).unwrap();

        call(&state, "sign_out", json!({})).unwrap();
        assert_eq!(status(&state), (json!(false), json!(true)));
        call(&state, "session_mark", json!({"authenticated": false})).unwrap();
        assert_eq!(
            status(&state),
            (json!(false), json!(true)),
            "marking the session dead is not a sign-in"
        );
        call(&state, "session_mark", json!({"authenticated": true})).unwrap();
        assert_eq!(status(&state), (json!(true), json!(false)));

        for bad in [
            json!({}),
            json!({"authenticated": "yes"}),
            json!({"authenticated": null}),
        ] {
            let err = state
                .dispatch(&cli(), "session_mark", &bad, b"")
                .unwrap_err();
            assert_eq!(err.kind, "request", "{bad}");
        }
    }

    #[test]
    fn sign_out_drops_every_session_and_the_flag_and_keeps_the_attempt_record() {
        let dir = Scratch::new("session-sign-out");
        let state = state_in(&dir);
        put(&state, "canvas", CANVAS).unwrap();
        put(&state, "sso", "sid=x").unwrap();
        put(&state, "ed", "jwt").unwrap();
        call(&state, "session_mark", json!({"authenticated": true})).unwrap();
        std::fs::write(
            crate::paths::sign_in_record(&dir.0),
            r#"{"paused":"locked"}"#,
        )
        .unwrap();
        let before = state.session_generation();

        assert_eq!(
            call(&state, "sign_out", json!({})).unwrap(),
            json!({"had": true})
        );
        let status = call(&state, "session_status", json!({})).unwrap();
        assert_eq!(
            (
                status["canvas"].clone(),
                status["sso"].clone(),
                status["ed"].clone()
            ),
            (json!(false), json!(false), json!(false))
        );
        assert_eq!(
            (
                status["authenticated"].clone(),
                status["signed_out"].clone()
            ),
            (json!(false), json!(true))
        );
        assert_eq!(status["generation"], before + 1);
        assert_eq!(
            std::fs::read_to_string(crate::paths::sign_in_record(&dir.0)).unwrap(),
            r#"{"paused":"locked"}"#
        );
        assert!(state.session_generation() > before);

        // Nothing left: the answer says so, and the marker stands.
        assert_eq!(
            call(&state, "sign_out", json!({})).unwrap(),
            json!({"had": false})
        );
        assert!(crate::paths::signed_out(&dir.0).exists());
        // Only the flag counts as something to drop, too.
        call(&state, "session_mark", json!({"authenticated": true})).unwrap();
        assert_eq!(
            call(&state, "sign_out", json!({})).unwrap(),
            json!({"had": true})
        );
    }

    #[test]
    fn clearing_sessions_leaves_the_sign_in_markers_and_record_alone() {
        let dir = Scratch::new("session-markers");
        let state = state_in(&dir);
        put(&state, "canvas", CANVAS).unwrap();
        std::fs::create_dir_all(crate::paths::session_dir(&dir.0)).unwrap();
        std::fs::write(crate::paths::signed_out(&dir.0), "").unwrap();
        std::fs::write(crate::paths::sign_in_record(&dir.0), "{\"x\":1}").unwrap();
        call(&state, "session_clear", json!({})).unwrap();
        assert!(crate::paths::signed_out(&dir.0).exists());
        assert_eq!(
            std::fs::read_to_string(crate::paths::sign_in_record(&dir.0)).unwrap(),
            "{\"x\":1}"
        );
    }

    #[test]
    fn a_session_is_sealed_in_the_vault_file() {
        let dir = Scratch::new("session-sealed");
        let state = state_in(&dir);
        put(&state, "canvas", CANVAS).unwrap();
        let raw = std::fs::read(crate::paths::vault(&dir.0)).unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("SECRET-CANVAS"));
        assert!(!raw.windows(13).any(|w| w == b"SECRET-CANVAS"));
    }
}
