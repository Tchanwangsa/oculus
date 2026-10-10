//! One request to Canvas through oculus-keyd, one to a file host directly, and
//! the redirects between them. oculus-keyd attaches the session cookie to its
//! `canvas` route and never follows a redirect, so a `Location` on the Canvas
//! host comes back here and goes through the route again, while one anywhere
//! else (a signed S3 link, a file CDN) is fetched without a cookie.

use std::io::Read;
use std::time::Duration;

use keyd_core::client::{StreamedResponse, SESSION_TIMEOUT};
use keyd_core::okta::{LoginError, SSO_HOST};
use url::{Position, Url};

use super::{Canvas, CanvasError, CANVAS_BASE, RETRIES, TIMEOUT};

/// Redirects followed before a request is called a loop.
pub(super) const MAX_REDIRECTS: usize = 5;

/// How much of a 401's body is read to tell a dead session from a forbidden
/// one.
const REJECTION_BODY: u64 = 64 * 1024;

/// What one request answered, redirects not followed.
pub(crate) struct Hop {
    pub status: u16,
    /// Names lowercase.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Hop {
    pub fn header(&self, name: &str) -> Option<&str> {
        header(&self.headers, name)
    }

    /// The `Location` of a redirect, resolved against the URL it answered.
    pub fn redirect_from(&self, at: &Url) -> Result<Option<Url>, CanvasError> {
        redirect(self.status, self.header("location"), at)
    }
}

/// A response whose body is still on the wire.
pub(super) struct Streamed {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Box<dyn Read + Send>,
}

