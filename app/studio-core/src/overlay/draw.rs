//! One drawing overlay -> Paths in its own node, from its target's box in
//! the overlay's parent space. Defaults scale with the target's em (a
//! Math's size, `targets::em`): a brace KaTeX's thickness at that size and
//! 0.12 em off, a Surround 0.2 em out with 0.12 em corners, an Underline
//! 0.1 em below; strokes 0.045 em. Brace grows from its centre and its
//! label fades in over the second half; Surround, Underline and Arrow draw on.

use super::arrow::{Look, arrow, edge};
use super::targets::{Found, Index, scale_of};
use super::{OverlayError, OverlayInk};
use crate::easing::in_out_cubic;
use crate::layout::LNode;
use crate::math::{BRACE_HEIGHT, brace};
use crate::scene::{
    End, Fill, Kind, LineCap, LineJoin, OverlayKind, OverlaySpec, PathSpec, Placed, Side,
    StrokeSpec, Style,
};
use std::f64::consts::FRAC_PI_2;
use vello_cpu::kurbo::{Affine, BezPath, Point, Rect, Shape, Vec2};

/// What an overlay is drawn from, in its parent's space: the target's box
/// (or an arrow's two ends) and its em.
#[derive(Clone, Copy)]
struct Measured {
    rect: Rect,
    /// How visible the target is (an arrow: the less visible end).
    alpha: f64,
    ends: Option<(Point, Point)>,
    em: f64,
    progress: f64,
}

pub fn overlay(
    n: &mut LNode,
    placed: &Placed,
    parent: Affine,
    index: &Index,
    ink: OverlayInk,
) -> Result<(), OverlayError> {
    let spec = &placed.now;
    let mut m = measure(spec, parent, index, n.loc)?;
    if let Some((was, p)) = &placed.was
        && std::mem::discriminant(&was.kind) == std::mem::discriminant(&spec.kind)
    {
        let a = measure(was, parent, index, n.loc)?;
        m = lerp(a, m, f64::from(*p));
    }
    let width = |em: f64| spec.width.unwrap_or(0.045 * em);
    let mut shapes: Vec<LNode> = Vec::new();
    let mut label_at: Option<(Rect, Side)> = None;
    let mut label_alpha = 1.0;
    match &spec.kind {
        OverlayKind::Brace {
            side, gap, size, ..
        } => {
            let em = size.unwrap_or(m.em);
            let gap = gap.unwrap_or(0.12 * m.em);
            let d = brace_path(m.rect, *side, gap, em);
            let mut node = path_node(d, Some(Fill::Solid(spec.color.unwrap_or(ink.text))), None);
            let k = m.progress.clamp(0.0, 1.0) as f32;
            if k < 1.0 {
                node.style.scale = Some(k);
            }
            label_at = Some((node.rect.inflate(0.15 * m.em, 0.15 * m.em), *side));
            label_alpha = in_out_cubic(((m.progress as f32 - 0.4) / 0.6).clamp(0.0, 1.0));
            shapes.push(node);
        }
        OverlayKind::Surround { pad, radius, .. } => {
            let pad = pad.unwrap_or(0.2 * m.em);
            let r = radius.unwrap_or(0.12 * m.em);
            let d = rounded(m.rect.inflate(pad, pad), r);
            let stroke = stroke(width(m.em), spec.color.unwrap_or(ink.accent));
            shapes.push(drawn(d, stroke, m.progress));
        }
        OverlayKind::Underline { gap, .. } => {
            let y = m.rect.y1 + gap.unwrap_or(0.1 * m.em);
            let mut d = BezPath::new();
            d.move_to((m.rect.x0, y));
            d.line_to((m.rect.x1, y));
            let stroke = stroke(width(m.em), spec.color.unwrap_or(ink.accent));
            shapes.push(drawn(d, stroke, m.progress));
        }
        OverlayKind::Arrow { bend, head, .. } => {
            let Some((from, to)) = m.ends else {
                return Ok(());
            };
            let w = spec.width.unwrap_or(4.0);
            let look = Look {
                width: w,
                head: head.unwrap_or(w * 4.5),
                color: spec.color.unwrap_or(ink.text),
                bend: *bend,
                progress: m.progress,
            };
            for (d, s) in arrow(from, to, &look) {
                shapes.push(match s {
                    Some(s) => drawn(d, s, m.progress),
                    None => path_node(d, Some(Fill::Solid(look.color)), None),
                });
            }
        }
        OverlayKind::Indicate { .. } => {}
    }
    if m.alpha < 1.0 {
        n.style.opacity = Some(n.style.opacity() * m.alpha as f32);
    }
    if let Some((around, side)) = label_at {
        place_label(&mut n.children, around, side, label_alpha);
    }
    let bounds = shapes
        .iter()
        .chain(n.children.iter())
        .map(|c| c.rect)
        .reduce(|a, b| a.union(b));
    n.children.splice(0..0, shapes);
    if let Some(r) = bounds {
        n.rect = r;
        n.content = r;
    }
    Ok(())
}

