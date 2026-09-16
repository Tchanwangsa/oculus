//! Voyage AI credential storage.
//!
//! The key never enters SQLite or the WebView. Rust reads it from the macOS
//! keychain and hands it straight to the embedding client, so it no longer
//! crosses a socket at all. Shape mirrors `mineru.rs` on purpose — same
//! keychain-only rule, same three commands, same `"ok"`/`"unverified"` answer.

use std::time::Duration;

const KEYCHAIN_SERVICE: &str = "com.tchan.oculus.voyage";
const KEYCHAIN_ACCOUNT: &str = "voyage";

/// The cheapest authenticated call Voyage has. Unlike MinerU there is no free
/// lookup to hide behind — no "list models", no GET that answers without
/// billing — so the probe embeds a two-letter string at the smallest output
/// width and pays one text token for the privilege. A bad key answers 401
/// before the model is ever loaded.
const PROBE_URL: &str = "https://api.voyageai.com/v1/embeddings";
const PROBE_BODY: &str = r#"{"model":"voyage-3.5","input":["ok"],"output_dimension":512}"#;

fn keychain() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT).map_err(|e| e.to_string())
}

pub(crate) fn stored_api_key() -> Option<String> {
    keychain().ok()?.get_password().ok()
}

/// What a probe learned about a key. `Unverified` means Voyage never gave a
/// verdict — it was unreachable, or it answered about the *account* rather
/// than the key — and offline is not a reason to refuse a key the user typed.
enum Verdict {
    Good,
    Unverified,
}

/// Pull Voyage's human-readable reason out of an error body. Voyage answers
/// `{"detail": "..."}`; the string is for *us* to read, never to show — a
/// server body can carry anything, including an echo of the request.
fn detail_of(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get("detail")?
        .as_str()
        .map(str::to_string)
}

/// Does this refusal describe the *account* rather than the key?
///
/// This is the non-obvious part. Voyage meters an account with no payment
/// method on file at 3 requests per minute, and the probe is a real request,
/// so a 429 — or a 401 whose detail talks about billing — is the routine
/// answer for a perfectly good key on a free account. Reading that as "wrong
/// key" would tell a student to re-paste a key that was right all along, and
/// no amount of re-pasting would ever clear it. When Voyage is talking about
/// money or pace, we learned nothing about the key: say so and store it.
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

/// Turn a status and body into a verdict. Split out of the HTTP call so the
/// part with the judgement in it can be tested without a network.
fn interpret(status: u16, body: &str) -> Result<Verdict, String> {
    let detail = detail_of(body);
    // Fall back to the whole body only for the limit sniff — a body we could
    // not parse may still be an HTML rate-limit page from a proxy in front.
    let text = detail.as_deref().unwrap_or(body);

    if status == 429 || is_about_the_account(text) {
        return Ok(Verdict::Unverified);
    }
    if !matches!(status, 401 | 403) {
        // It got past the gateway. Whatever else Voyage disliked about a
        // two-letter embedding request is not the user's problem here.
        return Ok(Verdict::Good);
    }

    let lowered = detail.as_deref().unwrap_or_default().to_lowercase();
    Err(if lowered.contains("expired") || lowered.contains("revoked") {
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
    })
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
        // Transport failure: no network, DNS, TLS. Nothing was learned.
        Err(_) => Ok(Verdict::Unverified),
    }
}

/// Store a key, but only one Voyage has not refused. Returns `"ok"` when it
/// was checked against Voyage and `"unverified"` when Voyage was unreachable
/// or rate-limited and the key was stored on trust.
#[tauri::command]
pub fn voyage_set_api_key(key: String) -> Result<String, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("empty key".into());
    }
    let verdict = probe(key)?;
    keychain()?.set_password(key).map_err(|e| e.to_string())?;
    Ok(match verdict {
        Verdict::Good => "ok".into(),
        Verdict::Unverified => "unverified".into(),
    })
}

#[tauri::command]
pub fn voyage_has_api_key() -> bool {
    stored_api_key().is_some()
}

#[tauri::command]
pub fn voyage_delete_api_key() -> Result<(), String> {
    match keychain()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
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
        // The whole point: this account is capped at 3 requests a minute
        // because no card is on file, so the probe hits this routinely.
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
        // Messages are written for a student, so nothing from the wire —
        // no body, no URL, no key — may be spliced into one.
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
        // A 400 means Voyage read the key, then disliked the request body —
        // which is ours, not the user's.
        assert!(is_good(400, r#"{"detail":"model not found"}"#));
        assert!(is_good(200, ""));
        assert!(is_good(500, "upstream error"));
    }
}
