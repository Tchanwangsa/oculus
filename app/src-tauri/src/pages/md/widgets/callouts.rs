//! Coloured boxes as blockquotes: `uom-ui-notice--*`, Bootstrap `alert-*` and
//! `uom-ui-emphasise` on a block, labelled when the class names a kind, and a
//! `div` styled with both a border and a background around block content.
//! Inline styles are otherwise paste noise, so nothing else reads them.

use scraper::node::Element;

use super::super::blocks::{block_md, quote};
use super::super::inline::inline_md;
use super::super::tree::{attr, element_children, is_heading, is_tag, tag, Ref};
use super::super::Ctx;

const BOXES: &[&str] = &["div", "p", "section", "aside", "article"];
const BLOCKS: &[&str] = &[
    "p",
    "div",
    "ul",
    "ol",
    "table",
    "pre",
    "blockquote",
    "details",
    "section",
];

pub(super) fn callout_md(n: Ref<'_>, ctx: &Ctx) -> Option<String> {
    let el = n.value().as_element()?;
    let label = match class_callout(el) {
        Some(label) => label,
        None if styled_box(n) => None,
        None => return None,
    };
    let body = if el.name() == "p" {
        inline_md(n, ctx)
    } else {
        block_md(n, ctx)
    };
    let body = body.trim();
    let body = match label {
        Some(l) if body.is_empty() => format!("**{l}**"),
        Some(l) => format!("**{l}**\n\n{body}"),
        None => body.to_string(),
    };
    Some(if body.is_empty() {
        String::new()
    } else {
        quote(&body)
    })
}

/// `Some(label)` when the classes make `el` a callout; the label is `None`
/// when they name no kind.
fn class_callout(el: &Element) -> Option<Option<&'static str>> {
    if !BOXES.contains(&el.name()) {
        return None;
    }
    let classes: Vec<&str> = el.classes().collect();
    if classes.iter().any(|c| c.starts_with("uom-ui-notice")) {
        let kind = classes
            .iter()
            .find_map(|c| c.strip_prefix("uom-ui-notice--"))
            .unwrap_or_default();
        return Some(kind_label(kind));
    }
    if classes.contains(&"alert") {
        let kind = classes
            .iter()
            .filter_map(|c| c.strip_prefix("alert-"))
            .find(|k| !matches!(*k, "dismissible" | "dismissable" | "heading" | "link"))
            .unwrap_or_default();
        return Some(kind_label(kind));
    }
    classes.contains(&"uom-ui-emphasise").then_some(None)
}

fn kind_label(kind: &str) -> Option<&'static str> {
    match kind {
        "info" | "primary" => Some("Note"),
        "warning" | "danger" => Some("Warning"),
        "success" => Some("Tip"),
        _ => None,
    }
}

/// A `div` whose style draws a real border and a background, holding block
/// content.
fn styled_box(n: Ref<'_>) -> bool {
    if !is_tag(&n, "div") {
        return false;
    }
    let Some(style) = attr(&n, "style") else {
        return false;
    };
    let (mut border, mut background) = (false, false);
    for decl in style.split(';') {
        let Some((prop, val)) = decl.split_once(':') else {
            continue;
        };
        let (prop, val) = (
            prop.trim().to_ascii_lowercase(),
            val.trim().to_ascii_lowercase(),
        );
        match prop.as_str() {
            "border" | "border-top" | "border-right" | "border-bottom" | "border-left"
            | "border-width" => border |= real_border(&val),
            "background" | "background-color" => {
                background |= !matches!(
                    val.as_str(),
                    "" | "none" | "transparent" | "inherit" | "initial" | "unset"
                )
            }
            _ => {}
        }
    }
    border
        && background
        && element_children(n)
            .any(|c| tag(&c).is_some_and(|t| BLOCKS.contains(&t) || is_heading(t)))
}

/// A border value with a non-zero width that isn't `none` or `hidden`.
fn real_border(val: &str) -> bool {
    let tokens: Vec<&str> = val.split_whitespace().collect();
    if tokens.iter().any(|t| matches!(*t, "none" | "hidden")) {
        return false;
    }
    tokens.iter().any(|t| {
        let num: String = t
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        matches!(*t, "thin" | "medium" | "thick") || num.parse::<f64>().is_ok_and(|v| v > 0.0)
    })
}
