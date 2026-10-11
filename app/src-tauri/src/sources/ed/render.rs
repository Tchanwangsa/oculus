//! Rendering a thread's replies and metadata to Markdown.

use super::document::document_md;

use std::collections::HashMap;

pub(super) fn author_name(item: &serde_json::Value, users: &HashMap<i64, String>) -> String {
    if item["is_anonymous"].as_bool().unwrap_or(false) {
        return "Anonymous".to_string();
    }
    item["user"]["name"]
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            item["user_id"]
                .as_i64()
                .and_then(|id| users.get(&id).cloned())
        })
        .unwrap_or_else(|| "Anonymous".to_string())
}

/// A reply and its children, each level one blockquote deeper.
pub(super) fn render_reply(
    item: &serde_json::Value,
    users: &HashMap<i64, String>,
    is_answer: bool,
    depth: usize,
    out: &mut String,
) {
    let mut badge = String::new();
    if is_answer {
        badge.push_str(" (answer)");
    }
    if item["is_endorsed"].as_bool().unwrap_or(false) {
        badge.push_str(" (endorsed)");
    }
    let head = format!(
        "**{}**{badge} · {}",
        author_name(item, users),
        fmt_ts(item["created_at"].as_str().unwrap_or(""))
    );
    let body = content_md(item);

    let quote = "> ".repeat(depth);
    out.push('\n');
    for line in std::iter::once(head.as_str())
        .chain(std::iter::once(""))
        .chain(body.lines())
    {
        out.push_str(&quote);
        out.push_str(line);
        out.push('\n');
    }

    for child in item["comments"].as_array().into_iter().flatten() {
        render_reply(child, users, false, depth + 1, out);
    }
}

/// The `<document>` XML when present (it keeps images and links), else the
/// plain-text `document` field.
pub(super) fn content_md(item: &serde_json::Value) -> String {
    let xml = item["content"].as_str().unwrap_or("");
    if !xml.is_empty() {
        let md = document_md(xml);
        if !md.trim().is_empty() {
            return md;
        }
    }
    item["document"].as_str().unwrap_or("").trim().to_string()
}

/// `"2026-08-07T15:42:01.522942+10:00"` → `"2026-08-07 15:42"` (already local).
pub(super) fn fmt_ts(iso: &str) -> String {
    if iso.len() >= 16 && iso.as_bytes()[10] == b'T' {
        format!("{} {}", &iso[..10], &iso[11..16])
    } else {
        iso.to_string()
    }
}
