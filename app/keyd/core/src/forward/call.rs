//! A validated `forward` request, minus its body.

use serde_json::Value;

use super::path::check_path;
use super::Route;
use crate::ops::OpError;

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
        let path = req
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| OpError::new("request", "missing \"path\""))?;
        check_path(path, route)?;

        let mut headers = Vec::new();
        if let Some(list) = req.get("headers") {
            let list = list.as_array().ok_or_else(|| {
                OpError::new("request", "headers must be a list of [name, value]")
            })?;
            for pair in list {
                let (name, value) = match pair.as_array().map(Vec::as_slice) {
                    Some([Value::String(n), Value::String(v)]) => (n, v),
                    _ => {
                        return Err(OpError::new(
                            "request",
                            "headers must be a list of [name, value]",
                        ))
                    }
                };
                if !route
                    .client_headers()
                    .iter()
                    .any(|h| h.eq_ignore_ascii_case(name))
                {
                    return Err(OpError::new(
                        "request",
                        format!("header {} may not be set", printable(name)),
                    ));
                }
                if value.bytes().any(|b| b.is_ascii_control()) || !value.is_ascii() {
                    return Err(OpError::new(
                        "request",
                        format!("header {name} has a control or non-ASCII character"),
                    ));
                }
                headers.push((name.clone(), value.clone()));
            }
        }
        Ok(Call {
            method,
            path: path.to_string(),
            headers,
        })
    }
}

/// Client text for an error detail: a short plain token, or a placeholder.
fn printable(name: &str) -> String {
    if !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        name.to_string()
    } else {
        "(unprintable)".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forward::Routes;
    use serde_json::json;

    fn route() -> Route {
        Routes::compiled().get("voyage").unwrap().clone()
    }

    fn call(req: Value, body: &[u8]) -> Result<Call, OpError> {
        Call::parse(&req, &route(), body)
    }

    fn call_on(name: &str, req: Value, body: &[u8]) -> Result<Call, OpError> {
        Call::parse(&req, Routes::compiled().get(name).unwrap(), body)
    }

    #[test]
    fn a_path_must_stay_under_its_own_routes_prefix() {
        let routes = Routes::compiled();
        let mineru = routes.get("mineru").unwrap();
        let groq = routes.get("groq").unwrap();
        let post = |path: &str| json!({"method": "POST", "path": path});
        assert!(Call::parse(&post("/api/v4/file-urls/batch"), mineru, b"{}").is_ok());
        assert!(Call::parse(&post("/openai/v1/audio/transcriptions"), groq, b"x").is_ok());
        assert!(Call::parse(&post("/v1/multimodalembeddings"), mineru, b"").is_err());
        assert!(Call::parse(&post("/api/v4/file-urls/batch"), groq, b"").is_err());
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
        assert!(call(
            json!({"method": "GET", "path": "/v1/models?limit=5&x=y"}),
            b""
        )
        .is_ok());
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
        let long = format!("/v1/{}", "a".repeat(super::super::path::MAX_PATH));
        assert!(call(json!({"method": "POST", "path": long}), b"").is_err());
        assert!(call(json!({"method": "POST"}), b"").is_err());
    }

    #[test]
    fn methods_other_than_get_and_post_are_refused_on_every_route() {
        for route in ["voyage", "canvas", "ed"] {
            let path = if route == "voyage" { "/v1/x" } else { "/api/x" };
            for method in [
                json!("PUT"),
                json!("PATCH"),
                json!("DELETE"),
                json!("HEAD"),
                json!("OPTIONS"),
                json!("get"),
                json!("CONNECT"),
                json!(5),
                Value::Null,
            ] {
                assert!(
                    call_on(route, json!({"method": method, "path": path}), b"").is_err(),
                    "{route} {method}"
                );
            }
            assert!(
                call_on(route, json!({"method": "GET", "path": path}), b"body").is_err(),
                "{route}: a GET with a body"
            );
            assert!(call_on(route, json!({"method": "GET", "path": path}), b"").is_ok());
            assert!(call_on(route, json!({"method": "POST", "path": path}), b"{}").is_ok());
        }
    }

    #[test]
    fn any_header_but_the_routes_own_is_refused_without_echoing_its_value() {
        for route in ["voyage", "canvas", "ed"] {
            let path = if route == "voyage" { "/v1/x" } else { "/api/x" };
            for name in [
                "Authorization",
                "authorization",
                "Host",
                "Cookie",
                "cookie",
                "X-Token",
                "x-token",
                "X-Forwarded-For",
                "Content-Length",
                "Transfer-Encoding",
                "Proxy-Authorization",
                "Origin",
                "X-CSRF-Token",
            ] {
                let err = call_on(
                    route,
                    json!({"method": "POST", "path": path, "headers": [[name, "Bearer pa-SECRET"]]}),
                    b"",
                )
                .unwrap_err();
                assert_eq!(err.kind, "request", "{route} {name}");
                assert!(!err.detail.contains("pa-SECRET"), "{}", err.detail);
            }
        }
        let err = call(json!({"method": "POST", "path": "/v1/x", "headers": [["Content-Type", "a\r\nAuthorization: x"]]}), b"").unwrap_err();
        assert_eq!(err.kind, "request");
        for shape in [
            json!("Content-Type"),
            json!([["Content-Type"]]),
            json!([["Accept", 1]]),
            json!({"Accept": "x"}),
        ] {
            assert!(
                call(
                    json!({"method": "POST", "path": "/v1/x", "headers": shape}),
                    b""
                )
                .is_err(),
                "{shape}"
            );
        }
    }

    #[test]
    fn range_and_validators_are_for_the_session_routes_only() {
        let headers = json!([
            ["Range", "bytes=0-99"],
            ["If-None-Match", "\"abc\""],
            ["If-Modified-Since", "Sat, 01 Jan 2000 00:00:00 GMT"],
            ["Accept", "*/*"]
        ]);
        for route in ["canvas", "ed"] {
            let c = call_on(
                route,
                json!({"method": "GET", "path": "/api/x", "headers": headers}),
                b"",
            )
            .unwrap();
            assert_eq!(c.headers.len(), 4, "{route}");
        }
        for name in ["Range", "If-None-Match", "If-Modified-Since"] {
            assert!(
                call(
                    json!({"method": "GET", "path": "/v1/x", "headers": [[name, "x"]]}),
                    b""
                )
                .is_err(),
                "{name}"
            );
        }
    }
}
