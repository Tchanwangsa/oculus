//! Text paint: one glyph run per parley run. With `reveal`, settled units
//! still batch into that run and only units in flight draw on their own.
//! Glyph positions are parley's either way, so a unit lands exactly where it
//! sits in the settled text.

mod reveal;
mod units;

use super::Canvas;
use crate::layout::TextLayout;
use crate::scene::{Color, Reveal};
use parley::PositionedLayoutItem;
use vello_cpu::Glyph;
use vello_cpu::kurbo::Affine;

/// Draws a laid-out text block; `t` maps layout units to the canvas.
pub fn draw(
    c: &mut Canvas,
    t: Affine,
    layout: &TextLayout,
    text: &str,
    color: Color,
    reveal: Option<&Reveal>,
) -> Result<(), String> {
    match reveal {
        Some(r) if r.progress <= 0.0 => Ok(()),
        Some(r) if r.progress < 1.0 => reveal::draw(c, t, layout, text, color, r),
        _ => draw_all(c, t, layout, color),
    }
}

fn draw_all(c: &mut Canvas, t: Affine, layout: &TextLayout, color: Color) -> Result<(), String> {
    for line in layout.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(run) = item else {
                continue;
            };
            let glyphs = run.positioned_glyphs().map(|g| Glyph {
                id: g.id,
                x: g.x,
                y: g.y,
            });
            c.glyphs(t, run.run(), glyphs, color)?;
        }
    }
    Ok(())
}