fn lerp(a: Measured, b: Measured, p: f64) -> Measured {
    let l = |x: f64, y: f64| x + (y - x) * p;
    let lp = |x: Point, y: Point| x.lerp(y, p);
    Measured {
        rect: Rect::new(
            l(a.rect.x0, b.rect.x0),
            l(a.rect.y0, b.rect.y0),
            l(a.rect.x1, b.rect.x1),
            l(a.rect.y1, b.rect.y1),
        ),
        ends: match (a.ends, b.ends) {
            (Some((a0, a1)), Some((b0, b1))) => Some((lp(a0, b0), lp(a1, b1))),
            (_, e) => e,
        },
        em: l(a.em, b.em),
        progress: l(a.progress, b.progress),
        alpha: l(a.alpha, b.alpha),
    }
}

/// A spec's target box (or arrow ends) and em in the parent's space.
fn measure(
    spec: &OverlaySpec,
    parent: Affine,
    index: &Index,
    loc: Option<(u32, u32)>,
) -> Result<Measured, OverlayError> {
    let inv = parent.inverse();
    let scale = scale_of(parent).max(1e-9);
    let find = |key: &str| -> Result<(Rect, Option<f64>, f64), OverlayError> {
        let Found { rect, em, alpha } = index
            .find(key)
            .ok_or_else(|| index.missing(spec.name(), key, loc))?;
        Ok((inv.transform_rect_bbox(rect), em.map(|e| e / scale), alpha))
    };
    let progress = f64::from(spec.progress);
    let em_of = |r: Rect, em: Option<f64>| em.unwrap_or(r.height() / 1.2).max(1.0);
    match &spec.kind {
        OverlayKind::Brace { target, .. }
        | OverlayKind::Surround { target, .. }
        | OverlayKind::Underline { target, .. }
        | OverlayKind::Indicate { target, .. } => {
            let (rect, em, alpha) = find(target)?;
            Ok(Measured {
                rect,
                alpha,
                ends: None,
                em: em_of(rect, em),
                progress,
            })
        }
        OverlayKind::Arrow { from, to, gap, .. } => {
            let end = |e: &End| -> Result<(Rect, Option<f64>, f64), OverlayError> {
                match e {
                    End::Key(k) => find(k),
                    End::Point(p) => Ok((Rect::from_points(*p, *p), None, 1.0)),
                }
            };
            let ((ra, ea, aa), (rb, eb, ab)) = (end(from)?, end(to)?);
            let em = ea.or(eb).unwrap_or(32.0);
            let gap = gap.unwrap_or(0.1 * em);
            let keyed = |e: &End| matches!(e, End::Key(_));
            let a = if keyed(from) {
                edge(ra, rb.center(), gap)
            } else {
                ra.center()
            };
            let b = if keyed(to) {
                edge(rb, ra.center(), gap)
            } else {
                rb.center()
            };
            Ok(Measured {
                rect: Rect::from_points(a, b),
                alpha: aa.min(ab),
                ends: Some((a, b)),
                em,
                progress,
            })
        }
    }
}

/// KaTeX's brace along one side of `r`, `gap` off it, `em` px per em,
/// centred on that side.
fn brace_path(r: Rect, side: Side, gap: f64, em: f64) -> BezPath {
    let h = BRACE_HEIGHT * em;
    let along = match side {
        Side::Below | Side::Above => r.width(),
        Side::Left | Side::Right => r.height(),
    };
    let mut d = brace(along / em, matches!(side, Side::Below | Side::Left));
    let w = d.bounding_box().x1.max(along / em) * em;
    let c = r.center();
    let t = match side {
        Side::Below => Affine::translate((c.x - w / 2.0, r.y1 + gap)),
        Side::Above => Affine::translate((c.x - w / 2.0, r.y0 - gap - h)),
        Side::Left => Affine::translate((r.x0 - gap, c.y - w / 2.0)) * Affine::rotate(FRAC_PI_2),
        Side::Right => {
            Affine::translate((r.x1 + gap + h, c.y - w / 2.0)) * Affine::rotate(FRAC_PI_2)
        }
    };
    d.apply_affine(t * Affine::scale(em));
    d
}

