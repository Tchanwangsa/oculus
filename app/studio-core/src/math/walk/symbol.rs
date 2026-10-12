//! A KaTeX symbol: one or more characters in one font, each its own glyph
//! piece, advancing by the font's widths. A positive italic correction is
//! the `margin-right` KaTeX writes on the symbol, unless its style sets one
//! of its own (written later, so it wins).

use super::super::fonts;
use super::super::{GlyphShape, Shape};
use super::{Ctx, Laid, Piece, classes, css};
use katex::dom_tree::SymbolNode;
use katex::types::CssProperty;
use std::sync::Arc;
use vello_cpu::kurbo::{Affine, Rect};

pub fn layout<'a>(s: &'a SymbolNode, cx: Ctx<'a>) -> Result<Laid<'a>, String> {
    let mut font = classes::font(cx.font, &s.classes);
    if let Some(family) = cx.family {
        font.family = family;
    }
    let colour = s.style.get(CssProperty::Color).or(cx.colour);
    let size = cx.size;
    let mut out = Laid::empty(0.0);
    let mut x = css::or_zero(&s.style, CssProperty::MarginLeft, size);
    let single = s.text.chars().count() == 1;
    for ch in s.text.chars() {
        let Some(g) = fonts::glyph(font, ch) else {
            // A space a face lacks (a no-break space in `\text`) takes
            // KaTeX's metric width, as the browser's fallback font would
            // roughly; anything else is an error.
            if ch.is_whitespace() {
                x += if single { s.width } else { 0.25 } * size;
                continue;
            }
            return Err(format!(
                "no bundled KaTeX font has a glyph for {ch:?} (U+{:04X})",
                u32::from(ch)
            ));
        };
        if let Some(ink) = g.ink() {
            // The glyph's box: its advance by KaTeX's height and depth for
            // the symbol, or its ink where either is empty.
            let (x0, x1) = if g.advance > 0.0 {
                (0.0, g.advance)
            } else {
                (ink.x0, ink.x1)
            };
            let (y0, y1) = if s.height + s.depth > 0.0 {
                (-s.height, s.depth)
            } else {
                (ink.y0, ink.y1)
            };
            let view = Rect::new(x0, y0, x1, y1);
            out.pieces.push(Piece {
                label: ch.to_string(),
                groups: cx.groups,
                script: cx.script,
                rect: Affine::new([size, 0.0, 0.0, size, x, 0.0]).transform_rect_bbox(view),
                shape: Shape::Glyph(GlyphShape {
                    outline: Arc::new(g.outline),
                    view,
                }),
                colour,
            });
        }
        x += g.advance * size;
    }
    let italic = if s.italic > 0.0 { s.italic * size } else { 0.0 };
    x += css::get(&s.style, CssProperty::MarginRight, size, None).unwrap_or(italic);
    out.width = x;
    Ok(out)
}
