//! One page's items rendered to markdown, with a block per located item.

use super::content::{kind_of, page_box};
use super::item::render_item;
use crate::parse::ParseBlock;
use serde_json::Value;
use std::collections::HashSet;

/// One page's sorted items joined by a blank line, plus a block per rendered
/// item that has a usable box. `None` when nothing rendered.
pub(super) fn render_page(
    items: &[&Value],
    images_rel: &str,
    dropped: &HashSet<String>,
    boilerplate: &HashSet<String>,
) -> Option<(String, Vec<ParseBlock>)> {
    const SEPARATOR: &str = "\n\n";
    let mut markdown = String::new();
    let mut blocks = Vec::new();
    // `markdown`'s length in UTF-16 code units, the unit the offsets use.
    let mut units = 0u32;
    let mut rendered_any = false;
    for item in items {
        let Some(text) = render_item(item, images_rel, dropped, boilerplate) else {
            continue;
        };
        if rendered_any {
            markdown.push_str(SEPARATOR);
            units += SEPARATOR.len() as u32;
        }
        rendered_any = true;
        let start = units;
        units += text.encode_utf16().count() as u32;
        markdown.push_str(&text);
        if let Some(bbox) = page_box(item) {
            blocks.push(ParseBlock {
                kind: kind_of(item).to_string(),
                bbox,
                start,
                end: units,
            });
        }
    }
    rendered_any.then_some((markdown, blocks))
}
