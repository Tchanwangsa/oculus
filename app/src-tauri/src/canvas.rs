//! Canvas HTTP access for the app and the `oculus` CLI: session cookie,
//! retries, Link-header pagination. In Rust, not a WebView — see
//! `docs/architecture.md`.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub use crate::paths::CANVAS_BASE;

/// A finished response. 4xx/5xx are values (a locked module is a 403 to
/// skip); `Err` means the request never completed, after retries.
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

/// Nothing above this layer has a timeout, so this is what ends a wedged sync.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);
const RETRIES: u32 = 2;

/// Returned by [`Canvas::download_to`] when its `cancelled` poll said stop.
pub const CANCELLED: &str = "cancelled";

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
    /// Load the persisted session cookie; check [`Canvas::has_session`].
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

    /// The raw cookie header, for the Ed LTI launch's Canvas legs.
    pub fn cookie_header(&self) -> String {
        self.cookie()
    }

    /// Fold a response's `Set-Cookie` values back into the store: Canvas
    /// rotates the HttpOnly `canvas_session` server-side, so the login snapshot
    /// alone goes stale.
    fn absorb(&self, resp: &ureq::Response) {
        let set: Vec<String> = resp.all("set-cookie").into_iter().map(str::to_string).collect();
        if set.is_empty() {
            return;
        }
        let mut guard = self.cookie.lock().unwrap();
        let Some(merged) = merged_cookie_header(&guard, &set) else {
            return;
        };
        if let Err(e) = crate::paths::write_private(&self.cookie_path, &merged) {
            eprintln!("[oculus] cookie refresh write failed: {e}");
        }
        *guard = merged;
    }

    /// GET an absolute URL or a Canvas-relative path. Only Canvas gets the
    /// cookie — a `public_url` is a pre-signed S3 link on another host.
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

    /// Stream a GET into `dest` without holding the body in memory, for files
    /// too large to buffer. `TIMEOUT` bounds each read, not the whole transfer.
    /// Only a Canvas URL gets the cookie, and ureq drops `Cookie` on every
    /// redirect, so the signed file host a Canvas download redirects to never
    /// sees it. Progress is whole percents; `cancelled` is polled per chunk.
    pub fn download_to(
        &self,
        url_or_path: &str,
        dest: &Path,
        on_progress: &dyn Fn(u8),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<u64, String> {
        let url = if url_or_path.starts_with("http") {
            url_or_path.to_string()
        } else {
            format!("{CANVAS_BASE}{url_or_path}")
        };
        let canvas_host = |u: &str| u.starts_with(&format!("{CANVAS_BASE}/"));
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(30))
            .timeout_read(TIMEOUT)
            .build();
        let mut req = agent.get(&url);
        if canvas_host(&url) {
            let c = self.cookie();
            if !c.is_empty() {
                req = req.set("Cookie", &c);
            }
        }
        let resp = match req.call() {
            Ok(resp) => resp,
            Err(ureq::Error::Status(code, _)) => return Err(format!("download HTTP {code}")),
            Err(e) => return Err(format!("download failed: {e}")),
        };
        // Set-Cookie from the file host is not Canvas's to keep.
        if canvas_host(resp.get_url()) {
            self.absorb(&resp);
        }
        // A login page where a file should be means the session lapsed.
        if resp.content_type().contains("text/html") {
            return Err("got HTML instead of the file — session or URL problem".to_string());
        }
        let total = resp
            .header("content-length")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let mut reader = resp.into_reader();
        let mut file = std::fs::File::create(dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
        let mut buf = vec![0u8; 256 * 1024];
        let mut done = 0u64;
        let mut last_pct = u8::MAX;
        loop {
            if cancelled() {
                return Err(CANCELLED.to_string());
            }
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("network read failed after {done} bytes: {e}"))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|e| format!("write failed after {done} bytes: {e}"))?;
            done += n as u64;
            if total > 0 {
                let pct = (done * 100 / total).min(100) as u8;
                if pct != last_pct {
                    last_pct = pct;
                    on_progress(pct);
                }
            }
        }
        if total > 0 && done != total {
            return Err(format!("download incomplete: {done} of {total} bytes"));
        }
        file.sync_all().map_err(|e| e.to_string())?;
        Ok(done)
    }

    pub fn get_json(&self, url_or_path: &str) -> Result<serde_json::Value, String> {
        let r = self.get(url_or_path)?;
        if !r.ok() {
            return Err(format!("HTTP {} for {url_or_path}", r.status));
        }
        r.json()
    }

    /// Follow `rel="next"` and concatenate every page; a 403/404 mid-walk
    /// stops with what we have.
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

    /// Ping Canvas and classify the answer — also the keep-alive, since Canvas
    /// rolls the session forward on use. `Unreachable` is distinct from
    /// `Rejected` so a flat network never forces a fresh SSO login.
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

/// Merge `Set-Cookie` values into a cookie header, preserving order. `None`
/// when nothing changed, so the file is not rewritten on every request.
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
        // Session values are base64 ending in '='; split on the first '=' only.
        let out = merged_cookie_header("s=old", &sc(&["s=abc==; path=/"])).unwrap();
        assert_eq!(out, "s=abc==");
    }

    /// Serves one canned response on a loopback port; returns its URL.
    fn serve_once(response: Vec<u8>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut head = [0u8; 4096];
            let _ = stream.read(&mut head);
            let _ = stream.write_all(&response);
        });
        format!("http://{addr}/video.mp4")
    }

    fn signed_out() -> Canvas {
        Canvas { cookie: Mutex::new(String::new()), cookie_path: PathBuf::new() }
    }

    #[test]
    fn a_download_streams_to_disk_and_refuses_a_short_body() {
        let dir = crate::test_support::Scratch::new("canvas-download");
        let body = vec![7u8; 300_000];
        let mut whole = format!("HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\n\r\n", body.len())
            .into_bytes();
        whole.extend_from_slice(&body);
        let seen = Mutex::new(Vec::new());
        let dest = dir.join("whole.mp4");
        let n = signed_out()
            .download_to(&serve_once(whole), &dest, &|p| seen.lock().unwrap().push(p), &|| false)
            .unwrap();
        assert_eq!(n, 300_000);
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        assert_eq!(seen.lock().unwrap().last(), Some(&100));

        let short = b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nonly this".to_vec();
        let err = signed_out()
            .download_to(&serve_once(short), &dir.join("short.mp4"), &|_| {}, &|| false)
            .unwrap_err();
        assert_ne!(err, CANCELLED);
    }

    #[test]
    fn a_cancelled_download_says_so() {
        let dir = crate::test_support::Scratch::new("canvas-cancel");
        let ok = b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nmp4!".to_vec();
        let err = signed_out()
            .download_to(&serve_once(ok), &dir.join("x.mp4"), &|_| {}, &|| true)
            .unwrap_err();
        assert_eq!(err, CANCELLED);
    }

    #[test]
    fn finds_next_page_in_link_header() {
        let link = r#"<https://c/api?page=1>; rel="current", <https://c/api?page=2>; rel="next""#;
        assert_eq!(parse_next_link(link).unwrap(), "https://c/api?page=2");
        assert!(parse_next_link(r#"<https://c/api?page=9>; rel="last""#).is_none());
    }
}
