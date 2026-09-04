//! Canvas HTTP access: session cookie, retries, Link-header pagination.
//!
//! This is the whole Canvas surface for both the app and the `oculus` CLI. It
//! replaces the old arrangement where a hidden WebView ran `scraper.js` and
//! tunnelled every request back through the local IPC server — that indirection
//! bought nothing (the cookie always lived here in Rust) and cost everything:
//! macOS suspends an off-screen WKWebView's content process, which froze the
//! scraper mid-run with no error to catch.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub use crate::paths::CANVAS_BASE;

/// A finished response. 4xx/5xx are values, not errors — callers branch on
/// `status` (a locked module is a 403 to skip, not a run to abort). `Err` is
/// reserved for "the request never completed", after retries.
pub struct Res {
    pub status: u16,
    pub body: Vec<u8>,
    pub content_type: String,
    /// `rel="next"` target from the Link header, if any.
    pub next: Option<String>,
}

impl Res {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
    pub fn json(&self) -> Result<serde_json::Value, String> {
        serde_json::from_slice(&self.body).map_err(|e| format!("bad JSON: {e}"))
    }
}

/// Long enough for a big file on a slow link, short enough that a wedged
/// request always ends. Nothing above this layer has its own timeout, so this
/// value is what guarantees a sync terminates.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);
const RETRIES: u32 = 2;

pub struct Canvas {
    cookie: Mutex<String>,
    cookie_path: PathBuf,
}

/// Outcome of pinging Canvas with the saved session.
pub enum SessionProbe {
    Valid(String),
    Rejected(String),
    Unreachable(String),
}

fn display_name(v: &serde_json::Value) -> String {
    v["name"]
        .as_str()
        .or_else(|| v["short_name"].as_str())
        .unwrap_or("Canvas user")
        .to_string()
}

impl Canvas {
    /// Load the persisted session cookie. Succeeds even with no cookie —
    /// callers check [`Canvas::has_session`] so they can report "not signed in"
    /// rather than a wall of 401s.
    pub fn open(data_dir: &Path) -> Self {
        let cookie_path = crate::paths::cookie_path(data_dir);
        let cookie = std::fs::read_to_string(&cookie_path).unwrap_or_default();
        Canvas {
            cookie: Mutex::new(cookie.trim().to_string()),
            cookie_path,
        }
    }

    pub fn has_session(&self) -> bool {
        !self.cookie.lock().unwrap().is_empty()
    }

    fn cookie(&self) -> String {
        self.cookie.lock().unwrap().clone()
    }

    /// The raw cookie header, for flows that drive Canvas pages outside this
    /// client — the Ed LTI launch hops between hosts and needs to attach it
    /// to the Canvas legs itself.
    pub fn cookie_header(&self) -> String {
        self.cookie()
    }

    /// Fold a response's `Set-Cookie` values back into the store.
    ///
    /// `canvas_session` carries no Expires — Canvas rotates it server-side now
    /// and then. Keeping the login snapshot forever means eventually presenting
    /// a value the server has moved past, so every response updates it.
    fn absorb(&self, resp: &ureq::Response) {
        let set: Vec<String> = resp.all("set-cookie").into_iter().map(str::to_string).collect();
        if set.is_empty() {
            return;
        }
        let mut guard = self.cookie.lock().unwrap();
        let Some(merged) = merged_cookie_header(&guard, &set) else {
            return;
        };
        if let Err(e) = std::fs::write(&self.cookie_path, &merged) {
            eprintln!("[oculus] cookie refresh write failed: {e}");
        }
        *guard = merged;
    }

