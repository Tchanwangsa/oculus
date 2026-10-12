//! The `site_set` op and the site every other op reads (`crate::site`).
//!
//! A site keyd was never given is UniMelb's, which every install from before
//! the op was. Moving Canvas's or Okta's origin is moving to another
//! university: every session, the authenticated flag and the Okta credentials
//! go with it, so nothing made for one origin is ever sent to another. Moving
//! only Ed's origin drops only Ed's token.

use serde_json::{json, Value};

use super::{OpError, Reply, State};
use crate::names;
use crate::session::{markers, store, Kind};
use crate::site::Site;
use crate::vault::Vault;

/// The site as `site_set` stored it, or UniMelb's.
fn stored(vault: &Vault) -> Result<Site, OpError> {
    let Some(raw) = vault.get(names::SITE)? else {
        return Ok(Site::unimelb());
    };
    serde_json::from_str::<Value>(&raw)
        .ok()
        .as_ref()
        .and_then(Site::from_wire)
        .ok_or_else(|| OpError::new("vault", "the stored site is unreadable"))
}

impl State {
    /// The site the routes and the sign-in use, with any origin a debug build
    /// moved (`with_origins`).
    pub(super) fn site(&self, vault: &Vault) -> Result<Site, OpError> {
        let mut site = stored(vault)?;
        if let Some(canvas) = &self.canvas_base {
            site.canvas = canvas.clone();
        }
        if let Some(sso) = &self.sso_base {
            site.sso = Some(sso.clone());
        }
        Ok(site)
    }

