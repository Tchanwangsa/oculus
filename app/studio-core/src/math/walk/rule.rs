//! Rules: lines that are a span's bottom border (`frac-line`,
//! `overline-line`, `underline-line`, `katex-hline`, `katex-hdashline`;
//! `width: 100%` of their vlist, the border's bottom on the row's
//! baseline) and `\rule` (`katex-rule`: a right and a top border, raised
//! by `bottom`). A dashed hline is drawn solid.

use super::super::Shape;
use super::{Ctx, Laid, Piece, css};
use katex::ClassList;
use katex::dom_tree::DomSpan;
use katex::types::CssProperty;
use vello_cpu::kurbo::Rect;

const LINES: [(&str, &str); 5] = [
    ("frac-line", "frac"),
    ("overline-line", "overline"),
    ("underline-line", "underline"),
    ("katex-hline", "hline"),
    ("katex-hdashline", "hline"),
];

/// The piece label of a border line's class, if it is one.
pub fn line_label(classes: &ClassList) -> Option<&'static str> {
    LINES
        .iter()
        .find(|(class, _)| classes.contains(class))
        .map(|(_, label)| *label)
}

pub fn line<'a>(s: &'a DomSpan, cx: Ctx<'a>, label: &str) -> Laid<'a> {
    let width = cx.avail.unwrap_or(0.0);
    let thickness = css::or_zero(&s.style, CssProperty::BorderBottomWidth, cx.size);
    let mut out = Laid::empty(width);
    out.fills = true;
    if width > 0.0 && thickness > 0.0 {
        out.pieces
            .push(piece(label, Rect::new(0.0, -thickness, width, 0.0), cx));
    }
    out
}

pub fn rule<'a>(s: &'a DomSpan, cx: Ctx<'a>) -> Laid<'a> {
    let st = &s.style;
    let w = css::or_zero(st, CssProperty::BorderRightWidth, cx.size);
    let h = css::or_zero(st, CssProperty::BorderTopWidth, cx.size);
    let raise = css::or_zero(st, CssProperty::Bottom, cx.size);
    let mut out = Laid::empty(w);
    if w > 0.0 && h > 0.0 {
        out.pieces
            .push(piece("rule", Rect::new(0.0, -raise - h, w, -raise), cx));
    }
    out
}

pub fn piece<'a>(label: &str, rect: Rect, cx: Ctx<'a>) -> Piece<'a> {
    Piece {
        label: label.to_owned(),
        groups: cx.groups,
        rect,
        shape: Shape::Rule,
        colour: cx.colour,
    }
}