    /// GET an absolute URL or a Canvas-relative path.
    ///
    /// Only Canvas gets the session cookie: a `public_url` points at a
    /// pre-signed S3 link, and handing our cookie to a third-party host would
    /// be a credential leak.
    pub fn get(&self, url_or_path: &str) -> Result<Res, String> {
        let url = if url_or_path.starts_with("http") {
            url_or_path.to_string()
        } else {
            format!("{CANVAS_BASE}{url_or_path}")
        };
        let is_canvas = url.starts_with(&format!("{CANVAS_BASE}/"));

        let mut last = String::new();
        for attempt in 0..=RETRIES {
            let mut req = ureq::get(&url).timeout(TIMEOUT);
            if is_canvas {
                let c = self.cookie();
                if !c.is_empty() {
                    req = req.set("Cookie", &c);
                }
            }

            match req.call() {
                Ok(resp) => {
                    self.absorb(&resp);
                    return Ok(collect(resp));
                }
                // Canvas 4xx/5xx still carry a body and a rotated cookie.
                Err(ureq::Error::Status(code, resp)) => {
                    self.absorb(&resp);
                    // 5xx is worth another go; 4xx is an answer, not a failure.
                    if code >= 500 && attempt < RETRIES {
                        last = format!("HTTP {code}");
                        backoff(attempt);
                        continue;
                    }
                    return Ok(collect(resp));
                }
                Err(e) => {
                    last = e.to_string();
                    if attempt < RETRIES {
                        backoff(attempt);
                    }
                }
            }
        }
        Err(format!("{url}: {last}"))
    }

    pub fn get_json(&self, url_or_path: &str) -> Result<serde_json::Value, String> {
        let r = self.get(url_or_path)?;
        if !r.ok() {
            return Err(format!("HTTP {} for {url_or_path}", r.status));
        }
        r.json()
    }

    /// Follow `rel="next"` to the end and concatenate every page.
    /// 403/404 mid-walk means "locked or absent" — stop with what we have.
    pub fn get_all(&self, url_or_path: &str) -> Result<Vec<serde_json::Value>, String> {
        let mut out = Vec::new();
        let mut next = Some(url_or_path.to_string());
        while let Some(url) = next {
            let r = self.get(&url)?;
            if !r.ok() {
                if r.status == 403 || r.status == 404 {
                    break;
                }
                return Err(format!("HTTP {} for {url}", r.status));
            }
            match r.json()? {
                serde_json::Value::Array(items) => out.extend(items),
                other => out.push(other),
            }
            next = r.next.clone();
        }
        Ok(out)
    }

    /// Ping Canvas and classify the answer.
    ///
    /// `Unreachable` is deliberately distinct from `Rejected` — a flat network
    /// is no reason to throw away a working session and force a fresh SSO
    /// login, and it is the difference between the keep-alive burning an Okta
    /// sign-in attempt and waiting for the wifi to come back.
    ///
    /// The request doubles as the keep-alive itself: Canvas rolls the session
    /// forward on use, and `get` folds any rotated cookie back to disk.
    pub fn probe(&self) -> SessionProbe {
        if !self.has_session() {
            return SessionProbe::Rejected("No saved Canvas session.".to_string());
        }
        match self.get("/api/v1/users/self") {
            Ok(r) if r.ok() => match r.json() {
                Ok(v) => SessionProbe::Valid(display_name(&v)),
                Err(e) => SessionProbe::Unreachable(format!("unreadable Canvas response: {e}")),
            },
            Ok(r) if r.status == 401 => SessionProbe::Rejected(
                "Canvas rejected the session (401) — sign in again.".to_string(),
            ),
            Ok(r) => SessionProbe::Rejected(format!("Canvas returned HTTP {}", r.status)),
            Err(e) => SessionProbe::Unreachable(format!("could not reach Canvas: {e}")),
        }
    }

    /// The signed-in user's display name, or an error describing why not.
    pub fn whoami(&self) -> Result<String, String> {
        match self.probe() {
            SessionProbe::Valid(name) => Ok(name),
            SessionProbe::Rejected(why) | SessionProbe::Unreachable(why) => Err(why),
        }
    }
}

