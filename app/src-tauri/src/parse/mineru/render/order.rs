//! The order items are rendered in.

use super::content::kind_of;
use serde_json::Value;
use std::cmp::Ordering;

/// Put headers first while preserving MinerU's body reading order. Every
/// non-header shares one key, so the sort must be stable (`sort_by`, never
/// `sort_unstable_by`).
pub(super) fn sort_key(item: &Value) -> (u8, f64, f64) {
    if kind_of(item) != "header" {
        return (1, 0.0, 0.0);
    }
    let bbox = item
        .get("bbox")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty());
    let at = |index: usize| {
        bbox.and_then(|values| values.get(index))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    };
    (0, at(1), at(0))
}

pub(super) fn compare(left: &Value, right: &Value) -> Ordering {
    let (a, b) = (sort_key(left), sort_key(right));
    a.0.cmp(&b.0)
        // A NaN coordinate compares equal rather than panicking.
        .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
        .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(Ordering::Equal))
}