/// A rounded rect's outline, clockwise from the top-left corner's end, so
/// a trim draws it on from there.
fn rounded(r: Rect, radius: f64) -> BezPath {
    let k = radius.min(r.width() / 2.0).min(r.height() / 2.0).max(0.0);
    let c = k * 0.552_284_75;
    let mut d = BezPath::new();
    d.move_to((r.x0 + k, r.y0));
    d.line_to((r.x1 - k, r.y0));
    d.curve_to((r.x1 - k + c, r.y0), (r.x1, r.y0 + k - c), (r.x1, r.y0 + k));
    d.line_to((r.x1, r.y1 - k));
    d.curve_to((r.x1, r.y1 - k + c), (r.x1 - k + c, r.y1), (r.x1 - k, r.y1));
    d.line_to((r.x0 + k, r.y1));
    d.curve_to((r.x0 + k - c, r.y1), (r.x0, r.y1 - k + c), (r.x0, r.y1 - k));
    d.line_to((r.x0, r.y0 + k));
    d.curve_to((r.x0, r.y0 + k - c), (r.x0 + k - c, r.y0), (r.x0 + k, r.y0));
    d.close_path();
    d
}

fn stroke(width: f64, color: crate::scene::Color) -> StrokeSpec {
    StrokeSpec {
        width,
        color,
        cap: LineCap::Round,
        join: LineJoin::Round,
        dash: Vec::new(),
        dash_offset: 0.0,
    }
}

/// A stroked path drawn on to `progress`.
fn drawn(d: BezPath, s: StrokeSpec, progress: f64) -> LNode {
    let q = progress.clamp(0.0, 1.0);
    let mut n = path_node(d, None, Some(s));
    if q < 1.0
        && let Some(p) = &mut n.path
    {
        p.trim = Some((0.0, q));
    }
    if q <= 0.0 {
        n.style.opacity = Some(0.0);
    }
    n
}

/// A Path whose viewBox is its own box in the parent's space, so it draws
/// at the coordinates it was built in.
fn path_node(d: BezPath, fill: Option<Fill>, stroke: Option<StrokeSpec>) -> LNode {
    let pad = stroke.as_ref().map_or(0.0, |s| s.width) + 1.0;
    let r = d.bounding_box().inflate(pad, pad);
    LNode {
        kind: Kind::Path,
        style: Style::default(),
        rect: r,
        content: r,
        text: None,
        image: None,
        path: Some(Box::new(PathSpec {
            d,
            view_box: r,
            fill,
            stroke,
            trim: None,
        })),
        children: Vec::new(),
        key: None,
        loc: None,
        scope: None,
        overlay: None,
    }
}

/// Moves the label (the overlay's own children) beside `around` on `side`
/// and fades it to `alpha`.
fn place_label(kids: &mut [LNode], around: Rect, side: Side, alpha: f32) {
    let Some(r) = kids.iter().map(|c| c.rect).reduce(|a, b| a.union(b)) else {
        return;
    };
    let c = around.center();
    let at = match side {
        Side::Below => Point::new(c.x - r.width() / 2.0, around.y1),
        Side::Above => Point::new(c.x - r.width() / 2.0, around.y0 - r.height()),
        Side::Left => Point::new(around.x0 - r.width(), c.y - r.height() / 2.0),
        Side::Right => Point::new(around.x1, c.y - r.height() / 2.0),
    };
    let d = at - r.origin();
    for k in kids.iter_mut() {
        shift(k, d);
        if alpha < 1.0 {
            k.style.opacity = Some(k.style.opacity() * alpha);
        }
    }
}

fn shift(n: &mut LNode, d: Vec2) {
    n.rect = n.rect + d;
    n.content = n.content + d;
    for c in &mut n.children {
        shift(c, d);
    }
}
