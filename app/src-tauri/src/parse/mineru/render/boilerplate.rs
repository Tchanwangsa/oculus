//! Headers and footers that repeat across pages.

use super::content::{kind_of, norm, page_idx, text_of};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// A header/footer on at least this fraction of a window's pages is template
/// furniture, not content.
const BOILERPLATE_PAGE_RATIO: f64 = 0.5;

/// Below this many pages the ratio means nothing.
const BOILERPLATE_MIN_PAGES: i64 = 4;

/// The normalised header/footer strings repeated across most of a window's
/// pages — counted by **distinct** page, not occurrence.
pub(super) fn find_boilerplate(items: &[&Value], window_pages: i64) -> HashSet<String> {
    if window_pages < BOILERPLATE_MIN_PAGES {
        return HashSet::new();
    }

    let mut pages_with: HashMap<String, HashSet<i64>> = HashMap::new();
    for item in items {
        let kind = kind_of(item);
        if kind != "header" && kind != "footer" {
            continue;
        }
        let key = norm(text_of(item));
        if !key.is_empty() {
            pages_with.entry(key).or_default().insert(page_idx(item));
        }
    }

    let threshold = window_pages as f64 * BOILERPLATE_PAGE_RATIO;
    pages_with
        .into_iter()
        .filter(|(_, pages)| pages.len() as f64 >= threshold)
        .map(|(key, _)| key)
        .collect()
}
