//! The Bearer token the embed player sends. Redeeming a launch's session id
//! answers 204 with the JWT in a `token` response header (not a cookie). One
//! token works for every link and lives about 13 hours.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

/// A Bearer is reused until this many seconds before it expires.
const MARGIN_SECS: u64 = 5 * 60;

#[derive(Debug, Clone, PartialEq)]
pub struct Bearer {
    /// `https://host` of the region that minted it, where it is sent.
    pub base: String,
    pub jwt: String,
    /// The JWT's `exp`, Unix seconds.
    pub exp: u64,
}

impl Bearer {
    /// Whether it is still worth sending at `now` (Unix seconds).
    pub fn usable_at(&self, now: u64) -> bool {
        now + MARGIN_SECS < self.exp
    }
}

/// `GET /api/ui/sessions/<sessionId>` at the launch's region, on its agent,
/// which carries its `PLAY_SESSION` cookie.
pub fn redeem(agent: &ureq::Agent, base: &str, session_id: &str) -> Result<Bearer, String> {
    let url = format!("{base}/api/ui/sessions/{session_id}");
    let resp = match agent.get(&url).call() {
        Ok(resp) => resp,
        Err(ureq::Error::Status(code, _)) => {
            return Err(format!(
                "Echo360 would not open a player session (HTTP {code}); \
                 its launch may have gone stale"
            ))
        }
        Err(e) => return Err(format!("Echo360 player session request failed: {e}")),
    };
    let jwt = resp
        .header("token")
        .filter(|t| !t.is_empty())
        .ok_or_else(|| {
            format!(
                "Echo360's player session (HTTP {}) carried no token",
                resp.status()
            )
        })?
        .to_string();
    let exp = jwt_exp(&jwt)?;
    Ok(Bearer {
        base: base.to_string(),
        jwt,
        exp,
    })
}

/// The `exp` claim of a JWT, read without checking its signature: it only
/// decides when to fetch a new one.
pub(super) fn jwt_exp(jwt: &str) -> Result<u64, String> {
    let payload = jwt
        .split('.')
        .nth(1)
        .ok_or("Echo360's token is not a JWT")?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .map_err(|e| format!("Echo360's token payload is not base64: {e}"))?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Echo360's token payload is not JSON: {e}"))?;
    claims["exp"]
        .as_u64()
        .or_else(|| claims["exp"].as_f64().map(|f| f as u64))
        .ok_or_else(|| "Echo360's token has no expiry".to_string())
}
