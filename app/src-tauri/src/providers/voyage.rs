//! Voyage AI credential storage.
//!
//! The key lives only in the macOS keychain, never in SQLite or the WebView.
//! Same shape as `mineru.rs`: three commands, an `"ok"`/`"unverified"` answer.

use std::time::Duration;

use crate::providers::credentials::{Secret, Verdict};

const KEY: Secret = Secret::new("com.tchan.oculus.voyage", "voyage");

/// The cheapest authenticated call Voyage has: Voyage has no free
/// authenticated GET, so the probe embeds a two-letter string (one text token).
const PROBE_URL: &str = "https://api.voyageai.com/v1/embeddings";
const PROBE_BODY: &str = r#"{"model":"voyage-3.5","input":["ok"],"output_dimension":512}"#;

/// `Err` when the keychain refused, as opposed to holding no key.
pub(crate) fn fetch_api_key() -> Result<Option<String>, String> {
    KEY.fetch()
}

/// Voyage's `{"detail": "..."}` — for us to read, never to show.
fn detail_of(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get("detail")?
        .as_str()
        .map(str::to_string)
}

/// Does this refusal describe the *account* rather than the key? On a free
/// account a 429, or a 401 about billing, is routine for a good key; reading
/// it as "wrong key" would have a student re-paste a correct one forever.
fn is_about_the_account(text: &str) -> bool {
    let text = text.to_lowercase();
    [
        "rate limit",
        "rate_limit",
        "ratelimit",
        "too many requests",
        "payment method",
        "payment_method",
        "billing",
        "add a card",
        "quota",
        "credit",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

/// Turn a status and body into a verdict; split out to test without a network.
fn interpret(status: u16, body: &str) -> Result<Verdict, String> {
    let detail = detail_of(body);
    // The whole body only for the limit sniff: it may be a proxy's HTML page.
    let text = detail.as_deref().unwrap_or(body);

    if status == 429 || is_about_the_account(text) {
        return Ok(Verdict::Unverified);
    }
    if !matches!(status, 401 | 403) {
        // It got past the gateway.
        return Ok(Verdict::Good);
    }

    let lowered = detail.as_deref().unwrap_or_default().to_lowercase();
    Err(
        if lowered.contains("expired") || lowered.contains("revoked") {
            "Voyage says this key is no longer active — create a new one in your \
         Voyage dashboard"
                .to_string()
        } else if lowered.contains("header") || lowered.contains("malformed") {
            "Voyage could not read this key — paste the key on its own, with \
         nothing around it"
                .to_string()
        } else {
            "Voyage rejected this key — check you copied all of it, including the \
         pa- prefix"
                .to_string()
        },
    )
}

/// Ask Voyage whether it accepts this key. `Err` is a key Voyage actively
/// refused, and carries the message the settings page shows.
fn probe(key: &str) -> Result<Verdict, String> {
    match ureq::post(PROBE_URL)
        .timeout(Duration::from_secs(10))
        .set("Authorization", &format!("Bearer {key}"))
        .set("Content-Type", "application/json")
        .send_string(PROBE_BODY)
    {
        Ok(_) => Ok(Verdict::Good),
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            interpret(status, &body)
        }
        Err(_) => Ok(Verdict::Unverified),
    }
}

/// Store a key, but only one Voyage has not refused. Returns `"ok"` when it
/// was checked against Voyage and `"unverified"` when Voyage was unreachable
/// or rate-limited and the key was stored on trust.
#[tauri::command]
pub fn voyage_set_api_key(key: String) -> Result<String, String> {
    KEY.store_checked(&key, probe)
}

#[tauri::command]
pub fn voyage_has_api_key() -> Result<bool, String> {
    KEY.has("Voyage API key")
}

#[tauri::command]
pub fn voyage_delete_api_key() -> Result<(), String> {
    KEY.delete()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_good(status: u16, body: &str) -> bool {
        matches!(interpret(status, body), Ok(Verdict::Good))
    }

    fn is_unverified(status: u16, body: &str) -> bool {
        matches!(interpret(status, body), Ok(Verdict::Unverified))
    }

    fn refusal(status: u16, body: &str) -> String {
        match interpret(status, body) {
            Err(message) => message,
            Ok(_) => panic!("expected a refusal"),
        }
    }

    #[test]
    fn a_429_is_never_a_rejection() {
        assert!(is_unverified(429, ""));
        assert!(is_unverified(429, r#"{"detail":"Rate limit exceeded"}"#));
        assert!(is_unverified(429, "<html>429 Too Many Requests</html>"));
    }

    #[test]
    fn a_401_about_money_is_not_a_rejection_either() {
        assert!(is_unverified(
            401,
            r#"{"detail":"You must add a payment method to use this model."}"#
        ));
        assert!(is_unverified(
            403,
            r#"{"detail":"Your account has run out of credit."}"#
        ));
    }

    #[test]
    fn a_401_about_the_key_is_a_rejection() {
        let message = refusal(401, r#"{"detail":"Provided API key is invalid."}"#);
        assert!(message.contains("pa-"));
    }

    #[test]
    fn an_expired_key_gets_its_own_wording() {
        let message = refusal(401, r#"{"detail":"This API key has expired."}"#);
        assert!(message.contains("no longer active"));
    }

    #[test]
    fn a_malformed_header_gets_its_own_wording() {
        let message = refusal(401, r#"{"detail":"Authorization header is malformed."}"#);
        assert!(message.contains("paste the key on its own"));
    }

    #[test]
    fn a_refusal_with_no_detail_still_reads_as_english() {
        assert!(!refusal(401, "").is_empty());
        assert!(!refusal(403, "not json at all").is_empty());
    }

    #[test]
    fn a_refusal_never_quotes_the_server() {
        // Nothing from the wire is spliced into a message.
        for body in [
            r#"{"detail":"Provided API key pa-SECRET is invalid."}"#,
            r#"{"detail":"see https://docs.voyageai.com/errors"}"#,
        ] {
            let message = refusal(401, body);
            assert!(!message.contains("SECRET"));
            assert!(!message.contains("http"));
        }
    }

    #[test]
    fn anything_that_got_past_the_gateway_is_good() {
        // A 400 means Voyage read the key and disliked our request body.
        assert!(is_good(400, r#"{"detail":"model not found"}"#));
        assert!(is_good(200, ""));
        assert!(is_good(500, "upstream error"));
    }
}
