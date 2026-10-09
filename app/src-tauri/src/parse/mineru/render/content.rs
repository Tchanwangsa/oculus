//! Content-list accessors.
//!
//! Untyped JSON from a backend we do not control: every read has a default.

use serde_json::Value;
use std::path::Path;

/// Equations may also carry an `img_path`, but are rendered as LaTeX.
pub(super) const IMAGE_TYPES: [&str; 3] = ["image", "chart", "table"];

pub(super) fn kind_of(item: &Value) -> &str {
    item.get("type").and_then(Value::as_str).unwrap_or("")
}

pub(super) fn text_of(item: &Value) -> &str {
    item.get("text").and_then(Value::as_str).unwrap_or("")
}

pub(super) fn page_idx(item: &Value) -> i64 {
    item.get("page_idx").and_then(Value::as_i64).unwrap_or(0)
}

/// `img_path`, when non-empty: an empty one falls through to `table_body`.
pub(super) fn img_path(item: &Value) -> Option<&str> {
    item.get("img_path")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
}

/// Python-style truthiness: `text_level: 0` is level-less, `3` is a heading.
pub(super) fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Number(number)) => number.as_f64().is_none_or(|n| n != 0.0),
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(fields)) => !fields.is_empty(),
    }
}

/// The link is the basename, so the archive's layout never leaks.
pub(super) fn basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The caption/footnote list, space-joined and trimmed.
pub(super) fn join_parts(item: &Value, key: &str) -> String {
    let Some(parts) = item.get(key).and_then(Value::as_array) else {
        return String::new();
    };
    parts
        .iter()
        .map(|part| part.as_str().unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// The comparison form for boilerplate: whitespace-collapsed and lowercased.
pub(super) fn norm(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
