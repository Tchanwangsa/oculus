//! The `session_*` ops: the Canvas cookie, the Okta cookie and Ed's token,
//! which keyd holds. A session is used through `forward`; these hand one
//! over, drop it, or report it present.

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

/// Which sessions keyd holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionStatus {
    pub canvas: bool,
    pub sso: bool,
    pub ed: bool,
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
    /// come back. It leaves the sign-in markers and the attempt record alone.
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

    /// Which sessions keyd holds, never a value.
    pub fn session_status(&self) -> Result<SessionStatus, KeydError> {
        let (reply, _) = self.exchange(&json!({"op": "session_status"}), &[], Some(OP_TIMEOUT))?;
        let flag = |field: &str| reply.get(field).and_then(Value::as_bool);
        match (flag("canvas"), flag("sso"), flag("ed")) {
            (Some(canvas), Some(sso), Some(ed)) => Ok(SessionStatus { canvas, sso, ed }),
            _ => Err(KeydError::Broken("session_status: no answer".into())),
        }
    }
}
