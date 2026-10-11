//! The cookie jar the sign-in walks its redirects with.

use std::collections::BTreeMap;

use crate::session::cookie::parse_set_cookie;

/// Cookies kept per host: Okta's `sid` must never reach Canvas, nor Canvas's
/// session Okta.
#[derive(Default)]
pub(super) struct Jar(BTreeMap<String, BTreeMap<String, String>>);

impl Jar {
    /// Applies each `Set-Cookie` by the shared rule (`session::cookie`). The
    /// jar alone drops the quotes around a value and keeps its cookies sorted
    /// by name, which the sign-in's replayed headers have always been.
    pub(super) fn absorb(&mut self, host: &str, resp: &ureq::Response) {
        let now = crate::clock::now_secs();
        let jar = self.0.entry(host.to_string()).or_default();
        for raw in resp.all("set-cookie") {
            let Some(cookie) = parse_set_cookie(raw, now) else {
                continue;
            };
            let value = cookie.value.trim_matches('"');
            if cookie.remove || value.is_empty() {
                jar.remove(cookie.name);
            } else {
                jar.insert(cookie.name.to_string(), value.to_string());
            }
        }
    }

    pub(super) fn header(&self, host: &str) -> String {
        self.0
            .get(host)
            .map(|m| {
                m.iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default()
    }

    pub(super) fn has(&self, host: &str, name: &str) -> bool {
        self.0.get(host).is_some_and(|m| m.contains_key(name))
    }

    /// How many cookies `host` has set.
    pub(super) fn count(&self, host: &str) -> usize {
        self.0.get(host).map_or(0, |m| m.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookies_are_filed_per_host() {
        let mut jar = Jar::default();
        jar.0
            .entry("a.example".into())
            .or_default()
            .insert("sid".into(), "1".into());
        jar.0
            .entry("b.example".into())
            .or_default()
            .insert("other".into(), "2".into());
        assert_eq!(jar.header("a.example"), "sid=1");
        assert!(!jar.has("b.example", "sid"));
        assert_eq!(jar.header("nowhere.example"), "");
    }
}
