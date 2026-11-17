//! Where an old path went: a longest-prefix map from old data-dir-relative
//! paths to new ones. A folder's rule covers everything under it; a file
//! renamed on a clash, or picked out of its folder, has a longer rule of its
//! own that wins.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// One old prefix and where it went; every one is kept in `path_moves` for
/// old citations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub old: String,
    pub new: String,
}

/// The characters that may follow a prefix for it to match: a deeper path,
/// a `#page=` anchor, a `:L15` line, a query.
const BOUNDARIES: [char; 4] = ['/', '#', ':', '?'];

#[derive(Default)]
pub struct Rules {
    by_old: HashMap<String, String>,
}

impl Rules {
    pub fn new(rules: &[Rule]) -> Self {
        let mut out = Self::default();
        for rule in rules {
            out.by_old.insert(rule.old.clone(), rule.new.clone());
        }
        out
    }

    /// Where `path` went, with whatever followed the matched prefix carried
    /// over; `None` when no rule covers it. Matching is exact in case, so a
    /// path already in the new tree never matches an old rule.
    pub fn map(&self, path: &str) -> Option<String> {
        let mut ends: Vec<usize> = path
            .char_indices()
            .filter(|(_, c)| BOUNDARIES.contains(c))
            .map(|(i, _)| i)
            .collect();
        ends.push(path.len());
        ends.into_iter().rev().find_map(|end| {
            let new = self.by_old.get(&path[..end])?;
            Some(format!("{new}{}", &path[end..]))
        })
    }

    /// `map`, or the path itself when nothing moved it.
    pub fn apply(&self, path: &str) -> String {
        self.map(path).unwrap_or_else(|| path.to_string())
    }
}

/// The rules in the order they were made, the last one for an old path
/// winning: what the plan records and `Rules` reads.
#[derive(Default)]
pub(super) struct RuleList {
    pub rules: Vec<Rule>,
    index: HashMap<String, usize>,
    live: Rules,
}

impl RuleList {
    pub fn add(&mut self, old: &str, new: &str) {
        let rule = Rule {
            old: old.to_string(),
            new: new.to_string(),
        };
        self.live.by_old.insert(rule.old.clone(), rule.new.clone());
        match self.index.get(old) {
            Some(&i) => self.rules[i] = rule,
            None => {
                self.index.insert(old.to_string(), self.rules.len());
                self.rules.push(rule);
            }
        }
    }

    /// `Rules::apply` over the rules made so far.
    pub fn apply(&self, path: &str) -> String {
        self.live.apply(path)
    }
}
