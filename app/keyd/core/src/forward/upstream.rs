//! The HTTPS client behind `forward`.

use std::io::Read;
use std::sync::Mutex;
use std::time::Duration;

use super::{Call, Route};
use crate::framing::MAX_BODY;
use crate::ops::OpError;

/// An answer whose head has arrived and whose body has not been read.
pub struct Opened {
    pub status: u16,
    /// Names lowercased (as ureq reports them), in the order they arrived.
    pub headers: Vec<(String, String)>,
    pub body: Box<dyn Read + Send>,
}

/// One HTTPS agent per read timeout for the process: its connection pool
/// outlives a request. No agent follows a redirect, reads a proxy from the
/// environment, or decompresses, so the bytes are the origin's and a
/// credential goes only where its route says.
pub struct Upstream {
    plain: ureq::Agent,
    timed: Mutex<Vec<(Duration, ureq::Agent)>>,
}

fn agent(read_timeout: Option<Duration>) -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new()
        .redirects(0)
        .try_proxy_from_env(false)
        .timeout_connect(Duration::from_secs(30));
    if let Some(timeout) = read_timeout {
        builder = builder.timeout_read(timeout);
    }
    builder.build()
}

impl Upstream {
    pub fn new() -> Upstream {
        Upstream {
            plain: agent(None),
            timed: Mutex::new(Vec::new()),
        }
    }

    fn agent_for(&self, read_timeout: Option<Duration>) -> ureq::Agent {
        let Some(timeout) = read_timeout else {
            return self.plain.clone();
        };
        let mut agents = self.timed.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((_, existing)) = agents.iter().find(|(t, _)| *t == timeout) {
            return existing.clone();
        }
        let fresh = agent(Some(timeout));
        agents.push((timeout, fresh.clone()));
        fresh
    }

    /// Sends `call` to `route`'s origin with `credential` attached the way
    /// the route says, and returns at the end of the answer's head. An
    /// `upstream` error only when no answer arrived (DNS, connect, TLS, a
    /// reset, a read that stalled past the route's timeout). Its detail is
    /// the failure's kind and cause, never the URL, a header or the body.
    pub fn open(
        &self,
        route: &Route,
        call: &Call,
        credential: &str,
        body: &[u8],
    ) -> Result<Opened, OpError> {
        let url = format!("{}{}", route.origin, call.path);
        let mut request = self
            .agent_for(route.read_timeout)
            .request(call.method, &url);
        for (name, value) in &call.headers {
            request = request.set(name, value);
        }
        let (name, value) = route.auth.header(credential);
        request = request.set(name, &value);
        let sent = if call.method == "GET" {
            request.call()
        } else {
            request.send_bytes(body)
        };
        let response = match sent {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(t)) => {
                return Err(OpError::new("upstream", transport_detail(&t)))
            }
        };

        let status = response.status();
        let mut headers = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for name in response.headers_names() {
            if seen.contains(&name) {
                continue;
            }
            for value in response.all(&name) {
                headers.push((name.clone(), value.to_string()));
            }
            seen.push(name);
        }
        Ok(Opened {
            status,
            headers,
            body: response.into_reader(),
        })
    }
}

/// The whole body, or an `upstream` error if it is over `MAX_BODY` or cannot
/// be read to its end.
pub fn read_capped(reader: impl Read) -> Result<Vec<u8>, OpError> {
    let mut out = Vec::new();
    reader
        .take(MAX_BODY + 1)
        .read_to_end(&mut out)
        .map_err(|e| OpError::new("upstream", format!("reading the answer: {e}")))?;
    if out.len() as u64 > MAX_BODY {
        return Err(OpError::new(
            "upstream",
            format!("the answer is over {MAX_BODY} bytes"),
        ));
    }
    Ok(out)
}

/// The kind, then each cause. ureq's own message is left out: it can name the
/// URL.
fn transport_detail(t: &ureq::Transport) -> String {
    let mut detail = t.kind().to_string();
    let mut source = std::error::Error::source(t);
    while let Some(cause) = source {
        detail.push_str(": ");
        detail.push_str(&cause.to_string());
        source = cause.source();
    }
    detail
}
