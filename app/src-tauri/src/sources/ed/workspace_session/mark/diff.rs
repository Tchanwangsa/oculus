//! A `check_diff` check's word diff, as the pane draws it. Ed sends each
//! line as `[[sign], [sign | "=", text]…]`: a one-element line sign (`-`
//! observed, `+` expected), then word runs. Every element becomes a
//! `[sign, text]` pair, so the line sign is the first pair, with empty text,
//! and a bare `["mn"]` (missing newline) keeps its sign with no text.

use serde_json::Value;

/// One word run: its sign (`-`, `+`, `=`, `mn`) and its text.
pub type Run = (String, String);

/// The lines of one `diff`, each a list of runs. Elements of a shape Ed is
/// not known to send are left out.
pub fn lines(diff: &Value) -> Vec<Vec<Run>> {
    diff.as_array()
        .into_iter()
        .flatten()
        .filter_map(|line| match line {
            Value::Array(elements) => Some(elements.iter().filter_map(run).collect()),
            Value::String(text) => Some(vec![("=".to_string(), text.clone())]),
            _ => None,
        })
        .collect()
}

fn run(element: &Value) -> Option<Run> {
    match element {
        Value::Array(parts) => {
            let sign = parts.first()?.as_str()?.to_string();
            let text: String = parts[1..].iter().filter_map(Value::as_str).collect();
            Some((sign, text))
        }
        Value::String(text) => Some(("=".to_string(), text.clone())),
        _ => None,
    }
}
