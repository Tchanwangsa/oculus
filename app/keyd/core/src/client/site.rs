//! `site_set`: which university keyd serves (`crate::site`).

use serde_json::{json, Value};

use super::{Client, KeydError, OP_TIMEOUT};
use crate::session::Kind;
use crate::site::Site;

/// What a `site_set` dropped because an origin moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteChange {
    pub changed: bool,
    pub cleared: Vec<Kind>,
    pub forgot_okta: bool,
}

impl Client {
    /// Points the `canvas` and `ed` routes and the sign-in at `site`. Moving
    /// Canvas or Okta drops every session, the authenticated flag and the Okta
    /// credentials; moving only Ed drops Ed's token.
    pub fn site_set(&self, site: &Site) -> Result<SiteChange, KeydError> {
        let (reply, _) = self.exchange(
            &json!({"op": "site_set", "site": site.to_wire()}),
            &[],
            Some(OP_TIMEOUT),
        )?;
        let broken = || KeydError::Broken("site_set: no answer".into());
        let cleared = reply
            .get("cleared")
            .and_then(Value::as_array)
            .ok_or_else(broken)?
            .iter()
            .map(|k| k.as_str().and_then(Kind::parse))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(broken)?;
        Ok(SiteChange {
            changed: reply
                .get("changed")
                .and_then(Value::as_bool)
                .ok_or_else(broken)?,
            cleared,
            forgot_okta: reply
                .get("forgot_okta")
                .and_then(Value::as_bool)
                .ok_or_else(broken)?,
        })
    }
}
