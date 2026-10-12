//! `Path` paint: the path is fitted into the content box from its viewBox
//! (`xMidYMid meet`), filled, then stroked up to its trim.

mod trim;

use super::Canvas;
use crate::scene::{LineCap, LineJoin, PathSpec, StrokeSpec};
use trim::trim;
use vello_cpu::kurbo::{Affine, BezPath, Cap, Join, Rect, Shape, Stroke};

pub fn draw(c: &mut Canvas, t: Affine, content: Rect, p: &PathSpec) {
    let t = t * fit(p.view_box, content);
    if let Some(f) = &p.fill {
        c.fill_shape(t, &p.d, f, p.d.bounding_box());
    }
    let Some(s) = &p.stroke else {
        return;
    };
    if s.width <= 0.0 {
        return;
    }
    let (start, end) = p.trim.unwrap_or((0.0, 1.0));
    if start <= 0.0 && end >= 1.0 {
        c.stroke_shape(t, &p.d, stroke(s, 0.0), s.color);
        return;
    }
    // Each subpath restarts the dash pattern, so a piece cut mid-subpath
    // carries its offset into it and dashes stay put as the trim moves.
    let mut whole = BezPath::new();
    for piece in trim(&p.d, start, end) {
        if piece.phase > 0.0 && !s.dash.is_empty() {
            c.stroke_shape(t, &piece.path, stroke(s, piece.phase), s.color);
        } else {
            whole.extend(piece.path.elements().iter().copied());
        }
    }
    if !whole.elements().is_empty() {
        c.stroke_shape(t, &whole, stroke(s, 0.0), s.color);
    }
}

/// viewBox -> content box, scaled uniformly to fit and centred; a zero
/// side does not constrain the scale.
fn fit(view: Rect, into: Rect) -> Affine {
    let sx = (view.width() > 0.0).then(|| into.width() / view.width());
    let sy = (view.height() > 0.0).then(|| into.height() / view.height());
    let s = match (sx, sy) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => 1.0,
    };
    let (c, v) = (into.center(), view.center());
    Affine::translate((c.x - v.x * s, c.y - v.y * s)) * Affine::scale(s)
}

/// The kurbo stroke, with SVG's defaults (butt caps, miter joins, limit 4).
fn stroke(s: &StrokeSpec, phase: f64) -> Stroke {
    let cap = match s.cap {
        LineCap::Butt => Cap::Butt,
        LineCap::Round => Cap::Round,
        LineCap::Square => Cap::Square,
    };
    let join = match s.join {
        LineJoin::Miter => Join::Miter,
        LineJoin::Round => Join::Round,
        LineJoin::Bevel => Join::Bevel,
    };
    let k = Stroke::new(s.width)
        .with_caps(cap)
        .with_join(join)
        .with_miter_limit(4.0);
    if s.dash.iter().sum::<f64>() > 0.0 {
        k.with_dashes(s.dash_offset + phase, &s.dash)
    } else {
        k
    }
}
