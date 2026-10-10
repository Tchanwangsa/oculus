//! Canvas HTTP access for the app and the `oculus` CLI: requests through
//! oculus-keyd's `canvas` route, which holds the session cookie and signs in
//! again when Canvas rejects it; retries; redirects; Link-header pagination.
//! In Rust, not a WebView — see `docs/architecture.md`.

mod download;
mod error;
mod transport;

#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::Mutex;

use crate::providers::credentials::Credentialed;

pub use crate::library::paths::CANVAS_BASE;
pub use error::CanvasError;
pub(crate) use transport::{is_canvas, resolve, Hop};

/// Bounds a request to a host other than Canvas. Canvas requests are bounded
/// by oculus-keyd's own read timeout, and nothing above this layer has one.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);
const RETRIES: u32 = 2;

/// Returned by [`Canvas::download_to`] when its `cancelled` poll said stop.
pub const CANCELLED: &str = "cancelled";

/// A finished response. 4xx/5xx are values (a locked module is a 403 to
/// skip); `Err` means the request never completed, after retries.
#[derive(Debug)]
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
    pub fn json(&self) -> Result<serde_json::Value, CanvasError> {
        serde_json::from_slice(&self.body)
            .map_err(|e| CanvasError::Failed(format!("bad JSON: {e}")))
    }
}

impl From<Hop> for Res {
    fn from(hop: Hop) -> Res {
        Res {
            content_type: hop.header("content-type").unwrap_or_default().to_string(),
            next: hop.header("link").and_then(parse_next_link),
            status: hop.status,
            body: hop.body,
        }
    }
}

/// One run's view of Canvas. It holds no session: oculus-keyd does, and
/// attaches it to each request. The first `CanvasError::Expired` is kept, and
/// every later request fails with it at once, so a run learns the session is
/// gone from one place ([`Canvas::expired`]) and does not ask oculus-keyd to
/// sign in again for each request left. Open a new `Canvas` per run.
pub struct Canvas {
    keyd: Credentialed,
    expired: Mutex<Option<CanvasError>>,
}

/// Outcome of pinging Canvas through the saved session.
pub enum SessionProbe {
    Valid(String),
    /// Canvas turned the session down, after oculus-keyd tried to sign in.
    Rejected(String),
    /// No answer, a 5xx, or no oculus-keyd: the session may be fine.
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
    /// A client of the oculus-keyd serving `data_dir`.
    pub fn open(data_dir: &Path) -> Self {
        Canvas {
            keyd: Credentialed::at(data_dir),
            expired: Mutex::new(None),
        }
    }

    /// Whether oculus-keyd holds a Canvas session right now. False when it
    /// holds none or cannot be asked; a request still signs in on demand, so
    /// this is for display, not a gate ([`Canvas::check_keyd`] is one).
    pub fn has_session(&self) -> bool {
        self.keyd.session_status().is_ok_and(|s| s.canvas)
    }

    /// `KeydAbsent` (or another keyd error) when oculus-keyd cannot be asked.
    pub fn check_keyd(&self) -> Result<(), CanvasError> {
        self.keyd
            .session_status()
            .map(|_| ())
            .map_err(CanvasError::from_keyd)
    }

    /// The `Expired` this client has met, if any.
    pub fn expired(&self) -> Option<CanvasError> {
        self.expired.lock().unwrap().clone()
    }

    fn latch<T>(&self, result: Result<T, CanvasError>) -> Result<T, CanvasError> {
        if let Err(e @ CanvasError::Expired(_)) = &result {
            self.expired
                .lock()
                .unwrap()
                .get_or_insert_with(|| e.clone());
        }
        result
    }

    /// GET an absolute URL or a Canvas-relative path, following up to five
    /// redirects. A Canvas URL goes through oculus-keyd, which attaches the
    /// cookie; any other host (a `public_url` is a pre-signed S3 link) is
    /// fetched directly with none.
    pub fn get(&self, url_or_path: &str) -> Result<Res, CanvasError> {
        let mut url = resolve(url_or_path)?;
        for followed in 0.. {
            let hop = if is_canvas(&url) {
                self.hop("GET", &url, &[], b"")?
            } else {
                transport::direct(&url)?
            };
            let Some(next) = hop.redirect_from(&url)? else {
                return Ok(hop.into());
            };
            if followed == transport::MAX_REDIRECTS {
                return Err(CanvasError::Failed(format!(
                    "Canvas redirected more than {} times",
                    transport::MAX_REDIRECTS
                )));
            }
            url = next;
        }
        unreachable!("the loop returns")
    }

    pub fn get_json(&self, url_or_path: &str) -> Result<serde_json::Value, CanvasError> {
        let r = self.get(url_or_path)?;
        if !r.ok() {
            return Err(CanvasError::Http {
                status: r.status,
                what: url_or_path.to_string(),
            });
        }
        r.json()
    }

    /// Follow `rel="next"` and concatenate every page; a 401, 403 or 404
    /// mid-walk (Canvas's "you may not see this") stops with what we have.
    pub fn get_all(&self, url_or_path: &str) -> Result<Vec<serde_json::Value>, CanvasError> {
        let mut out = Vec::new();
        let mut next = Some(url_or_path.to_string());
        while let Some(url) = next {
            let r = self.get(&url)?;
            if !r.ok() {
                if matches!(r.status, 401 | 403 | 404) {
                    break;
                }
                return Err(CanvasError::Http {
                    status: r.status,
                    what: url,
                });
            }
            match r.json()? {
                serde_json::Value::Array(items) => out.extend(items),
                other => out.push(other),
            }
            next = r.next.clone();
        }
        Ok(out)
    }

    /// Ping Canvas and classify the answer. Only `Rejected` means the session
    /// is dead: `Unreachable` covers a flat network, a Canvas outage and a
    /// missing oculus-keyd, so none of them forces a fresh sign-in.
    pub fn probe(&self) -> SessionProbe {
        match self.get("/api/v1/users/self") {
            Ok(r) if r.ok() => match r.json() {
                Ok(v) => SessionProbe::Valid(display_name(&v)),
                Err(e) => SessionProbe::Unreachable(format!("unreadable Canvas response: {e}")),
            },
            Ok(r) if r.status == 401 => SessionProbe::Rejected(
                "Canvas rejected the session (401) — sign in again.".to_string(),
            ),
            Ok(r) if r.status >= 500 => {
                SessionProbe::Unreachable(format!("Canvas returned HTTP {}", r.status))
            }
            Ok(r) => SessionProbe::Rejected(format!("Canvas returned HTTP {}", r.status)),
            Err(e @ CanvasError::Expired(_)) => SessionProbe::Rejected(e.to_string()),
            Err(e) => SessionProbe::Unreachable(e.to_string()),
        }
    }

    pub fn whoami(&self) -> Result<String, String> {
        match self.probe() {
            SessionProbe::Valid(name) => Ok(name),
            SessionProbe::Rejected(why) | SessionProbe::Unreachable(why) => Err(why),
        }
    }
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