fn backoff(attempt: u32) {
    std::thread::sleep(std::time::Duration::from_millis(500 * (attempt as u64 + 1)));
}

fn collect(resp: ureq::Response) -> Res {
    let status = resp.status();
    let content_type = resp.content_type().to_string();
    let next = resp.header("Link").and_then(parse_next_link);
    let mut body = Vec::new();
    let _ = resp.into_reader().read_to_end(&mut body);
    Res { status, body, content_type, next }
}

/// `<https://…?page=2>; rel="next", <…>; rel="last"` → the next URL.
fn parse_next_link(link: &str) -> Option<String> {
    link.split(',')
        .find(|part| part.contains("rel=\"next\""))
        .and_then(|part| {
            let start = part.find('<')? + 1;
            let end = part[start..].find('>')? + start;
            Some(part[start..end].to_string())
        })
}

// ── Cookie merge ─────────────────────────────────────────────────────────────

fn parse_cookie_header(header: &str) -> Vec<(String, String)> {
    header
        .split(';')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            let (name, value) = part.split_once('=')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

/// Merge `Set-Cookie` values into a cookie header. Returns the new header, or
/// `None` when nothing changed — a sync makes hundreds of requests and most
/// carry no rotation, so this is what keeps us from rewriting the file
/// constantly. Order is preserved so the header stays stable.
pub fn merged_cookie_header(current: &str, set_cookies: &[String]) -> Option<String> {
    if current.is_empty() {
        return None;
    }

    let mut pairs = parse_cookie_header(current);
    let mut changed = false;

    for raw in set_cookies {
        // "name=value; Path=/; HttpOnly" → we only care about the first pair.
        let Some(first) = raw.split(';').next() else { continue };
        let Some((name, value)) = first.trim().split_once('=') else { continue };
        let (name, value) = (name.trim(), value.trim());
        if name.is_empty() {
            continue;
        }
        match pairs.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => {
                if slot.1 != value {
                    slot.1 = value.to_string();
                    changed = true;
                }
            }
            None => {
                pairs.push((name.to_string(), value.to_string()));
                changed = true;
            }
        }
    }

    changed.then(|| {
        pairs
            .iter()
            .map(|(n, v)| format!("{n}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sc(vals: &[&str]) -> Vec<String> {
        vals.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn replaces_rotated_value_in_place() {
        let out = merged_cookie_header(
            "a=1; canvas_session=OLD; z=9",
            &sc(&["canvas_session=NEW; path=/; secure; httponly"]),
        );
        // Position must be preserved, not appended to the end.
        assert_eq!(out.unwrap(), "a=1; canvas_session=NEW; z=9");
    }

    #[test]
    fn appends_cookies_not_seen_before() {
        assert_eq!(merged_cookie_header("a=1", &sc(&["b=2; path=/"])).unwrap(), "a=1; b=2");
    }

    #[test]
    fn no_write_when_value_is_unchanged() {
        assert!(merged_cookie_header("a=1; b=2", &sc(&["b=2; path=/"])).is_none());
    }

    #[test]
    fn ignores_junk_and_empty_store() {
        assert!(merged_cookie_header("", &sc(&["a=1"])).is_none());
        assert!(merged_cookie_header("a=1", &sc(&["novalue; path=/"])).is_none());
    }

    #[test]
    fn keeps_base64_padding_in_values() {
        // Canvas session values are base64 and end in '='; splitting on the
        // first '=' only is what preserves them.
        let out = merged_cookie_header("s=old", &sc(&["s=abc==; path=/"])).unwrap();
        assert_eq!(out, "s=abc==");
    }

    #[test]
    fn finds_next_page_in_link_header() {
        let link = r#"<https://c/api?page=1>; rel="current", <https://c/api?page=2>; rel="next""#;
        assert_eq!(parse_next_link(link).unwrap(), "https://c/api?page=2");
        assert!(parse_next_link(r#"<https://c/api?page=9>; rel="last""#).is_none());
    }
}
