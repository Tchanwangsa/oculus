//! A page's text: glyphs collected from the content stream, grouped into
//! lines in reading order.

use hayro::hayro_interpret::font::{Glyph, GlyphRun, PositionedGlyph};
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::{
    interpret_page, BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image,
    ImageDrawProps, InterpreterCache, InterpreterSettings, SoftMask, TransformExt,
};
use hayro::hayro_syntax::page::Page;
use hayro::hayro_syntax::Pdf;
use hayro::kurbo::{Affine, BezPath, Rect, Shape};

use super::documents::{guarded, page_of};
use super::layout::{from_upright, layout, to_upright, union, TextGlyph};
use super::wire::{round, PageText, TextLine};

pub(super) fn page_text(pdf: &Pdf, page: u32) -> Result<PageText, String> {
    let page = page_of(pdf, page)?;
    guarded(|| extract_text(page))
}

/// A hayro device that paints nothing and records each glyph's character and
/// em box in rendered-page space, one flow per writing direction.
struct GlyphCollector {
    /// Text running right, down, left and up: a quarter turn each.
    flows: [Vec<TextGlyph>; 4],
    /// The last filled run: fill-and-stroke text draws the same run twice.
    last_fill: Option<(usize, usize)>,
    /// The rendered page, `(0, 0)` to its render dimensions.
    page: Rect,
}

impl<'a> Device<'a> for GlyphCollector {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, 'a>, props: DrawProps<'a>, mode: &DrawMode) {
        let glyphs = run.glyphs();
        let id = (glyphs.as_ptr() as usize, glyphs.len());
        if matches!(mode, DrawMode::Stroke(_)) {
            if self.last_fill.take() == Some(id) {
                return;
            }
        } else {
            self.last_fill = Some(id);
        }
        for glyph in glyphs {
            self.push(glyph, props.transform * glyph.transform());
        }
    }
    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

impl GlyphCollector {
    fn push(&mut self, glyph: &PositionedGlyph<'_>, transform: Affine) {
        let Some(unicode) = glyph.as_unicode() else {
            return;
        };
        let text = match unicode {
            BfString::Char(ch) => ch.to_string(),
            BfString::String(text) => text,
        };
        let chars: Vec<char> = text.chars().filter(|ch| !ch.is_control()).collect();
        if chars.is_empty() {
            return;
        }
        // Glyph space is 1000 units per em; the em box runs from the
        // descender (-200) to the ascender (800) and across the advance.
        let advance = match &**glyph {
            Glyph::Outline(outline) => outline
                .advance_width()
                .filter(|advance| advance.is_finite() && *advance > 0.0)
                .map(f64::from)
                .unwrap_or_else(|| outline.outline().bounding_box().width().max(500.0)),
            Glyph::Type3(_) => 600.0,
        };
        let bounds = transform.transform_rect_bbox(Rect::new(0.0, -200.0, advance, 800.0));
        if ![bounds.x0, bounds.y0, bounds.x1, bounds.y1]
            .iter()
            .all(|v| v.is_finite())
            || bounds.width() > 10_000.0
            || bounds.height() > 10_000.0
        {
            return;
        }
        // Text no reader sees: zero-scale glyphs (LaTeXiT and Keynote embed
        // equation source that way) and glyphs parked off the page (Beamer
        // overlays). Thresholds match the reference-text comparison in app/pdf-bench.
        if !(bounds.area() > 1e-4) || bounds.intersect(self.page).area() <= 0.0 {
            return;
        }
        // The baseline's direction, to the nearest quarter turn (y down).
        let [dx, dy, ..] = transform.as_coeffs();
        let quarter = if dx.abs() >= dy.abs() {
            if dx >= 0.0 {
                0
            } else {
                2
            }
        } else if dy > 0.0 {
            1
        } else {
            3
        };
        // A ligature's characters share its box evenly, along the baseline.
        let n = chars.len() as f64;
        for (i, ch) in chars.into_iter().enumerate() {
            let (from, to) = (i as f64 / n, (i + 1) as f64 / n);
            let rect = match quarter {
                0 => [
                    bounds.x0 + bounds.width() * from,
                    bounds.y0,
                    bounds.x0 + bounds.width() * to,
                    bounds.y1,
                ],
                2 => [
                    bounds.x1 - bounds.width() * to,
                    bounds.y0,
                    bounds.x1 - bounds.width() * from,
                    bounds.y1,
                ],
                1 => [
                    bounds.x0,
                    bounds.y0 + bounds.height() * from,
                    bounds.x1,
                    bounds.y0 + bounds.height() * to,
                ],
                _ => [
                    bounds.x0,
                    bounds.y1 - bounds.height() * to,
                    bounds.x1,
                    bounds.y1 - bounds.height() * from,
                ],
            };
            self.flows[quarter].push(TextGlyph {
                ch,
                rect: rect.map(|v| v as f32),
            });
        }
    }
}

