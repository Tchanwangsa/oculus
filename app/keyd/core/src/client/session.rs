//! The `session_*` ops and `sign_out`: the Canvas cookie, the Okta cookie and
//! Ed's token, which keyd holds. A session is used through `forward`; these
//! hand one over, drop it, report it present, or set the markers beside it.

use std::fmt;

use serde_json::{json, Value};

use super::{Client, KeydError, OP_TIMEOUT};
use crate::session::{check_value, Kind};

/// The two cookie headers keyd returns to the app, which seeds WebKit with
/// them. Debug prints which are present, never a value.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionCookies {
    pub canvas: Option<String>,
    pub sso: Option<String>,
}

impl fmt::Debug for SessionCookies {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionCookies")
            .field("canvas", &self.canvas.is_some())
            .field("sso", &self.sso.is_some())
            .finish()
    }
}

/// Which sessions keyd holds, and the two markers beside them: `authenticated`
/// (the app believes it is signed in, which its startup probe reads) and
/// `signed_out` (automatic sign-in is off until a session is established).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionStatus {
    pub canvas: bool,
    pub sso: bool,
    pub ed: bool,
    pub authenticated: bool,
    pub signed_out: bool,
}

impl SessionStatus {
    pub fn has(&self, kind: Kind) -> bool {
        match kind {
            Kind::Canvas => self.canvas,
            Kind::Sso => self.sso,
            Kind::Ed => self.ed,
        }
    }
}

fn text(reply: &Value, field: &str) -> Option<String> {
    reply.get(field)?.as_str().map(str::to_string)
}

impl Client {
    /// The Canvas and Okta cookie headers. App only: any other caller gets
    /// `KeydError::Caller`. The one call that returns a cookie.
    pub fn session_get(&self) -> Result<SessionCookies, KeydError> {
        let (reply, _) = self.exchange(&json!({"op": "session_get"}), &[], Some(OP_TIMEOUT))?;
        Ok(SessionCookies {
            canvas: text(&reply, "canvas"),
            sso: text(&reply, "sso"),
        })
    }

    /// Replaces the whole session `kind`: a cookie header for Canvas or Okta,
    /// the token for Ed. A value keyd could not carry (empty, over
    /// `session::MAX_VALUE`, or holding a control or non-ASCII character) is
    /// `KeydError::Request`, without a round trip.
    pub fn session_put(&self, kind: Kind, value: &str) -> Result<(), KeydError> {
        check_value(value).map_err(KeydError::Request)?;
        self.exchange(
            &json!({"op": "session_put", "kind": kind.wire(), "value": value}),
            &[],
            Some(OP_TIMEOUT),
        )
        .map(|_| ())
    }

    /// Drops `kinds` (`Kind::ALL` for every session); the ones that were held
    /// come back. It leaves the markers and the attempt record alone: a
    /// sign-out is `sign_out`.
    pub fn session_clear(&self, kinds: &[Kind]) -> Result<Vec<Kind>, KeydError> {
        let wire: Vec<&str> = kinds.iter().map(|k| k.wire()).collect();
        let (reply, _) = self.exchange(
            &json!({"op": "session_clear", "kinds": wire}),
            &[],
            Some(OP_TIMEOUT),
        )?;
        Ok(reply
            .get("cleared")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|k| Kind::parse(k.as_str()?))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Which sessions keyd holds and the two markers, never a value.
    pub fn session_status(&self) -> Result<SessionStatus, KeydError> {
        let (reply, _) = self.exchange(&json!({"op": "session_status"}), &[], Some(OP_TIMEOUT))?;
        let flag = |field: &str| reply.get(field).and_then(Value::as_bool);
        match (
            flag("canvas"),
            flag("sso"),
            flag("ed"),
            flag("authenticated"),
            flag("signed_out"),
        ) {
            (Some(canvas), Some(sso), Some(ed), Some(authenticated), Some(signed_out)) => {
                Ok(SessionStatus {
                    canvas,
                    sso,
                    ed,
                    authenticated,
                    signed_out,
                })
            }
            _ => Err(KeydError::Broken("session_status: no answer".into())),
        }
    }

    /// A person signed in (`true`: sets the authenticated flag and lifts the
    /// signed-out marker) or the app found its session dead (`false`: clears
    /// the flag). Call it after `session_put`ting the cookies a browser or
    /// login window signed in with; keyd's own sign-ins mark it themselves.
    pub fn session_mark(&self, authenticated: bool) -> Result<(), KeydError> {
        self.exchange(
            &json!({"op": "session_mark", "authenticated": authenticated}),
            &[],
            Some(OP_TIMEOUT),
        )
        .map(|_| ())
    }

    /// Drops every session and the authenticated flag, and stands automatic
    /// sign-in down until a session is established again. The attempt record
    /// stays. True when there was anything to drop.
    pub fn sign_out(&self) -> Result<bool, KeydError> {
        let (reply, _) = self.exchange(&json!({"op": "sign_out"}), &[], Some(OP_TIMEOUT))?;
        reply
            .get("had")
            .and_then(Value::as_bool)
            .ok_or_else(|| KeydError::Broken("sign_out: no answer".into()))
    }
}
