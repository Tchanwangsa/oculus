//! Who may talk to the server. Host must name this Mac (one of its current
//! addresses, or loopback) with the bound port, which defeats DNS rebinding;
//! a browser's Origin, when sent, must be that same `http://<Host>`. Share
//! endpoints then need `Authorization: Bearer <shareId>.<secret>`.

use super::http::{error, session_ended, Reply};
use crate::companion::core::Core;
use crate::companion::grants::{Access, Share};
use hyper::header::{HeaderValue, AUTHORIZATION, WWW_AUTHENTICATE};
use hyper::{HeaderMap, StatusCode};
use std::net::IpAddr;
use std::sync::Arc;

pub fn host_allowed(core: &Core, port: u16, host: Option<&str>, origin: Option<&str>) -> bool {
    let Some(host) = host else {
        return false;
    };
    let Some((name, named_port)) = split_host(host) else {
        return false;
    };
    if named_port != port {
        return false;
    }
    let ours = name.eq_ignore_ascii_case("localhost")
        || name
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback() || core.addresses.contains(ip));
    ours && origin.is_none_or(|origin| {
        origin
            .strip_prefix("http://")
            .is_some_and(|rest| rest.eq_ignore_ascii_case(host))
    })
}

/// The IP a Host header names, if it names one rather than a name.
pub fn host_ip(host: &str) -> Option<IpAddr> {
    split_host(host)?.0.parse().ok()
}

/// `name:port` or `[v6]:port`; a Host without a port is refused.
fn split_host(host: &str) -> Option<(&str, u16)> {
    let (name, port) = if let Some(rest) = host.strip_prefix('[') {
        let (name, rest) = rest.split_once(']')?;
        (name, rest.strip_prefix(':')?)
    } else {
        host.rsplit_once(':')
            .filter(|(name, _)| !name.contains(':'))?
    };
    Some((name, port.parse().ok()?))
}

pub fn header(headers: &HeaderMap, name: impl hyper::header::AsHeaderName) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

/// The share `path_id` names, if the request carries its bearer token;
/// otherwise the refusal to send.
pub fn authenticate(
    core: &Core,
    path_id: &str,
    headers: &HeaderMap,
) -> Result<Arc<Share>, Box<Reply>> {
    let Some(token) = header(headers, AUTHORIZATION).and_then(bearer) else {
        let mut reply = error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "This request needs the share link's key.",
        );
        reply
            .headers_mut()
            .insert(WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        return Err(Box::new(reply));
    };
    let forbidden = || {
        Box::new(error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "This share link is not valid.",
        ))
    };
    let Some((id, secret)) = token.split_once('.') else {
        return Err(forbidden());
    };
    if id != path_id {
        return Err(forbidden());
    }
    match core.grants.authorize(id, secret) {
        Access::Granted(share) => Ok(share),
        Access::Ended(_) => Err(Box::new(session_ended())),
        Access::Denied => Err(forbidden()),
    }
}

fn bearer(value: &str) -> Option<&str> {
    let (scheme, token) = value.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| token.trim())
        .filter(|token| !token.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_need_a_port_and_v6_needs_brackets() {
        assert_eq!(
            split_host("192.168.1.2:43821"),
            Some(("192.168.1.2", 43821))
        );
        assert_eq!(split_host("[fd00::1]:80"), Some(("fd00::1", 80)));
        assert_eq!(split_host("localhost"), None);
        assert_eq!(split_host("[fd00::1]"), None);
        assert_eq!(split_host("fd00::1"), None);
    }

    #[test]
    fn bearer_tokens_are_read_case_insensitively() {
        assert_eq!(bearer("Bearer a.b"), Some("a.b"));
        assert_eq!(bearer("bearer  a.b "), Some("a.b"));
        assert_eq!(bearer("Basic a.b"), None);
        assert_eq!(bearer("Bearer "), None);
    }
}
