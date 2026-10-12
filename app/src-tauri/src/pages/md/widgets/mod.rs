//! Page widgets whose meaning is in their classes: the University's `uom-ui-*`
//! kit, Bootstrap alerts, Canvas `element_toggler` and code-block themes. In a
//! table cell none of these apply; the cell flattens them to one line of text.

mod callouts;
mod collapsible;

pub(super) use collapsible::{toggle_targets, toggler_md};

use super::blocks::{block_md, code_text, fence};
use super::inline::inline_md;
use super::tree::{dropped, element_children, has_class, is_tag, next_element, squeeze, Ref};
use super::Ctx;

/// A widget met at block level, as markdown ending in a blank line, or `None`
/// for an ordinary element.
pub(super) fn widget_md(n: Ref<'_>, ctx: &Ctx) -> Option<String> {
    if ctx.in_cell {
        return None;
    }
    collapsible::tabs_md(n, ctx)
        .or_else(|| collapsible::accordion_md(n, ctx))
        .or_else(|| collapsible::reveal_md(n, ctx))
        .or_else(|| checklist_md(n, ctx))
        .or_else(|| tiles_md(n, ctx))
        .or_else(|| code_md(n))
        .or_else(|| callouts::callout_md(n, ctx))
}

/// A run of `p.uom-ui-checkbox` siblings as one tight GFM task list.
fn checklist_md(n: Ref<'_>, ctx: &Ctx) -> Option<String> {
    if !has_class(&n, "uom-ui-checkbox") {
        return None;
    }
    let mut out = String::new();
    let mut item = Some(n);
    while let Some(it) = item {
        let text = squeeze(&inline_md(it, ctx));
        if !text.is_empty() {
            out.push_str(&format!("- [ ] {text}\n"));
        }
        item = next_element(it).filter(|s| has_class(s, "uom-ui-checkbox") && !dropped(s, ctx));
        if let Some(s) = &item {
            ctx.consume(s);
        }
    }
    out.push('\n');
    Some(out)
}

/// `table.uom-ui-tiles` lays out cards, so each cell's content becomes
/// ordinary blocks rather than a markdown table.
fn tiles_md(n: Ref<'_>, ctx: &Ctx) -> Option<String> {
    if !is_tag(&n, "table") || !has_class(&n, "uom-ui-tiles") {
        return None;
    }
    let own_row = |r: &Ref<'_>| {
        is_tag(r, "tr")
            && r.ancestors().find(|a| is_tag(a, "table")).map(|t| t.id()) == Some(n.id())
    };
    let mut out = String::new();
    for row in n.descendants().filter(own_row) {
        for cell in element_children(row).filter(|c| is_tag(c, "td") || is_tag(c, "th")) {
            out.push_str(&block_md(cell, ctx));
        }
    }
    Some(out)
}

/// An `amber-pre` code block holding no `<pre>` (its lines are styled spans):
/// a fence of its text. One with a `<pre>` inside is left to reach the `<pre>`.
fn code_md(n: Ref<'_>) -> Option<String> {
    if !has_class(&n, "amber-pre") || n.descendants().any(|d| is_tag(&d, "pre")) {
        return None;
    }
    Some(fence(&code_text(n)))
}
