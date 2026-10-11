//! HTML → Markdown for Canvas bodies. Cruft nodes are skipped during the walk
//! rather than stripped up front.

mod blocks;
mod inline;
mod lists;
mod tables;
#[cfg(test)]
mod tests;
mod tree;

pub use blocks::collapse_blank_lines;

use std::collections::HashMap;

use scraper::Html;

use blocks::block_md;
use tree::{is_tag, Ctx};

const NBSP: char = '\u{a0}';

/// Never rendered, in any position.
const SKIP_TAGS: &[&str] = &["script", "style", "noscript", "svg", "path"];

/// `<img src>` → local relative path, filled in by the image downloader.
pub type ImageMap = HashMap<String, String>;

/// Convert a Canvas HTML body to Markdown. `images` may be empty.
pub fn to_markdown(html: &str, images: &ImageMap) -> String {
    let doc = Html::parse_document(html);
    let body = doc
        .tree
        .root()
        .descendants()
        .find(|n| is_tag(n, "body"))
        .unwrap_or_else(|| doc.tree.root());

    let ctx = Ctx {
        images,
        in_cell: false,
    };
    let out = block_md(body, &ctx);
    collapse_blank_lines(&out).trim().to_string()
}

/// Every `<img>` in a body, as (file API endpoint, src); the src keys the
/// rewrite map.
pub fn image_refs(html: &str) -> Vec<(String, String)> {
    let doc = Html::parse_document(html);
    doc.tree
        .root()
        .descendants()
        .filter(|n| is_tag(n, "img"))
        .filter_map(|n| {
            let el = n.value().as_element()?;
            let endpoint = match el.attr("data-api-endpoint") {
                Some(e) => e.to_string(),
                None => format!("/api/v1/files/{}", el.attr("data-id")?),
            };
            Some((endpoint, el.attr("src").unwrap_or_default().to_string()))
        })
        .collect()
}

/// Canvas page slugs and course file ids linked from a body — pages no module
/// lists are reachable only this way.
pub fn canvas_links(html: &str, course_id: i64) -> (Vec<String>, Vec<String>) {
    let doc = Html::parse_document(html);
    let prefix = format!("/courses/{course_id}/");
    let (mut pages, mut files) = (Vec::new(), Vec::new());

    for n in doc.tree.root().descendants() {
        if !is_tag(&n, "a") {
            continue;
        }
        let Some(href) = n.value().as_element().and_then(|e| e.attr("href")) else {
            continue;
        };

        if let Some(slug) = after_marker(href, "/pages/") {
            let slug = slug.split(['?', '#', '/']).next().unwrap_or("");
            if !slug.is_empty() && href.contains("/courses/") && !pages.iter().any(|p| p == slug) {
                pages.push(slug.to_string());
            }
        }
        if href.contains(&prefix) {
            if let Some(rest) = after_marker(href, "/files/") {
                let id: String = rest.chars().take_while(char::is_ascii_digit).collect();
                if !id.is_empty() && !files.iter().any(|f| f == &id) {
                    files.push(id);
                }
            }
        }
    }
    (pages, files)
}

fn after_marker<'a>(s: &'a str, marker: &str) -> Option<&'a str> {
    s.find(marker).map(|i| &s[i + marker.len()..])
}
