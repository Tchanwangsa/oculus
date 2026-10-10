//! Where a credential may be sent and how it is attached.

use std::time::Duration;

use super::{CANVAS, ED};
use crate::names;
use crate::paths::CANVAS_BASE;

/// Longest a session route waits for one read from its origin.
pub const SESSION_READ_TIMEOUT: Duration = Duration::from_secs(180);

/// How the credential reaches the origin. Each variant names the vault entry
/// it is read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth {
    /// `Authorization: Bearer <key>`: a cloud service's key.
    Bearer(&'static str),
    /// `Cookie: <session>`: Canvas's cookie header, which keyd also keeps
    /// current from the `Set-Cookie` the origin sends back.
    Cookie(&'static str),
    /// `<name>: <session>`: Ed's `x-token`.
    Header {
        name: &'static str,
        secret: &'static str,
    },
}

impl Auth {
    /// The vault entry holding the credential.
    pub fn secret(self) -> &'static str {
        match self {
            Auth::Bearer(secret) | Auth::Cookie(secret) | Auth::Header { secret, .. } => secret,
        }
    }

    /// The header and value to send.
    pub(super) fn header(self, value: &str) -> (&'static str, String) {
        match self {
            Auth::Bearer(_) => ("Authorization", format!("Bearer {value}")),
            Auth::Cookie(_) => ("Cookie", value.to_string()),
            Auth::Header { name, .. } => (name, value.to_string()),
        }
    }
}

/// Where a credential may be sent: `origin` (scheme and host, no trailing
/// slash) and the prefix every forwarded path must start with.
#[derive(Debug, Clone)]
pub struct Route {
    /// What a client calls it.
    pub name: &'static str,
    pub origin: String,
    pub prefix: &'static str,
    pub auth: Auth,
    /// The longest one read from the origin may take; `None` waits for ever.
    pub read_timeout: Option<Duration>,
}

impl Route {
    fn cloud(secret: &'static str, origin: &str, prefix: &'static str) -> Route {
        Route {
            name: secret,
            origin: origin.to_string(),
            prefix,
            auth: Auth::Bearer(secret),
            read_timeout: None,
        }
    }

    fn session(name: &'static str, origin: &str, prefix: &'static str, auth: Auth) -> Route {
        Route {
            name,
            origin: origin.to_string(),
            prefix,
            auth,
            read_timeout: Some(SESSION_READ_TIMEOUT),
        }
    }

    /// A login session rather than a cloud key: a wider path charset, a few
    /// more request headers, and no cookie or token in any answer.
    pub fn is_session(&self) -> bool {
        !matches!(self.auth, Auth::Bearer(_))
    }

    /// The headers a client may set; keyd adds the credential itself.
    pub(super) fn client_headers(&self) -> &'static [&'static str] {
        if self.is_session() {
            &[
                "content-type",
                "accept",
                "range",
                "if-none-match",
                "if-modified-since",
            ]
        } else {
            &["content-type", "accept"]
        }
    }
}

/// The routes keyd forwards for. A name with no route cannot be forwarded.
#[derive(Debug, Clone)]
pub struct Routes(Vec<Route>);

impl Routes {
    pub fn compiled() -> Routes {
        Routes(vec![
            Route::cloud(names::VOYAGE, "https://api.voyageai.com", "/v1/"),
            Route::cloud(names::MINERU, "https://mineru.net", "/api/v4/"),
            Route::cloud(names::GROQ, "https://api.groq.com", "/openai/v1/"),
            Route::session(
                CANVAS,
                CANVAS_BASE,
                "/",
                Auth::Cookie(names::SESSION_CANVAS),
            ),
            Route::session(
                ED,
                "https://edstem.org",
                "/api/",
                Auth::Header {
                    name: "x-token",
                    secret: names::SESSION_ED,
                },
            ),
        ])
    }

    pub fn get(&self, name: &str) -> Option<&Route> {
        self.0.iter().find(|r| r.name == name)
    }

