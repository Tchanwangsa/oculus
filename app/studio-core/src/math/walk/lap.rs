//! `\mathllap`, `\mathrlap`, `\mathclap`: a zero-width span whose
//! `katex-inner` is absolutely placed, its right edge on the span's
//! (`llap`), its left edge (`rlap`) or its centre (`clap`, whose inner span
//! has `margin-left: -50%`).

use super::{Ctx, Laid, span};
use katex::dom_tree::{DomSpan, HtmlDomNode};

pub fn layout<'a>(s: &'a DomSpan, cx: Ctx<'a>) -> Result<Laid<'a>, String> {
    let mut out = Laid::empty(0.0);
    let inner = s.children.iter().find_map(|c| match c {
        HtmlDomNode::DomSpan(i) if i.classes.contains("katex-inner") => Some(i),
        _ => None,
    });
    let Some(inner) = inner else {
        return Ok(out);
    };
    let laid = span(inner, cx)?;
    let dx = if s.classes.contains("llap") {
        -laid.width
    } else if s.classes.contains("clap") {
        -laid.width / 2.0
    } else {
        0.0
    };
    out.add(laid, dx, 0.0);
    Ok(out)
}
