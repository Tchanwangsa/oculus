//! The cookie jar the sign-in walks its redirects with.

use std::collections::BTreeMap;

/// Cookies kept per host: Okta's `sid` must never reach Canvas, nor Canvas's
/// session Okta.
#[derive(Default)]
pub(super) struct Jar(BTreeMap<String, BTreeMap<String, String>>);

impl Jar {
    pub(super) fn absorb(&mut self, host: &str, resp: &ureq::Response) {
        let jar = self.0.entry(host.to_string()).or_default();
        for raw in resp.all("set-cookie") {
            let Some((k, v)) = raw.split(';').next().unwrap_or("").split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim().trim_matches('"'));
            if v.is_empty() {
                jar.remove(k);
            } else {
                jar.insert(k.to_string(), v.to_string());
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
