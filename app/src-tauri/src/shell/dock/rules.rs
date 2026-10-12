//! The one-hop rule as data: which paths of a service are its messages, its
//! login, a shared item, or off limits. The table is `scripts/rules/<id>.json`,
//! read here by the native guard (`page.rs`) and prepended verbatim to the
//! JS guard (`scripts/hop.js`), so the two can't drift. Both are tested
//! against `scripts/rules/<id>.cases.json`. See `docs/dock.md`.
//!
//! A template is a list of path segments: a literal must equal the segment,
//! `:_` matches any one, `:key` matches any one and captures it. Extra
//! trailing segments are allowed, and the first matching template wins.

use serde::Deserialize;
use url::Url;

pub const INSTAGRAM: &str = include_str!("scripts/rules/instagram.json");

#[derive(Debug, Deserialize)]
pub struct Rules {
    /// Hosts whose paths the table judges; any other host is off-site.
    hosts: Vec<String>,
    /// The messages: entering one leaves any shared item.
    home: Vec<Vec<String>>,
    /// Login and the site's own plumbing: always allowed, hop state untouched.
    pass: Vec<Vec<String>>,
    items: Vec<ItemRule>,
}

#[derive(Debug, Deserialize)]
struct ItemRule {
    /// The item's kind in its identity, so a post's `/p/`, `/reel/` and
    /// `/reels/` URLs count as one item.
    kind: String,
    path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    Home,
    Pass,
    /// A shared item, by identity (`post:<code>`, `story:<owner>`, …).
    Item(String),
    Blocked,
    /// Another host, or not http(s).
    Offsite,
}

/// `Some(captured key)` (empty without a `:key`) when `template` matches.
fn matches<'a>(template: &[String], segments: &[&'a str]) -> Option<&'a str> {
    if segments.len() < template.len() {
        return None;
    }
    let mut key = "";
    for (want, got) in template.iter().zip(segments) {
        match want.as_str() {
            ":_" => {}
            ":key" => key = got,
            literal if literal == *got => {}
            _ => return None,
        }
    }
    Some(key)
}

impl Rules {
    pub fn parse(json: &str) -> Result<Rules, String> {
        serde_json::from_str(json).map_err(|e| format!("bad dock rules: {e}"))
    }

    pub fn classify(&self, url: &Url) -> Place {
        if !matches!(url.scheme(), "http" | "https") {
            return Place::Offsite;
        }
        let host = url.host_str().unwrap_or("").to_ascii_lowercase();
        if !self.hosts.contains(&host) {
            return Place::Offsite;
        }
        let segments: Vec<&str> = url.path().split('/').filter(|s| !s.is_empty()).collect();
        let hit =
            |templates: &[Vec<String>]| templates.iter().any(|t| matches(t, &segments).is_some());
        if hit(&self.home) {
            return Place::Home;
        }
        if hit(&self.pass) {
            return Place::Pass;
        }
        self.items
            .iter()
            .find_map(|rule| {
                matches(&rule.path, &segments)
                    .map(|key| Place::Item(format!("{}:{key}", rule.kind)))
            })
            .unwrap_or(Place::Blocked)
    }
}

/// Whether a page in `state` (`None` = in its messages, `Some(item)` =
/// inside that shared item) may go to `place`.
pub fn allows(state: Option<&str>, place: &Place) -> bool {
    match place {
        Place::Home | Place::Pass | Place::Offsite => true,
        Place::Item(id) => state.is_none_or(|s| s == id),
        Place::Blocked => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture's name for a place, the same strings `hop.js` returns.
    fn kind(place: &Place) -> &'static str {
        match place {
            Place::Home => "home",
            Place::Pass => "pass",
            Place::Item(_) => "item",
            Place::Blocked => "blocked",
            Place::Offsite => "offsite",
        }
    }

    #[derive(Deserialize)]
    struct Case {
        url: String,
        from: Option<String>,
        kind: String,
        id: Option<String>,
        allow: bool,
    }

    /// The same fixture runs through the shipped JS matcher in
    /// `app/tests/lib/dock/hop.test.ts`.
    #[test]
    fn instagram_matches_every_captured_shape() {
        let rules = Rules::parse(INSTAGRAM).unwrap();
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("scripts/rules/instagram.cases.json")).unwrap();
        assert!(cases.len() > 20);
        for case in cases {
            let url: Url = case.url.parse().unwrap();
            let place = rules.classify(&url);
            assert_eq!(kind(&place), case.kind, "{}", case.url);
            let id = match &place {
                Place::Item(id) => Some(id.clone()),
                _ => None,
            };
            assert_eq!(id, case.id, "{}", case.url);
            assert_eq!(
                allows(case.from.as_deref(), &place),
                case.allow,
                "{} from {:?}",
                case.url,
                case.from
            );
        }
    }

    #[test]
    fn templates_match_by_segment() {
        let t = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(matches(&t(&["p", ":key"]), &["p", "abc", "x"]), Some("abc"));
        assert_eq!(matches(&t(&["p", ":key"]), &["p"]), None);
        assert_eq!(
            matches(&t(&[":_", "p", ":key"]), &["u", "p", "c"]),
            Some("c")
        );
        assert_eq!(matches(&t(&["direct"]), &["direct"]), Some(""));
        assert_eq!(matches(&t(&["direct"]), &["directx"]), None);
    }
}
