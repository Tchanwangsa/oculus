//! Keeping provider keys out of errors and logs.

/// Take the secret the app is holding out of an error that may quote the
/// request back.
pub(super) fn redact(s: &str, secret: &str) -> String {
    if secret.len() < 8 {
        return s.to_string();
    }
    s.replace(secret, "[redacted]")
}

/// Truncate at the first key-like field name, so a response body echoing a
/// provider key never reaches a log.
pub(super) fn scrub(s: &str) -> String {
    let mut out = s.to_string();
    for key in ["apiKey", "api_key", "Authorization", "authorization"] {
        if let Some(at) = out.find(key) {
            out.truncate(at);
            out.push_str("[redacted]");
        }
    }
    out
}
