//! The Web API token: Login5's access token for the live session, the same
//! account and client id the player uses, so Oculus needs no Client ID of its
//! own. Kept in memory for the session it came from and asked for again a
//! minute before it expires. Never stored, never sent to the frontend.

use std::time::{Duration, SystemTime};

use tauri::AppHandle;

use super::error::WebError;
use crate::music::state;

/// Ask for a new token this long before the current one expires.
const EARLY: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub(super) struct Cached {
    /// The `state::session` generation it was issued for.
    gen: u64,
    pub(super) access: String,
    expires_at: SystemTime,
}

impl Cached {
    pub(super) fn usable(&self, gen: u64, now: SystemTime) -> bool {
        self.gen == gen && now + EARLY < self.expires_at
    }
}

/// A token for the connected session: the cached one while it is usable,
/// else Login5's. `refused` is a token the Web API just answered 401 to;
/// Login5 handing that same one back fails rather than looping.
pub(super) async fn get(app: &AppHandle, refused: Option<&str>) -> Result<String, WebError> {
    let (session, gen) = state::session(app).ok_or(WebError::NotConnected)?;
    let web = state::web(app);
    if refused.is_none() {
        if let Some(cached) = web.token().filter(|c| c.usable(gen, SystemTime::now())) {
            return Ok(cached.access);
        }
    }
    web.set_token(None);
    let token = session
        .login5()
        .auth_token()
        .await
        .map_err(|e| WebError::Token(e.to_string()))?;
    if refused == Some(token.access_token.as_str()) {
        return Err(WebError::StillUnauthorized);
    }
    web.set_token(Some(Cached {
        gen,
        access: token.access_token.clone(),
        expires_at: token.timestamp + token.expires_in,
    }));
    Ok(token.access_token)
}

#[cfg(test)]
pub(super) fn cached(gen: u64, access: &str, expires_at: SystemTime) -> Cached {
    Cached {
        gen,
        access: access.into(),
        expires_at,
    }
}