    /// Points `name` at a loopback origin, so tests can run a fake upstream.
    /// Debug builds only: a release keyd has no way to move an origin.
    #[cfg(debug_assertions)]
    pub fn with_origin(mut self, name: &str, origin: &str) -> Result<Routes, String> {
        let rest = origin
            .strip_prefix("http://127.0.0.1:")
            .ok_or("a test origin must be http://127.0.0.1:<port>")?;
        if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
            return Err("a test origin must be http://127.0.0.1:<port>".to_string());
        }
        self.route_mut(name)?.origin = origin.to_string();
        Ok(self)
    }

    /// Moves `name`'s origin to `origin` without `with_origin`'s checks; the
    /// caller has already judged it (`State::with_origins`).
    #[cfg(debug_assertions)]
    pub(crate) fn set_origin(&mut self, name: &str, origin: &str) {
        if let Ok(route) = self.route_mut(name) {
            route.origin = origin.to_string();
        }
    }

    /// Shortens (or lifts) a route's read timeout, so a test need not wait
    /// out 180 s. Debug builds only.
    #[cfg(debug_assertions)]
    pub fn with_read_timeout(
        mut self,
        name: &str,
        timeout: Option<Duration>,
    ) -> Result<Routes, String> {
        self.route_mut(name)?.read_timeout = timeout;
        Ok(self)
    }

    #[cfg(debug_assertions)]
    fn route_mut(&mut self, name: &str) -> Result<&mut Route, String> {
        self.0
            .iter_mut()
            .find(|r| r.name == name)
            .ok_or_else(|| format!("{name} has no route"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_route_has_one_fixed_origin_prefix_and_credential_and_nothing_else_does() {
        let routes = Routes::compiled();
        for (name, origin, prefix, auth) in [
            (
                "voyage",
                "https://api.voyageai.com",
                "/v1/",
                Auth::Bearer("voyage"),
            ),
            (
                "mineru",
                "https://mineru.net",
                "/api/v4/",
                Auth::Bearer("mineru"),
            ),
            (
                "groq",
                "https://api.groq.com",
                "/openai/v1/",
                Auth::Bearer("groq"),
            ),
            (
                "canvas",
                "https://canvas.lms.unimelb.edu.au",
                "/",
                Auth::Cookie("session.canvas"),
            ),
            (
                "ed",
                "https://edstem.org",
                "/api/",
                Auth::Header {
                    name: "x-token",
                    secret: "session.ed",
                },
            ),
        ] {
            let route = routes.get(name).unwrap();
            assert_eq!(
                (route.origin.as_str(), route.prefix, route.auth),
                (origin, prefix, auth),
                "{name}"
            );
        }
        for name in [
            "okta.username",
            "okta.password",
            "okta.totp_secret",
            "session.canvas",
            "session.sso",
            "sso",
            "nope",
        ] {
            assert!(routes.get(name).is_none(), "{name}");
        }
    }

    #[test]
    fn only_the_session_routes_have_a_stall_detector() {
        let routes = Routes::compiled();
        for name in ["voyage", "mineru", "groq"] {
            let route = routes.get(name).unwrap();
            assert!(
                !route.is_session() && route.read_timeout.is_none(),
                "{name}"
            );
        }
        for name in ["canvas", "ed"] {
            let route = routes.get(name).unwrap();
            assert!(route.is_session(), "{name}");
            assert_eq!(route.read_timeout, Some(Duration::from_secs(180)));
        }
    }

    #[test]
    fn a_test_origin_must_be_loopback_http_with_a_port() {
        for bad in [
            "https://evil.example",
            "http://127.0.0.1:",
            "http://127.0.0.1:80/x",
            "http://localhost:80",
            "http://127.0.0.1:80@evil",
        ] {
            assert!(
                Routes::compiled().with_origin("voyage", bad).is_err(),
                "{bad}"
            );
            assert!(Routes::compiled().with_origin("canvas", bad).is_err());
        }
        let moved = Routes::compiled()
            .with_origin("voyage", "http://127.0.0.1:4321")
            .unwrap()
            .with_origin("ed", "http://127.0.0.1:4322")
            .unwrap();
        assert_eq!(moved.get("voyage").unwrap().origin, "http://127.0.0.1:4321");
        assert_eq!(moved.get("ed").unwrap().origin, "http://127.0.0.1:4322");
        assert!(Routes::compiled()
            .with_origin("nope", "http://127.0.0.1:1")
            .is_err());
    }
}
