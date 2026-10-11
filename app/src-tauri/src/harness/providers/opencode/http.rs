//! The server's HTTP surface: one place that builds URLs, sends, and turns an
//! answer or an error into text that is safe to log.

use serde_json::Value;

use super::events::error_sentence;
use super::redact::scrub;
use super::server::OpencodeServer;

impl OpencodeServer {
    pub(super) fn get(&self, path: &str) -> Result<Value, String> {
        self.finish(self.api.get(&format!("{}{path}", self.base)).call(), path)
    }

    /// `send_string`: ureq's `json` feature is off in this crate.
    pub(super) fn post(&self, path: &str, body: Value) -> Result<Value, String> {
        self.finish(
            self.api
                .post(&format!("{}{path}", self.base))
                .set("Content-Type", "application/json")
                .send_string(&body.to_string()),
            path,
        )
    }

    /// A `POST` with no body, for endpoints that declare none (`/abort`).
    pub(super) fn post_empty(&self, path: &str) -> Result<Value, String> {
        self.finish(self.api.post(&format!("{}{path}", self.base)).call(), path)
    }

    pub(super) fn delete(&self, path: &str) -> Result<Value, String> {
        self.finish(
            self.api.delete(&format!("{}{path}", self.base)).call(),
            path,
        )
    }

    /// 204 or an empty body is `Null`. Error text is [`super::redact::scrub`]bed: response
    /// bodies can echo provider keys, and errors end up in logs and rows.
    pub(super) fn finish(
        &self,
        r: Result<ureq::Response, ureq::Error>,
        path: &str,
    ) -> Result<Value, String> {
        match r {
            Ok(resp) => {
                let body = resp.into_string().unwrap_or_default();
                if body.trim().is_empty() {
                    return Ok(Value::Null);
                }
                serde_json::from_str(&body)
                    .map_err(|e| format!("opencode {path}: unreadable answer ({e})"))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                let msg = match serde_json::from_str::<Value>(&body) {
                    Ok(v) => error_sentence(&v),
                    Err(_) => scrub(&body.chars().take(400).collect::<String>()),
                };
                Err(format!("opencode {path}: HTTP {code} {msg}"))
            }
            Err(e) => Err(format!("opencode {path}: {}", scrub(&e.to_string()))),
        }
    }

    /// No `?directory=`: `/auth/{id}` is machine-wide, not instance-scoped.
    pub(super) fn put_auth(&self, provider: &str, body: Value) -> Result<(), String> {
        let path = format!("/auth/{}", urlencode(provider));
        let v = self.finish(
            self.api
                .put(&format!("{}{path}", self.base))
                .set("Content-Type", "application/json")
                .send_string(&body.to_string()),
            &path,
        )?;
        if v.as_bool() == Some(false) {
            return Err("opencode would not accept that credential.".into());
        }
        Ok(())
    }
}

pub(super) fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
