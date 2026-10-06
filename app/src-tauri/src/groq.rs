//! Groq credential storage, for video transcription (`transcribe/groq.rs`).
//!
//! The key lives only in the macOS keychain, never in SQLite or the WebView.
//! Same shape as `voyage.rs`: three commands, an `"ok"`/`"unverified"` answer.

use std::time::Duration;

use crate::credentials::{Secret, Verdict};

const KEY: Secret = Secret::new("com.tchan.oculus.groq", "groq");

/// Listing models is authenticated and free. Settings may never make a billed
/// call, so the probe must not transcribe anything.
const PROBE_URL: &str = "https://api.groq.com/openai/v1/models";

pub(crate) fn stored_api_key() -> Option<String> {
    KEY.read()
}

/// Groq's OpenAI-shaped `{"error": {"message": "..."}}`.
pub(crate) fn message_of(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get("error")?
        .get("message")?
        .as_str()
        .map(str::to_string)
}

/// Does this refusal describe the account or the network rather than the key?
/// Groq answers a blocked region or a restricted organisation with a 403.
fn is_about_the_account(text: &str) -> bool {
    let text = text.to_lowercase();
    [
        "rate limit",
        "rate_limit",
        "too many requests",
        "organization",
        "organisation",
        "restricted",
        "network",
        "region",
        "country",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

/// Turn a status and body into a verdict; split out to test without a network.
fn interpret(status: u16, body: &str) -> Result<Verdict, String> {
    let message = message_of(body);
    // The whole body only for the sniff: it may be a proxy's HTML page.
    let text = message.as_deref().unwrap_or(body);

    if status == 429 || is_about_the_account(text) {
        return Ok(Verdict::Unverified);
    }
    if !matches!(status, 401 | 403) {
        // It got past the gateway.
        return Ok(Verdict::Good);
    }

    let lowered = message.as_deref().unwrap_or_default().to_lowercase();
    Err(if lowered.contains("header") || lowered.contains("malformed") {
        "Groq could not read this key — paste the key on its own, with \
         nothing around it"
            .to_string()
    } else {
        "Groq rejected this key — check you copied all of it, including the \
         gsk_ prefix"
            .to_string()
    })
}

/// Ask Groq whether it accepts this key. `Err` is a key Groq actively
/// refused, and carries the message the settings page shows.
fn probe(key: &str) -> Result<Verdict, String> {
    match ureq::get(PROBE_URL)
        .timeout(Duration::from_secs(10))
        .set("Authorization", &format!("Bearer {key}"))
        .call()
    {
        Ok(_) => Ok(Verdict::Good),
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            interpret(status, &body)
        }
        Err(_) => Ok(Verdict::Unverified),
    }
}

/// Store a key, but only one Groq has not refused. Returns `"ok"` when it
/// was checked against Groq and `"unverified"` when Groq was unreachable
/// or rate-limited and the key was stored on trust.
#[tauri::command]
pub fn groq_set_api_key(key: String) -> Result<String, String> {
    KEY.store_checked(&key, probe)
}

#[tauri::command]
pub fn groq_has_api_key() -> bool {
    stored_api_key().is_some()
}

#[tauri::command]
pub fn groq_delete_api_key() -> Result<(), String> {
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
        assert!(is_unverified(
            429,
            r#"{"error":{"message":"Rate limit reached for model","type":"tokens"}}"#
        ));
        assert!(is_unverified(429, "<html>429 Too Many Requests</html>"));
    }

    #[test]
    fn a_403_about_the_network_is_not_a_rejection() {
        assert!(is_unverified(
            403,
            r#"{"error":{"message":"Access denied. Please check your network settings."}}"#
        ));
        assert!(is_unverified(
            403,
            r#"{"error":{"message":"Your organization has been restricted."}}"#
        ));
    }

    #[test]
    fn a_401_about_the_key_is_a_rejection() {
        let message = refusal(
            401,
            r#"{"error":{"message":"Invalid API Key","type":"invalid_request_error","code":"invalid_api_key"}}"#,
        );
        assert!(message.contains("gsk_"));
    }

    #[test]
    fn a_malformed_header_gets_its_own_wording() {
        let message = refusal(401, r#"{"error":{"message":"Authorization header is malformed."}}"#);
        assert!(message.contains("paste the key on its own"));
    }

    #[test]
    fn a_refusal_with_no_message_still_reads_as_english() {
        assert!(!refusal(401, "").is_empty());
        assert!(!refusal(403, "not json at all").is_empty());
    }

    #[test]
    fn a_refusal_never_quotes_the_server() {
        // Nothing from the wire is spliced into a message.
        for body in [
            r#"{"error":{"message":"Invalid API Key gsk_SECRET"}}"#,
            r#"{"error":{"message":"see https://console.groq.com/docs/errors"}}"#,
        ] {
            let message = refusal(401, body);
            assert!(!message.contains("SECRET"));
            assert!(!message.contains("http"));
        }
    }

    #[test]
    fn anything_that_got_past_the_gateway_is_good() {
        assert!(is_good(200, r#"{"object":"list","data":[]}"#));
        assert!(is_good(404, r#"{"error":{"message":"Unknown request URL"}}"#));
        assert!(is_good(500, "upstream error"));
    }
}
