//! The `forward` op's HTTP half: one request to a secret's fixed origin, with
//! keyd adding `Authorization: Bearer <secret>`.
//!
//! The client names a path under the origin's prefix and may set only
//! `Content-Type` and `Accept`. The answer's status, headers and body come back
//! as the origin sent them, so each client's own 401/429/quota handling keeps
//! working. Redirects are returned, never followed, so nothing leaves the
//! origin. ureq's 30 s connect timeout is the only timeout: an embed or a parse
//! takes minutes (docs/architecture.md).

use std::io::Read;
use std::time::Duration;

use serde_json::Value;

use crate::framing::MAX_BODY;
use crate::names;
use crate::ops::OpError;

/// Where a secret may be sent: `origin` (scheme and host, no trailing slash)
/// and the prefix every forwarded path must start with.
#[derive(Debug, Clone)]
pub struct Route {
    pub secret: &'static str,
    pub origin: String,
    pub prefix: &'static str,
}

/// The routes keyd forwards for. A secret with no route cannot be forwarded.
#[derive(Debug, Clone)]
pub struct Routes(Vec<Route>);

impl Routes {
    pub fn compiled() -> Routes {
        Routes(vec![Route { secret: names::VOYAGE, origin: "https://api.voyageai.com".to_string(), prefix: "/v1/" }])
    }

    pub fn get(&self, secret: &str) -> Option<&Route> {
        self.0.iter().find(|r| r.secret == secret)
    }

    /// Points `secret` at a loopback origin, so tests can run a fake upstream.
    /// Debug builds only: a release keyd has no way to move an origin.
    #[cfg(debug_assertions)]
    pub fn with_origin(mut self, secret: &str, origin: &str) -> Result<Routes, String> {
        let rest = origin.strip_prefix("http://127.0.0.1:").ok_or("a test origin must be http://127.0.0.1:<port>")?;
        if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
            return Err("a test origin must be http://127.0.0.1:<port>".to_string());
        }
        let route = self.0.iter_mut().find(|r| r.secret == secret).ok_or_else(|| format!("{secret} has no route"))?;
        route.origin = origin.to_string();
        Ok(self)
    }
}

/// The only headers a client may set; keyd adds `Authorization` itself.
const CLIENT_HEADERS: &[&str] = &["content-type", "accept"];
const MAX_PATH: usize = 2048;

/// A validated `forward` request, minus its body.
#[derive(Debug)]
pub struct Call {
    pub method: &'static str,
    pub path: String,
    pub headers: Vec<(String, String)>,
}

impl Call {
    /// Checks the header's `method`, `path` and `headers` against `route`.
    /// Details name what was wrong, never a header's value.
    pub fn parse(req: &Value, route: &Route, body: &[u8]) -> Result<Call, OpError> {
        let method = match req.get("method").and_then(Value::as_str) {
            Some("GET") => "GET",
            Some("POST") => "POST",
            _ => return Err(OpError::new("request", "method must be GET or POST")),
        };
        if method == "GET" && !body.is_empty() {
            return Err(OpError::new("request", "a GET takes no body"));
        }
        let path = req.get("path").and_then(Value::as_str).ok_or_else(|| OpError::new("request", "missing \"path\""))?;
        check_path(path, route.prefix)?;

        let mut headers = Vec::new();
        if let Some(list) = req.get("headers") {
            let list = list.as_array().ok_or_else(|| OpError::new("request", "headers must be a list of [name, value]"))?;
            for pair in list {
                let (name, value) = match pair.as_array().map(Vec::as_slice) {
                    Some([Value::String(n), Value::String(v)]) => (n, v),
                    _ => return Err(OpError::new("request", "headers must be a list of [name, value]")),
                };
                if !CLIENT_HEADERS.iter().any(|h| h.eq_ignore_ascii_case(name)) {
                    return Err(OpError::new("request", format!("header {} may not be set", printable(name))));
                }
                if value.bytes().any(|b| b.is_ascii_control()) || !value.is_ascii() {
                    return Err(OpError::new("request", format!("header {name} has a control or non-ASCII character")));
                }
                headers.push((name.clone(), value.clone()));
            }
        }
        Ok(Call { method, path: path.to_string(), headers })
    }
}

/// A path under `prefix`, written so no URL parser can read it as another
/// origin or another directory: unreserved characters, `/`, and a query of
/// `=` and `&`. No `%`, because a parser may decode `%2e%2e` to `..`.
fn check_path(path: &str, prefix: &str) -> Result<(), OpError> {
    let bad = |why: &str| Err(OpError::new("request", format!("path {why}")));
    if path.len() > MAX_PATH {
        return bad("is too long");
    }
    if !path.starts_with(prefix) {
        return bad(&format!("must start with {prefix}"));
    }
    let allowed = |b: u8| b.is_ascii_alphanumeric() || b"-._~/?=&".contains(&b);
    if !path.bytes().all(allowed) {
        return bad("has a character outside [A-Za-z0-9-._~/?=&]");
    }
    let route_part = path.split('?').next().unwrap_or("");
    if route_part.contains("//") {
        return bad("has an empty segment");
    }
    if route_part.split('/').any(|seg| seg == "." || seg == "..") {
        return bad("has a dot segment");
    }
    Ok(())
}

/// Client text for an error detail: a short plain token, or a placeholder.
fn printable(name: &str) -> String {
    if !name.is_empty() && name.len() <= 64 && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        name.to_string()
    } else {
        "(unprintable)".to_string()
    }
}

