//! A vlist (`vlist-t > vlist-r > vlist > row…`): KaTeX's vertical stack.
//! Each row is `<span style="top:T"><span class="pstrut" style="height:H">
//! </span>elem</span>`, which puts the element's baseline `T + H` below the
//! vlist's baseline; that is what KaTeX's `makeVList` means by it, so the
//! table CSS around it is not modelled. The vlist is as wide as its widest
//! row (with the row's block margins); each row's line sits in that width
//! by the inherited `text-align`, and a `width: 100%` element (a fraction
//! bar, a root sign's tail) takes the whole width, so rows holding one are
//! laid out again once the width is known. A second `vlist-r` only carries
//! the depth and is skipped. In a script holder's vlist each row but the
//! base is a script, up or down (`Holder`).

use super::{Ctx, Holder, Laid, css, inside};
use katex::dom_tree::{DomSpan, HtmlDomNode};
use katex::types::CssProperty;

struct Row<'a> {
    span: &'a DomSpan,
    cx: Ctx<'a>,
    baseline: f64,
    margins: (f64, f64),
    laid: Laid<'a>,
}

fn spans(children: &[HtmlDomNode]) -> impl Iterator<Item = &DomSpan> {
    children.iter().filter_map(|c| match c {
        HtmlDomNode::DomSpan(s) => Some(s),
        _ => None,
    })
}

/// The row's element(s): everything but the pstrut.
fn elems(row: &DomSpan) -> impl Iterator<Item = &HtmlDomNode> {
    row.children
        .iter()
        .filter(|c| !matches!(c, HtmlDomNode::DomSpan(s) if s.classes.contains("pstrut")))
}

pub fn layout<'a>(table: &'a DomSpan, cx: Ctx<'a>) -> Result<Laid<'a>, String> {
    let Some(cell) = spans(&table.children)
        .find(|r| r.classes.contains("vlist-r"))
        .and_then(|r| spans(&r.children).find(|c| c.classes.contains("vlist")))
    else {
        return Ok(Laid::empty(0.0));
    };
    // KaTeX's order is bottom to top: rows before the operator's are below it.
    let base = (cx.holder == Holder::Limits)
        .then(|| spans(&cell.children).position(is_base))
        .flatten();
    let mut rows = Vec::new();
    for (i, span) in spans(&cell.children).enumerate() {
        let mut rcx = inside(span, cx);
        rcx.avail = None;
        let size = rcx.size;
        let pstrut = spans(&span.children)
            .find(|s| s.classes.contains("pstrut"))
            .map_or(0.0, |p| css::or_zero(&p.style, CssProperty::Height, size));
        let top = css::or_zero(&span.style, CssProperty::Top, size);
        let margins = (
            css::or_zero(&span.style, CssProperty::MarginLeft, size),
            css::or_zero(&span.style, CssProperty::MarginRight, size),
        );
        rcx.script = match (cx.holder, base) {
            (Holder::SupSub, _) => rcx.script.push(top + pstrut < 0.0),
            (Holder::Limits, Some(b)) if i != b => rcx.script.push(i > b),
            _ => rcx.script,
        };
        let laid = line(span, rcx)?;
        rows.push(Row {
            span,
            cx: rcx,
            baseline: top + pstrut,
            margins,
            laid,
        });
    }
    let width = rows
        .iter()
        .map(|r| r.margins.0 + r.laid.width + r.margins.1)
        .fold(0.0, f64::max);
    for r in rows.iter_mut().filter(|r| r.laid.fills) {
        r.cx.avail = Some((width - r.margins.0 - r.margins.1).max(0.0));
        r.laid = line(r.span, r.cx)?;
    }
    let mut out = Laid::empty(width);
    for r in rows.into_iter().rev() {
        let room = width - r.margins.0 - r.margins.1;
        let x = r.margins.0 + r.cx.align * (room - r.laid.width);
        out.add(r.laid, x, r.baseline);
    }
    out.fills = false;
    Ok(out)
}

/// The operator's row under limits: KaTeX wraps the base in a span with no
/// class, and each limit is a styled group.
fn is_base(row: &DomSpan) -> bool {
    elems(row).any(|e| matches!(e, HtmlDomNode::DomSpan(s) if s.classes.iter().next().is_none()))
}

fn line<'a>(row: &'a DomSpan, cx: Ctx<'a>) -> Result<Laid<'a>, String> {
    let mut out = Laid::empty(0.0);
    for e in elems(row) {
        let laid = super::node(e, cx)?;
        let w = laid.width;
        out.add(laid, out.width, 0.0);
        out.width += w;
    }
    Ok(out)
}
