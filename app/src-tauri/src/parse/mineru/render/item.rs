//! Rendering one content-list item to a markdown block.

use super::content::{basename, img_path, join_parts, kind_of, norm, text_of, truthy, IMAGE_TYPES};
use serde_json::Value;
use std::collections::HashSet;

/// Convert one content-list entry to a markdown block, or drop it.
///
/// The branch order is load-bearing; two interactions look like bugs and are
/// preserved, because changing either rewrites the library's markdown:
///
/// * a `table` carrying **both** `img_path` and `table_body` loses the HTML
///   body — the image wins, and the caption survives only as alt text;
/// * a `footer` with a truthy `text_level` is caught by the heading branch
///   before the footer branch and becomes a `##` heading rather than being
///   dropped.
pub(super) fn render_item(
    item: &Value,
    images_rel: &str,
    dropped: &HashSet<String>,
    boilerplate: &HashSet<String>,
) -> Option<String> {
    let kind = kind_of(item);

    // MinerU's equation text carries its own `$$` delimiters.
    if kind == "equation" {
        let text = text_of(item).trim();
        return (!text.is_empty()).then(|| text.to_string());
    }

    if IMAGE_TYPES.contains(&kind) {
        let caption = join_parts(item, &format!("{kind}_caption"));
        let footnote = join_parts(item, &format!("{kind}_footnote"));

        let block = match img_path(item) {
            // Dropped by the size filter, caption with it.
            Some(path) if dropped.contains(&basename(path)) => String::new(),
            Some(path) => format!("![{caption}]({images_rel}/{})", basename(path)),
            None => item
                .get("table_body")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string(),
        };

        let joined = [block, footnote]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        return (!joined.is_empty()).then_some(joined);
    }

    let text = text_of(item).trim();
    if text.is_empty() || boilerplate.contains(&norm(text)) {
        return None;
    }

    // One fixed level: MinerU's `text_level` numbers are per-page guesses.
    if kind == "header" || truthy(item.get("text_level")) {
        return Some(format!("## {text}"));
    }
    if kind == "footer" {
        return None;
    }
    Some(text.to_string())
}
