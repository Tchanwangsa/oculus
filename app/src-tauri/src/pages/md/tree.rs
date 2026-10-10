use ego_tree::NodeRef;
use scraper::node::Node;

use super::{ImageMap, SKIP_TAGS};

pub(super) type Ref<'a> = NodeRef<'a, Node>;

pub(super) struct Ctx<'a> {
    pub(super) images: &'a ImageMap,
    /// Inside a table cell: images and block structure are dropped, because a
    /// GFM cell must stay on one line.
    pub(super) in_cell: bool,
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

pub(super) fn is_heading(name: &str) -> bool {
    matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
}

/// Element children only — the equivalent of `.children`, not `.childNodes`.
pub(super) fn element_children<'a>(n: Ref<'a>) -> impl Iterator<Item = Ref<'a>> {
    n.children().filter(|c| c.value().is_element())
}

pub(super) fn text_content(n: Ref<'_>) -> String {
    n.descendants()
        .filter_map(|d| d.value().as_text().map(|t| t.to_string()))
        .collect()
}

/// Decorative and accessibility cruft that must not reach the output. In a
/// table cell the net is wider: an image or an icon there breaks the row.
pub(super) fn dropped(n: &Ref<'_>, in_cell: bool) -> bool {
    let Some(el) = n.value().as_element() else {
        return false;
    };
    let name = el.name();

    if SKIP_TAGS.contains(&name) {
        return true;
    }
    let classes: Vec<&str> = el.classes().collect();
    if classes.iter().any(|c| {
        *c == "screenreader-only" || *c == "external_link_icon" || c.contains("screenReaderContent")
    }) {
        return true;
    }
    if name == "button" && classes.contains(&"ally-accessible-versions") {
        return true;
    }
    if name == "img" && (el.attr("role") == Some("presentation") || in_cell) {
        return true;
    }
    false
}
