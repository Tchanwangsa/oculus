use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use ego_tree::{NodeId, NodeRef};
use scraper::node::{Element, Node};
use url::Url;

use super::maths::is_equation_image;
use super::{ImageMap, SKIP_TAGS};

pub(super) type Ref<'a> = NodeRef<'a, Node>;

/// State shared by one document's walk.
pub(super) struct Walk<'a> {
    pub(super) images: &'a ImageMap,
    /// The Canvas a relative iframe src is on; `None` keeps only absolute ones.
    pub(super) canvas: Option<&'a Url>,
    /// `element_toggler` targets by id: each renders at its toggler.
    pub(super) toggles: HashMap<&'a str, Ref<'a>>,
    /// Nodes a widget rendered somewhere else (a reveal's answer, an accordion
    /// panel, a toggled div), skipped where they sit.
    pub(super) consumed: RefCell<HashSet<NodeId>>,
}

#[derive(Clone, Copy)]
pub(super) struct Ctx<'a> {
    pub(super) walk: &'a Walk<'a>,
    /// Inside a table cell: images and block structure are dropped, because a
    /// GFM cell must stay on one line.
    pub(super) in_cell: bool,
}

impl Ctx<'_> {
    pub(super) fn consume(&self, n: &Ref<'_>) {
        self.walk.consumed.borrow_mut().insert(n.id());
    }
}

pub(super) fn is_tag(n: &Ref<'_>, name: &str) -> bool {
    n.value().as_element().is_some_and(|e| e.name() == name)
}

pub(super) fn tag<'a>(n: &Ref<'a>) -> Option<&'a str> {
    n.value().as_element().map(|e| e.name())
}

pub(super) fn attr<'a>(n: &Ref<'a>, name: &str) -> Option<&'a str> {
    n.value().as_element().and_then(|e| e.attr(name))
}

pub(super) fn has_class(n: &Ref<'_>, class: &str) -> bool {
    n.value()
        .as_element()
        .is_some_and(|e| e.classes().any(|c| c == class))
}

pub(super) fn is_heading(name: &str) -> bool {
    matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
}

/// Element children only — the equivalent of `.children`, not `.childNodes`.
pub(super) fn element_children<'a>(n: Ref<'a>) -> impl Iterator<Item = Ref<'a>> {
    n.children().filter(|c| c.value().is_element())
}

/// The next element sibling, across whitespace text; `None` past any other text.
pub(super) fn next_element<'a>(n: Ref<'a>) -> Option<Ref<'a>> {
    for s in n.next_siblings() {
        if s.value().is_element() {
            return Some(s);
        }
        if s.value().as_text().is_some_and(|t| !t.trim().is_empty()) {
            return None;
        }
    }
    None
}

pub(super) fn text_content(n: Ref<'_>) -> String {
    n.descendants()
        .filter_map(|d| d.value().as_text().map(|t| t.to_string()))
        .collect()
}

pub(super) fn squeeze(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A widget label: `n`'s text without dropped or consumed nodes, on one line.
pub(super) fn plain_text(n: Ref<'_>, ctx: &Ctx) -> String {
    fn walk(n: Ref<'_>, ctx: &Ctx, out: &mut String) {
        for c in n.children() {
            if let Some(t) = c.value().as_text() {
                out.push_str(t);
            } else if c.value().is_element() && !dropped(&c, ctx) {
                walk(c, ctx, out);
            }
        }
    }
    let mut out = String::new();
    walk(n, ctx, &mut out);
    squeeze(&out)
}

/// Decorative, accessibility and pasted cruft that must not reach the output,
/// and nodes a widget already rendered. In a table cell the net is wider: an
/// image or an icon there breaks the row.
pub(super) fn dropped(n: &Ref<'_>, ctx: &Ctx) -> bool {
    let Some(el) = n.value().as_element() else {
        return false;
    };
    let name = el.name();

    if SKIP_TAGS.contains(&name) || ctx.walk.consumed.borrow().contains(&n.id()) {
        return true;
    }
    let classes: Vec<&str> = el.classes().collect();
    if classes.iter().any(|c| {
        matches!(
            *c,
            "screenreader-only" | "external_link_icon" | "pdf-page-container" | "plv-text-layer"
        ) || *c == "textLayer"
            || c.contains("screenReaderContent")
    }) {
        return true;
    }
    if name == "button" && classes.contains(&"ally-accessible-versions") {
        return true;
    }
    // Canvas's own view heading, pasted into an announcement above itself.
    if el.attr("data-testid") == Some("message_title") {
        return true;
    }
    name == "img" && dropped_image(el, &classes, ctx.in_cell)
}

fn dropped_image(el: &Element, classes: &[&str], in_cell: bool) -> bool {
    if is_equation_image(el) {
        return false;
    }
    in_cell
        || el.attr("role") == Some("presentation")
        || classes.contains(&"ally-accessibility-score-indicator-image")
        || el.attr("src").is_none_or(|s| s.trim().is_empty())
}
