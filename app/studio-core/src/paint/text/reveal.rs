//! `reveal`: N units (words, characters or lines, in text order) appear one
//! after another. Unit i starts at i·d and lasts u, with (N-1)·d + u = 1 and
//! d = stagger·u; its progress is eased with easeOutCubic. `rise` lifts a
//! unit from 0.35 em below, `blur` sharpens it from 0.25 em; all three fade.

use super::Canvas;
use super::units::Units;
use crate::layout::TextLayout;
use crate::scene::{Color, Reveal, RevealFrom};
use parley::PositionedLayoutItem;
use vello_cpu::Glyph;
use vello_cpu::kurbo::Affine;

pub fn draw(
    c: &mut Canvas,
    t: Affine,
    layout: &TextLayout,
    text: &str,
    color: Color,
    r: &Reveal,
) -> Result<(), String> {
    let units = Units::new(text, r.by, layout.len());
    if units.count == 0 {
        return super::draw_all(c, t, layout, color);
    }
    let stagger = f64::from(r.stagger.clamp(0.0, 1.0));
    let u = 1.0 / ((units.count - 1) as f64 * stagger + 1.0);
    let d = stagger * u;
    let progress = f64::from(r.progress.clamp(0.0, 1.0));
    let unit_p = |i: usize| {
        let p = ((progress - i as f64 * d) / u).clamp(0.0, 1.0);
        1.0 - (1.0 - p).powi(3)
    };
    let mut glyphs: Vec<(Glyph, Option<usize>)> = Vec::new();
    for (line_index, line) in layout.lines().enumerate() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(run) = item else {
                continue;
            };
            glyphs.clear();
            glyphs.extend(run.positioned_glyphs().map(|g| {
                let glyph = Glyph {
                    id: g.id,
                    x: g.x,
                    y: g.y,
                };
                (glyph, None)
            }));
            // Clusters in visual order own the glyphs in the same order; a
            // ligature continuation owns none.
            let mut i = 0;
            for cluster in run.run().visual_clusters() {
                let unit = units.of(cluster.text_range().start, line_index);
                for _ in 0..cluster.glyphs().count() {
                    if let Some(g) = glyphs.get_mut(i) {
                        g.1 = unit;
                    }
                    i += 1;
                }
            }
            let settled = glyphs
                .iter()
                .filter(|(_, unit)| unit.is_none_or(|u| unit_p(u) >= 1.0))
                .map(|(g, _)| *g);
            c.glyphs(t, run.run(), settled, color)?;
            let em = f64::from(run.run().font_size());
            let mut k = 0;
            while k < glyphs.len() {
                let unit = glyphs[k].1;
                let end = k + glyphs[k..].iter().take_while(|g| g.1 == unit).count();
                if let Some(unit) = unit {
                    let p = unit_p(unit);
                    if p > 0.0 && p < 1.0 {
                        let span = glyphs[k..end].iter().map(|(g, _)| *g);
                        in_flight(c, t, run.run(), span, color, r.from, p, em)?;
                    }
                }
                k = end;
            }
        }
    }
    Ok(())
}

/// One unit part-way in, at eased progress `p` (0 < p < 1).
#[allow(clippy::too_many_arguments)] // one draw call's inputs
fn in_flight(
    c: &mut Canvas,
    t: Affine,
    run: &parley::Run<'_, ()>,
    glyphs: impl Iterator<Item = Glyph> + Clone,
    color: Color,
    from: RevealFrom,
    p: f64,
    em: f64,
) -> Result<(), String> {
    let alpha = p as f32;
    match from {
        RevealFrom::Fade => c.glyphs(t, run, glyphs, color.with_alpha_mul(alpha)),
        RevealFrom::Rise => {
            let lift = Affine::translate((0.0, (1.0 - p) * 0.35 * em));
            c.glyphs(t * lift, run, glyphs, color.with_alpha_mul(alpha))
        }
        RevealFrom::Blur => {
            c.push_group(t, alpha, ((1.0 - p) * 0.25 * em) as f32);
            let drawn = c.glyphs(t, run, glyphs, color);
            c.pop();
            drawn
        }
    }
}
