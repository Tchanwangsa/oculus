//! `get`: the one op that reads a stored value back, for the names in
//! `names::READABLE` only, and to the app alone (`require_role`). Spotify's
//! stored credentials are the case: librespot logs in with them itself, so
//! keyd has no request to attach them to the way `forward` attaches a key.

use serde_json::{json, Value};

use super::{secret_name, OpError, Reply, State};
use crate::names;

impl State {
    /// `{"value": string|null}`; null when nothing is stored.
    pub(super) fn get(&self, req: &Value) -> Result<Reply, OpError> {
        let name = secret_name(req)?;
        if !names::READABLE.contains(&name) {
            return Err(OpError::new(
                "request",
                format!("{name} is never read back"),
            ));
        }
        let value = self.vault()?.get(name)?.filter(|v| !v.is_empty());
        Ok(Reply {
            note: Some(format!("present={}", value.is_some())),
            ..json!({"value": value}).into()
        })
    }
}