/// What the origin answered, whatever the status.
#[derive(Debug)]
pub struct Answer {
    pub status: u16,
    /// Names lowercased (as ureq reports them), in the order they arrived.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// One HTTPS agent for the process: its connection pool outlives a request.
pub struct Upstream {
    agent: ureq::Agent,
}

impl Upstream {
    pub fn new() -> Upstream {
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .try_proxy_from_env(false)
            .timeout_connect(Duration::from_secs(30))
            .build();
        Upstream { agent }
    }

    /// An `upstream` error only when no answer arrived (DNS, connect, TLS, a
    /// reset). Its detail is the failure's kind and cause, never the URL, a
    /// header or the body.
    pub fn send(&self, origin: &str, call: &Call, key: &str, body: &[u8]) -> Result<Answer, OpError> {
        let url = format!("{origin}{}", call.path);
        let mut request = self.agent.request(call.method, &url);
        for (name, value) in &call.headers {
            request = request.set(name, value);
        }
        request = request.set("Authorization", &format!("Bearer {key}"));
        let sent = if call.method == "GET" { request.call() } else { request.send_bytes(body) };
        let response = match sent {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(t)) => return Err(OpError::new("upstream", transport_detail(&t))),
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

        let mut out = Vec::new();
        response
            .into_reader()
            .take(MAX_BODY + 1)
            .read_to_end(&mut out)
            .map_err(|e| OpError::new("upstream", format!("reading the answer: {e}")))?;
        if out.len() as u64 > MAX_BODY {
            return Err(OpError::new("upstream", format!("the answer is over {MAX_BODY} bytes")));
        }
        Ok(Answer { status, headers, body: out })
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn route() -> Route {
        Routes::compiled().get("voyage").unwrap().clone()
    }

    fn call(req: Value, body: &[u8]) -> Result<Call, OpError> {
        Call::parse(&req, &route(), body)
    }

    #[test]
    fn only_voyage_has_a_route_and_its_origin_is_fixed() {
        let routes = Routes::compiled();
        assert_eq!(routes.get("voyage").unwrap().origin, "https://api.voyageai.com");
        for name in ["mineru", "groq", "okta.password", "nope"] {
            assert!(routes.get(name).is_none(), "{name}");
        }
    }

    #[test]
    fn a_test_origin_must_be_loopback_http_with_a_port() {
        for bad in ["https://evil.example", "http://127.0.0.1:", "http://127.0.0.1:80/x", "http://localhost:80", "http://127.0.0.1:80@evil"] {
            assert!(Routes::compiled().with_origin("voyage", bad).is_err(), "{bad}");
        }
        let moved = Routes::compiled().with_origin("voyage", "http://127.0.0.1:4321").unwrap();
        assert_eq!(moved.get("voyage").unwrap().origin, "http://127.0.0.1:4321");
    }

    #[test]
    fn good_paths_and_headers_pass() {
        let c = call(
            json!({"method": "POST", "path": "/v1/multimodalembeddings", "headers": [["Content-Type", "application/json"], ["accept", "application/json"]]}),
            b"{}",
        )
        .unwrap();
        assert_eq!(c.method, "POST");
        assert_eq!(c.headers.len(), 2);
        assert!(call(json!({"method": "GET", "path": "/v1/models?limit=5&x=y"}), b"").is_ok());
    }

    #[test]
    fn paths_that_could_leave_the_prefix_or_the_origin_are_refused() {
        for path in [
            "/v2/embeddings",
            "v1/embeddings",
            "/v1",
            "/v1/../admin",
            "/v1/./x",
            "/v1/a/..",
            "/v1//evil.example/x",
            "/v1/%2e%2e/admin",
            "/v1/x\\y",
            "/v1/x\r\nHost: evil",
            "/v1/x\u{0}",
            "/v1/@evil.example",
            "/v1/x#frag",
            "/v1/https://evil.example",
            "/v1/ü",
        ] {
            let err = call(json!({"method": "POST", "path": path}), b"").unwrap_err();
            assert_eq!(err.kind, "request", "{path:?}");
        }
        let long = format!("/v1/{}", "a".repeat(MAX_PATH));
        assert!(call(json!({"method": "POST", "path": long}), b"").is_err());
        assert!(call(json!({"method": "POST"}), b"").is_err());
    }

    #[test]
    fn methods_other_than_get_and_post_are_refused() {
        for method in [json!("PUT"), json!("DELETE"), json!("get"), json!("CONNECT"), json!(5), Value::Null] {
            assert!(call(json!({"method": method, "path": "/v1/x"}), b"").is_err(), "{method}");
        }
        assert!(call(json!({"method": "GET", "path": "/v1/x"}), b"body").is_err(), "a GET with a body");
    }

    #[test]
    fn any_header_but_content_type_and_accept_is_refused_without_echoing_its_value() {
        for name in ["Authorization", "authorization", "Host", "Cookie", "X-Forwarded-For", "Content-Length", "Transfer-Encoding"] {
            let err = call(json!({"method": "POST", "path": "/v1/x", "headers": [[name, "Bearer pa-SECRET"]]}), b"").unwrap_err();
            assert_eq!(err.kind, "request", "{name}");
            assert!(!err.detail.contains("pa-SECRET"), "{}", err.detail);
        }
        let err = call(json!({"method": "POST", "path": "/v1/x", "headers": [["Content-Type", "a\r\nAuthorization: x"]]}), b"").unwrap_err();
        assert_eq!(err.kind, "request");
        for shape in [json!("Content-Type"), json!([["Content-Type"]]), json!([["Accept", 1]]), json!({"Accept": "x"})] {
            assert!(call(json!({"method": "POST", "path": "/v1/x", "headers": shape}), b"").is_err(), "{shape}");
        }
    }
}
