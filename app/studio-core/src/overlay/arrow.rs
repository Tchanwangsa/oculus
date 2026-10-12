//! A keyed `<Arrow>`: its ends are points or targets' boxes (left `gap`
//! short of the box edge, on the line between the two ends' centres), the
//! shaft straight or an arc turning through `bend` degrees, drawn up to
//! `progress` with the head riding the drawn end as the prelude's Arrow
//! does (`prelude/arrow.js`): the shaft stops inside the head, and the
//! head grows in over its first head length.

use crate::paint::end_of;
use crate::scene::{Color, LineCap, LineJoin, StrokeSpec};
use vello_cpu::kurbo::{BezPath, Point, Rect, Vec2};

/// Where a keyed end leaves its box: from the centre towards `other`,
/// `gap` past the edge.
pub fn edge(r: Rect, other: Point, gap: f64) -> Point {
    let c = r.center();
    let d = other - c;
    let len = d.hypot();
    if len < 1e-9 {
        return c;
    }
    let u = d / len;
    let (hw, hh) = (r.width() / 2.0, r.height() / 2.0);
    let tx = if u.x.abs() > 1e-9 {
        hw / u.x.abs()
    } else {
        f64::INFINITY
    };
    let ty = if u.y.abs() > 1e-9 {
        hh / u.y.abs()
    } else {
        f64::INFINITY
    };
    c + u * (tx.min(ty) + gap)
}

pub struct Look {
    pub width: f64,
    pub head: f64,
    pub color: Color,
    pub bend: f64,
    pub progress: f64,
}

/// The shaft's path and stroke, and the head's filled triangle.
pub fn arrow(from: Point, to: Point, look: &Look) -> Vec<(BezPath, Option<StrokeSpec>)> {
    let chord = to - from;
    let len = chord.hypot();
    if len < 1e-9 {
        return Vec::new();
    }
    // An arc as one quadratic: the control point off the chord's middle so
    // each end's tangent turns half the bend from the chord.
    let normal = Vec2::new(-chord.y, chord.x) / len;
    let lift = (len / 2.0) * (look.bend.to_radians() / 2.0).tan();
    let control = from.midpoint(to) - normal * lift;
    let curved = look.bend.abs() > 1e-6;
    let end_dir = if curved { to - control } else { chord };
    let dir = end_dir / end_dir.hypot().max(1e-9);
    let back = if look.head > 0.0 {
        (look.head * 0.6).min(len * 0.9)
    } else {
        0.0
    };
    let end = to - dir * back;
    let mut d = BezPath::new();
    d.move_to(from);
    if curved {
        d.quad_to(control, end);
    } else {
        d.line_to(end);
    }
    let q = look.progress.clamp(0.0, 1.0);
    let stroke = StrokeSpec {
        width: look.width,
        color: look.color,
        cap: LineCap::Round,
        join: LineJoin::Round,
        dash: Vec::new(),
        dash_offset: 0.0,
    };
    let mut out = vec![(d.clone(), Some(stroke))];
    if look.head <= 0.0 {
        return out;
    }
    let Some(at) = end_of(&d, q) else {
        return out;
    };
    let u = at.tangent;
    let k = ((q * at.length) / look.head.min(at.length)).clamp(0.0, 1.0);
    let tip = at.point + u * back * k;
    let s = look.head * k;
    let base = tip - u * s;
    let side = Vec2::new(-u.y, u.x) * s * 0.5;
    let mut head = BezPath::new();
    head.move_to(tip);
    head.line_to(base + side);
    head.line_to(base - side);
    head.close_path();
    out.push((head, None));
    out
}
