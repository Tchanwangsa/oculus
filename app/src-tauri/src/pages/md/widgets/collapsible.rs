//! Show/hide widgets as collapsed `<details>`, as the page shows them: UoM
//! accordions, reveal buttons and tabs, and Canvas `element_toggler`. Each
//! pairs a label with a body elsewhere in the tree; the body is consumed so
//! the walk doesn't render it again where it sits.

use std::collections::{HashMap, HashSet};

use super::super::blocks::block_md;
use super::super::embeds::details_block;
use super::super::escape::{escape_loose, escape_md};
use super::super::inline::inline_md;
use super::super::tree::{attr, dropped, has_class, next_element, plain_text, Ref};
use super::super::Ctx;

/// Accordion class prefixes; each has `-trigger`, `-panel` and `-title`.
const ACCORDIONS: &[&str] = &["uom-ui-accordion", "uom-mspace-accordion"];

/// A trigger and the panel inside it or right after it.
pub(super) fn accordion_md(n: Ref<'_>, ctx: &Ctx) -> Option<String> {
    let kit = ACCORDIONS
        .iter()
        .find(|k| has_class(&n, &format!("{k}-trigger")))?;
    let panel_class = format!("{kit}-panel");
    let panel = n
        .descendants()
        .find(|d| has_class(d, &panel_class))
        .or_else(|| next_element(n).filter(|s| has_class(s, &panel_class)))
        .filter(|p| !dropped(p, ctx))?;
    ctx.consume(&panel);
    let title_class = format!("{kit}-title");
    let label = match n.descendants().find(|d| has_class(d, &title_class)) {
        Some(t) => plain_text(t, ctx),
        None => plain_text(n, ctx),
    };
    Some(details_block(Some(&label), &block_md(panel, ctx)))
}

/// A "Reveal answer" button and the `uom-ui-hidden-content` right after it.
pub(super) fn reveal_md(n: Ref<'_>, ctx: &Ctx) -> Option<String> {
    if !has_class(&n, "uom-ui-reveal") {
        return None;
    }
    let hidden =
        next_element(n).filter(|s| has_class(s, "uom-ui-hidden-content") && !dropped(s, ctx))?;
    ctx.consume(&hidden);
    let label = plain_text(n, ctx);
    let label = if label.is_empty() {
        "Reveal answer".to_string()
    } else {
        label
    };
    Some(details_block(Some(&label), &block_md(hidden, ctx)))
}

fn is_panel(n: &Ref<'_>) -> bool {
    has_class(n, "uom-ui-tab-panel") || attr(n, "role") == Some("tabpanel")
}

/// A `uom-ui-tablist`'s triggers paired with the tab panels beside it: by
/// `aria-controls` → `id` when that matches, else in order. One `<details>`
/// per tab.
pub(super) fn tabs_md(list: Ref<'_>, ctx: &Ctx) -> Option<String> {
    if !has_class(&list, "uom-ui-tablist") {
        return None;
    }
    let mut triggers: Vec<Ref<'_>> = list
        .descendants()
        .filter(|d| has_class(d, "uom-ui-trigger"))
        .collect();
    if triggers.is_empty() {
        triggers = list
            .descendants()
            .filter(|d| attr(d, "role") == Some("tab"))
            .collect();
    }
    if triggers.is_empty() {
        return None;
    }
    // The outermost panels under the list's parent, outside the list itself.
    let parent = list.parent()?;
    let mut panels: Vec<Ref<'_>> = parent
        .descendants()
        .filter(|d| {
            is_panel(d)
                && !dropped(d, ctx)
                && !d
                    .ancestors()
                    .take_while(|a| a.id() != parent.id())
                    .any(|a| a.id() == list.id() || is_panel(&a))
        })
        .collect();

    let mut out = String::new();
    for t in triggers {
        let label = plain_text(t, ctx);
        let by_id = attr(&t, "aria-controls")
            .and_then(|id| panels.iter().position(|p| attr(p, "id") == Some(id)));
        match by_id.or((!panels.is_empty()).then_some(0)) {
            Some(i) => {
                let panel = panels.remove(i);
                ctx.consume(&panel);
                out.push_str(&details_block(Some(&label), &block_md(panel, ctx)));
            }
            None if !label.is_empty() => {
                out.push_str(&escape_loose(&label));
                out.push_str("\n\n");
            }
            None => {}
        }
    }
    Some(out)
}

fn controls<'a>(n: &Ref<'a>) -> Option<&'a str> {
    attr(n, "aria-controls")?.split_whitespace().next()
}

/// The elements `element_toggler`s show, by id. A target holding a toggler
/// itself is left out, so a toggle can never render its own ancestor.
pub(in crate::pages::md) fn toggle_targets(root: Ref<'_>) -> HashMap<&str, Ref<'_>> {
    let wanted: HashSet<&str> = root
        .descendants()
        .filter(|d| has_class(d, "element_toggler"))
        .filter_map(|d| controls(&d))
        .collect();
    let mut out = HashMap::new();
    if wanted.is_empty() {
        return out;
    }
    for d in root.descendants() {
        let Some(id) = attr(&d, "id").filter(|id| wanted.contains(id)) else {
            continue;
        };
        if !d.descendants().any(|x| has_class(&x, "element_toggler")) {
            out.entry(id).or_insert(d);
        }
    }
    out
}

/// A Canvas `element_toggler` with its target's content as the body. In a
/// table cell, the label then the target's text.
pub(in crate::pages::md) fn toggler_md(n: Ref<'_>, ctx: &Ctx) -> Option<String> {
    if !has_class(&n, "element_toggler") {
        return None;
    }
    let target = *ctx.walk.toggles.get(controls(&n)?)?;
    let label = plain_text(n, ctx);
    if ctx.in_cell {
        return Some(format!("{} {}", escape_md(&label), inline_md(target, ctx)));
    }
    Some(format!(
        "\n\n{}",
        details_block(Some(&label), &block_md(target, ctx))
    ))
}