impl Streamed {
    pub fn header(&self, name: &str) -> Option<&str> {
        header(&self.headers, name)
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn canvas_origin() -> Url {
    Url::parse(CANVAS_BASE).expect("CANVAS_BASE is a URL")
}

/// An absolute URL, or a path on Canvas.
pub(crate) fn resolve(url_or_path: &str) -> Result<Url, CanvasError> {
    let text = if url_or_path.starts_with("http") {
        url_or_path.to_string()
    } else {
        format!("{CANVAS_BASE}{url_or_path}")
    };
    Url::parse(&text).map_err(|e| CanvasError::Failed(format!("not a usable URL: {e}")))
}

/// Whether `url` is on Canvas, the only host that gets the session.
pub(crate) fn is_canvas(url: &Url) -> bool {
    url.origin() == canvas_origin().origin()
}

/// The path and query oculus-keyd's `canvas` route takes.
fn target(url: &Url) -> &str {
    &url[Position::BeforePath..Position::AfterQuery]
}

pub(super) fn redirect(
    status: u16,
    location: Option<&str>,
    at: &Url,
) -> Result<Option<Url>, CanvasError> {
    if !matches!(status, 301 | 302 | 303 | 307 | 308) {
        return Ok(None);
    }
    let Some(location) = location else {
        return Ok(None);
    };
    let next = at
        .join(location.trim())
        .map_err(|e| CanvasError::Failed(format!("a redirect to nowhere usable: {e}")))?;
    if !matches!(next.scheme(), "http" | "https") {
        return Err(CanvasError::Failed(
            "a redirect to something that is not a web page".into(),
        ));
    }
    Ok(Some(next))
}

/// Canvas answers a signed-in user who may not see something with a 401 whose
/// JSON says `"unauthorized"`; only the other 401s are a dead session.
fn is_authorisation_failure(body: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("status")?.as_str().map(|s| s == "unauthorized"))
        .unwrap_or(false)
}

fn leads_to_sign_in(at: &Url, location: &str) -> bool {
    let Ok(target) = at.join(location.trim()) else {
        return false;
    };
    if target
        .host_str()
        .is_some_and(|h| h.eq_ignore_ascii_case(SSO_HOST))
    {
        return true;
    }
    is_canvas(&target) && (target.path() == "/login" || target.path().starts_with("/login/"))
}

/// Whether an answer that oculus-keyd had already signed in again for says
/// the session is dead. keyd retried once, so there is nothing left to try.
fn judge(
    status: u16,
    location: Option<&str>,
    body: &[u8],
    signin: Option<LoginError>,
    at: &Url,
) -> Result<(), CanvasError> {
    if let Some(why) = signin {
        return Err(CanvasError::Expired(Some(why)));
    }
    let dead = match status {
        401 => !is_authorisation_failure(body),
        301 | 302 | 303 | 307 | 308 => location.is_some_and(|l| leads_to_sign_in(at, l)),
        _ => false,
    };
    if dead {
        return Err(CanvasError::Expired(None));
    }
    Ok(())
}

fn backoff(attempt: u32) {
    if !cfg!(test) {
        std::thread::sleep(Duration::from_millis(500 * (attempt as u64 + 1)));
    }
}

/// `attempt` up to `RETRIES` more times while it fails to reach the host or
/// the host answers 5xx. Only a GET: a POST is not safe to send twice.
fn retried(
    get: bool,
    mut attempt: impl FnMut() -> Result<Hop, CanvasError>,
) -> Result<Hop, CanvasError> {
    let mut tries = 0;
    loop {
        let again = match attempt() {
            Ok(hop) if hop.status >= 500 => Ok(hop),
            Err(CanvasError::Unreachable(why)) => Err(CanvasError::Unreachable(why)),
            done => return done,
        };
        if !get || tries >= RETRIES {
            return again;
        }
        backoff(tries);
        tries += 1;
    }
}

impl Canvas {
    /// One request to Canvas through oculus-keyd, redirects not followed.
    /// `Err(Expired)` when the session is dead after oculus-keyd's own
    /// sign-in and retry. Retries a GET that got no answer or a 5xx.
    pub(crate) fn hop(
        &self,
        method: &str,
        at: &Url,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<Hop, CanvasError> {
        if let Some(expired) = self.expired() {
            return Err(expired);
        }
        self.latch(retried(method == "GET", || {
            let r = self
                .keyd
                .send("canvas", method, target(at), headers, body, SESSION_TIMEOUT)
                .map_err(CanvasError::from_keyd)?;
            judge(
                r.status,
                header(&r.headers, "location"),
                &r.body,
                r.signin,
                at,
            )?;
            Ok(Hop {
                status: r.status,
                headers: r.headers,
                body: r.body,
            })
        }))
    }

    /// `hop`, but the body is left on the wire. A GET only.
    pub(super) fn hop_stream(&self, at: &Url) -> Result<Streamed, CanvasError> {
        if let Some(expired) = self.expired() {
            return Err(expired);
        }
        let mut tries = 0;
        let streamed = loop {
            let sent = self
                .keyd
                .send_stream("canvas", "GET", target(at), &[], b"", SESSION_TIMEOUT)
                .map_err(CanvasError::from_keyd);
            match sent {
                Err(CanvasError::Unreachable(_)) if tries < RETRIES => {
                    backoff(tries);
                    tries += 1;
                }
                Err(e) => break Err(e),
                Ok(r) => break judged_stream(r, at),
            }
        };
        self.latch(streamed)
    }
}

fn judged_stream(r: StreamedResponse, at: &Url) -> Result<Streamed, CanvasError> {
    let StreamedResponse {
        status,
        headers,
        signin,
        mut body,
    } = r;
    let mut head = Vec::new();
    if status == 401 {
        body.by_ref()
            .take(REJECTION_BODY)
            .read_to_end(&mut head)
            .ok();
    }
    judge(status, header(&headers, "location"), &head, signin, at)?;
    Ok(Streamed {
        status,
        headers,
        body: Box::new(body),
    })
}

fn agent(read: Option<Duration>) -> ureq::Agent {
    let builder = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(30));
    match read {
        Some(read) => builder.timeout_read(read),
        None => builder.timeout(TIMEOUT),
    }
    .build()
}

/// Names the host and why, never the URL: a file link is signed.
fn unreachable(url: &Url, e: &ureq::Transport) -> CanvasError {
    CanvasError::Unreachable(format!(
        "{}: {}",
        url.host_str().unwrap_or("the file host"),
        e.message()
            .map_or_else(|| e.kind().to_string(), str::to_string)
    ))
}

fn headers_of(resp: &ureq::Response) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for name in resp.headers_names() {
        for value in resp.all(&name) {
            out.push((name.to_lowercase(), value.to_string()));
        }
    }
    out
}

/// A GET to a host that is not Canvas: no cookie, no redirects followed.
pub(super) fn direct(url: &Url) -> Result<Hop, CanvasError> {
    retried(true, || {
        let resp = match agent(None).get(url.as_str()).call() {
            Ok(resp) => resp,
            Err(ureq::Error::Status(_, resp)) => resp,
            Err(ureq::Error::Transport(t)) => return Err(unreachable(url, &t)),
        };
        let (status, headers) = (resp.status(), headers_of(&resp));
        let mut body = Vec::new();
        resp.into_reader()
            .read_to_end(&mut body)
            .map_err(|e| CanvasError::Unreachable(format!("the answer was cut off: {e}")))?;
        Ok(Hop {
            status,
            headers,
            body,
        })
    })
}

/// `direct`, with the body left on the wire; each read is bounded by `TIMEOUT`.
pub(super) fn direct_stream(url: &Url) -> Result<Streamed, CanvasError> {
    let resp = match agent(Some(TIMEOUT)).get(url.as_str()).call() {
        Ok(resp) => resp,
        Err(ureq::Error::Status(_, resp)) => resp,
        Err(ureq::Error::Transport(t)) => return Err(unreachable(url, &t)),
    };
    Ok(Streamed {
        status: resp.status(),
        headers: headers_of(&resp),
        body: Box::new(resp.into_reader()),
    })
}
