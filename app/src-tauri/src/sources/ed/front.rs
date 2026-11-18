//! Front matter on scraped Ed markdown. Every value is a JSON literal, which
//! is also valid YAML, and a non-empty list is one JSON value per item; the
//! encoding is described in `docs/sync.md`.

use serde_json::Value;

/// `---`-fenced front matter for `fields`, in order. Missing data goes in as
/// `null`, never as an omitted key.
pub fn front_matter(fields: &[(&str, Value)]) -> String {
    let mut out = String::from("---\n");
    for (key, value) in fields {
        match value {
            Value::Array(items) if !items.is_empty() => {
                out.push_str(&format!("{key}:\n"));
                for item in items {
                    out.push_str(&format!("  - {}\n", literal(item)));
                }
            }
            other => out.push_str(&format!("{key}: {}\n", literal(other))),
        }
    }
    out.push_str("---\n");
    out
}

/// A string field, `null` when absent or empty.
pub fn text_or_null(value: &Value) -> Value {
    match value.as_str() {
        Some(s) if !s.is_empty() => Value::from(s),
        _ => Value::Null,
    }
}

fn literal(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".into())
}
