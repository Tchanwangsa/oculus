//! Reading the API's error bodies and vetting the URLs it hands out.

use crate::parse::ParseError;
use serde_json::Value;
use url::Url;

/// A0211 is an expired token; A0202 one MinerU never accepted.
pub(super) fn auth_error(body: &str) -> ParseError {
    let code = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|payload| {
            payload
                .get("msgCode")
                .or_else(|| payload.get("code"))
                .and_then(Value::as_str)
                .map(sanitise_code)
        });
    let expired = code.as_deref() == Some("A0211");
    ParseError::RejectedCredentials { code, expired }
}

/// Clip a network-supplied `code` to something that can only be a code.
pub(super) fn sanitise_code(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect()
}

pub(super) fn safe_code(code: Option<&Value>) -> String {
    match code {
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::String(text)) => sanitise_code(text),
        _ => "unknown".into(),
    }
}

/// Signed URLs must be `https`; loopback is exempt so tests can drive the
/// protocol, and cannot carry a signature off this machine.
pub(super) fn check_transfer_url(url: &str, what: &str) -> Result<(), ParseError> {
    let parsed = Url::parse(url).map_err(|_| ParseError::Document {
        code: format!("{what}-url-invalid"),
    })?;
    let host = parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or(ParseError::Document {
            code: format!("{what}-url-invalid"),
        })?;
    let loopback = matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]");
    if parsed.scheme() != "https" && !loopback {
        return Err(ParseError::Document {
            code: format!("{what}-url-insecure"),
        });
    }
    Ok(())
}