/// The page's text lines in reading order. Each writing direction is laid out
/// on its own, upright, and the flows join by their top-most glyph.
fn extract_text(page: &Page<'_>) -> PageText {
    let (width, height) = page.render_dimensions();
    let cache = InterpreterCache::new();
    let mut context = Context::new(
        page.initial_transform(true).to_kurbo(),
        Rect::new(0.0, 0.0, width as f64, height as f64),
        &cache,
        page.xref(),
        InterpreterSettings::default(),
    );
    let mut collector = GlyphCollector {
        flows: Default::default(),
        last_fill: None,
        page: Rect::new(0.0, 0.0, width as f64, height as f64),
    };
    interpret_page(page, &mut context, &mut collector);

    let mut flows = Vec::new();
    for (quarter, mut glyphs) in collector.flows.into_iter().enumerate() {
        if glyphs.is_empty() {
            continue;
        }
        let top = glyphs
            .iter()
            .map(|glyph| glyph.rect[1])
            .fold(f32::INFINITY, f32::min);
        for glyph in &mut glyphs {
            glyph.rect = to_upright(glyph.rect, quarter, width, height);
        }
        let lines: Vec<TextLine> = layout(glyphs)
            .iter()
            .filter_map(|line| emit_line(line, quarter, width, height))
            .collect();
        flows.push((top, lines));
    }
    flows.sort_by(|a, b| a.0.total_cmp(&b.0));
    PageText {
        lines: flows.into_iter().flat_map(|(_, lines)| lines).collect(),
    }
}

/// A laid-out line (upright frame) as the command's line: its text with the
/// inferred word spaces, its box, and one stop per UTF-16 unit plus the end.
fn emit_line(
    line: &[(TextGlyph, bool)],
    quarter: usize,
    width: f32,
    height: f32,
) -> Option<TextLine> {
    let mut text = String::new();
    let mut stops = Vec::new();
    let mut bounds = line.first()?.0.rect;
    for (k, (glyph, space_before)) in line.iter().enumerate() {
        if *space_before && k > 0 {
            text.push(' ');
            stops.push(line[k - 1].0.rect[2]);
        }
        text.push(glyph.ch);
        stops.extend(std::iter::repeat(glyph.rect[0]).take(glyph.ch.len_utf16()));
        union(&mut bounds, &glyph.rect);
    }
    stops.push(line.last()?.0.rect[2]);
    if text.trim().is_empty() {
        return None;
    }
    let [x0, y0, x1, y1] = from_upright(bounds, quarter, width, height);
    // An upright x back to the page axis the line runs along.
    let along = |u: f32| match quarter {
        2 => width - u,
        3 => height - u,
        _ => u,
    };
    let to = |v: f32| round(v as f64);
    Some(TextLine {
        text,
        x: to(x0),
        y: to(y0),
        width: to(x1 - x0),
        height: to(y1 - y0),
        vertical: quarter % 2 == 1,
        chars: stops.into_iter().map(|u| to(along(u))).collect(),
    })
}