    /// `{"site": {canvas, login_path, sso, ed}}` in; `{"changed", "cleared",
    /// "forgot_okta"}` out, naming what the move dropped. It waits for a
    /// sign-in already running, which would otherwise save a session for the
    /// old site after this.
    pub(super) fn site_set(&self, req: &Value) -> Result<Reply, OpError> {
        let next = req.get("site").and_then(Site::from_wire).ok_or_else(|| {
            OpError::new(
                "request",
                "site_set needs a \"site\" with canvas, login_path, sso and ed",
            )
        })?;
        next.check().map_err(|why| OpError::new("request", why))?;
        self.flight.exclusively(|| {
            let vault = self.vault()?;
            self.import_sessions(&vault)?;
            let before = stored(&vault)?;
            let moved = before.canvas != next.canvas || before.sso != next.sso;
            let kinds: &[Kind] = if moved {
                &Kind::ALL
            } else if before.ed != next.ed {
                &[Kind::Ed]
            } else {
                &[]
            };
            let cleared = if kinds.is_empty() {
                Vec::new()
            } else {
                store::clear(&vault, &self.generation, kinds)?
            };
            let mut forgot_okta = false;
            if moved {
                markers::clear_authenticated(&self.data_dir)
                    .map_err(|e| OpError::new("record", format!("the authenticated flag: {e}")))?;
                forgot_okta = self.forget_okta(&vault)?.0;
            }
            vault.update(|e| e.insert(names::SITE, &next.to_wire().to_string()))?;
            let cleared: Vec<&str> = cleared.into_iter().map(Kind::wire).collect();
            Ok(Reply {
                note: Some(format!("moved={moved} cleared={}", cleared.join(","))),
                ..json!({"changed": before != next, "cleared": cleared, "forgot_okta": forgot_okta})
                    .into()
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{call, state_in};
    use super::*;
    use crate::test_support::Scratch;

    fn rmit() -> Site {
        Site {
            canvas: "https://rmit.instructure.com".to_string(),
            login_path: "/login".to_string(),
            sso: None,
            ed: "https://edstem.org".to_string(),
        }
    }

    fn set(state: &State, site: &Site) -> Result<Value, OpError> {
        call(state, "site_set", json!({"site": site.to_wire()}))
    }

    fn held(state: &State) -> Value {
        call(state, "session_status", json!({})).unwrap()
    }

    fn fill(state: &State) {
        for (kind, value) in [
            ("canvas", "canvas_session=a"),
            ("sso", "sid=b"),
            ("ed", "jwt"),
        ] {
            call(state, "session_put", json!({"kind": kind, "value": value})).unwrap();
        }
        call(state, "session_mark", json!({"authenticated": true})).unwrap();
        call(
            state,
            "okta_save",
            json!({"username": "u", "password": "p", "totp_secret": "JBSWY3DPEHPK3PXP"}),
        )
        .unwrap();
    }

    #[test]
    fn a_site_never_set_is_unimelbs_and_setting_it_again_drops_nothing() {
        let dir = Scratch::new("site-default");
        let state = state_in(&dir);
        assert_eq!(
            state.site(&state.vault().unwrap()).unwrap(),
            Site::unimelb()
        );
        fill(&state);
        let reply = set(&state, &Site::unimelb()).unwrap();
        assert_eq!(
            reply,
            json!({"changed": false, "cleared": [], "forgot_okta": false})
        );
        let status = held(&state);
        assert_eq!(
            (&status["canvas"], &status["authenticated"]),
            (&json!(true), &json!(true))
        );
    }

    #[test]
    fn moving_canvas_drops_every_session_the_flag_and_the_okta_credentials() {
        let dir = Scratch::new("site-move");
        let state = state_in(&dir);
        fill(&state);
        let reply = set(&state, &rmit()).unwrap();
        assert_eq!(
            reply,
            json!({"changed": true, "cleared": ["canvas", "sso", "ed"], "forgot_okta": true})
        );
        let status = held(&state);
        for field in ["canvas", "sso", "ed", "authenticated"] {
            assert_eq!(status[field], json!(false), "{field}");
        }
        let okta = call(&state, "okta_status", json!({})).unwrap();
        assert_eq!(okta["username"], Value::Null);
        assert_eq!(state.site(&state.vault().unwrap()).unwrap(), rmit());
    }

    #[test]
    fn moving_ed_drops_only_eds_token() {
        let dir = Scratch::new("site-ed");
        let state = state_in(&dir);
        fill(&state);
        let site = Site {
            ed: "https://us.edstem.org".to_string(),
            ..Site::unimelb()
        };
        let reply = set(&state, &site).unwrap();
        assert_eq!(
            reply,
            json!({"changed": true, "cleared": ["ed"], "forgot_okta": false})
        );
        let status = held(&state);
        assert_eq!(
            (&status["canvas"], &status["ed"]),
            (&json!(true), &json!(false))
        );
    }

    #[test]
    fn a_bad_site_is_refused_and_changes_nothing() {
        let dir = Scratch::new("site-bad");
        let state = state_in(&dir);
        for req in [
            json!({}),
            json!({"site": "https://evil.example"}),
            json!({"site": Site { canvas: "https://evil.example/x".into(), ..rmit() }.to_wire()}),
            json!({"site": Site { login_path: "//evil.example".into(), ..rmit() }.to_wire()}),
        ] {
            let err = call(&state, "site_set", req.clone()).unwrap_err();
            assert_eq!(err.kind, "request", "{req}");
        }
        assert_eq!(
            state.site(&state.vault().unwrap()).unwrap(),
            Site::unimelb()
        );
    }

    #[test]
    fn a_site_without_okta_answers_a_sign_in_with_not_okta() {
        let dir = Scratch::new("site-not-okta");
        let state = state_in(&dir);
        set(&state, &rmit()).unwrap();
        let reply = call(&state, "ensure_signed_in", json!({"trigger": "manual"})).unwrap();
        assert_eq!(reply["code"], "not_okta");
        assert_eq!(reply["detail"], "https://rmit.instructure.com");
    }

    #[test]
    fn a_non_okta_site_keeps_its_sessions_across_a_broker_restart_and_startup_site_set() {
        let dir = Scratch::new("site-not-okta-restart");
        {
            let state = state_in(&dir);
            set(&state, &rmit()).unwrap();
            fill(&state);
        }
        let state = state_in(&dir);
        let reply = set(&state, &rmit()).unwrap();
        assert_eq!(reply["changed"], false);
        assert_eq!(reply["cleared"], json!([]));
        let status = held(&state);
        for field in ["canvas", "sso", "ed", "authenticated"] {
            assert_eq!(status[field], true, "{field}");
        }
        assert_eq!(
            store::get(&state.vault().unwrap(), Kind::Canvas)
                .unwrap()
                .as_deref(),
            Some("canvas_session=a")
        );
    }
}
