//! Trim: the part of a path between two fractions of its total arc length,
//! across subpaths in order. Cuts are exact (`inv_arclen` + `subsegment`), so
//! caps land on the trimmed ends. `end_of` measures where a `[0, f]` trim
//! ends with the same arc lengths, for the prelude's `Arrow` head.

use vello_cpu::kurbo::{
    BezPath, CubicBez, Line, ParamCurve, ParamCurveArclen, ParamCurveDeriv, PathEl, PathSeg, Point,
    QuadBez, Rect, Shape, Vec2,
};

/// One continuous run of the trimmed path.
pub struct Piece {
    pub path: BezPath,
    /// Arc length from its subpath's start to where this piece starts.
    pub phase: f64,
}

struct Seg {
    seg: PathSeg,
    sub: usize,
    len: f64,
}

struct Sub {
    /// Total length before this subpath.
    start: f64,
    len: f64,
    closed: bool,
}

pub fn trim(path: &BezPath, start: f64, end: f64) -> Vec<Piece> {
    let accuracy = accuracy(path.bounding_box());
    let (segs, subs) = segments(path, accuracy);
    let total: f64 = segs.iter().map(|s| s.len).sum();
    let (s0, s1) = (start.clamp(0.0, 1.0) * total, end.clamp(0.0, 1.0) * total);
    let mut pieces: Vec<Piece> = Vec::new();
    // The segment the open piece last reached the end of, if any.
    let mut open: Option<usize> = None;
    let mut at = 0.0;
    for (i, s) in segs.iter().enumerate() {
        let (a, b) = (at, at + s.len);
        at = b;
        let (lo, hi) = (a.max(s0), b.min(s1));
        if hi <= lo {
            // A zero-length segment inside the range keeps the piece going.
            if s.len == 0.0 && i > 0 && open == Some(i - 1) && a < s1 {
                open = Some(i);
            }
            continue;
        }
        let t0 = if lo > a {
            s.seg.inv_arclen(lo - a, accuracy)
        } else {
            0.0
        };
        let t1 = if hi < b {
            s.seg.inv_arclen(hi - a, accuracy)
        } else {
            1.0
        };
        let part = s.seg.subsegment(t0..t1);
        let joins = i > 0 && open == Some(i - 1) && segs[i - 1].sub == s.sub;
        if !joins {
            let mut p = BezPath::new();
            p.move_to(part.start());
            pieces.push(Piece {
                path: p,
                phase: lo - subs[s.sub].start,
            });
        }
        if let Some(piece) = pieces.last_mut() {
            push(&mut piece.path, part);
            let sub = &subs[s.sub];
            let whole = piece.phase == 0.0 && hi >= sub.start + sub.len;
            if sub.closed && whole && is_last_of(&segs, i) {
                piece.path.close_path();
            }
        }
        open = (hi >= b).then_some(i);
    }
    pieces
}

/// Where a `[0, fraction]` trim of a path ends.
pub struct End {
    pub point: Point,
    /// The unit direction of travel there.
    pub tangent: Vec2,
    /// The whole path's arc length.
    pub length: f64,
}

/// The point `trim(path, 0, fraction)` ends at and the path's direction
/// there; `None` for a path with no length. At a corner it is the end of
/// the segment arriving there, as with the trim.
pub fn end_of(path: &BezPath, fraction: f64) -> Option<End> {
    let accuracy = accuracy(path.bounding_box());
    let (segs, _) = segments(path, accuracy);
    let length: f64 = segs.iter().map(|s| s.len).sum();
    let s1 = fraction.clamp(0.0, 1.0) * length;
    let mut at = 0.0;
    for s in &segs {
        let (a, b) = (at, at + s.len);
        at = b;
        if s.len == 0.0 || b < s1 {
            continue;
        }
        let t = if s1 < b {
            s.seg.inv_arclen(s1 - a, accuracy)
        } else {
            1.0
        };
        return Some(End {
            point: s.seg.eval(t),
            tangent: tangent(s.seg, t)?,
            length,
        });
    }
    None
}

/// The unit derivative at `t`, or the nearby chord where the derivative
/// vanishes (a control point on its end point).
fn tangent(seg: PathSeg, t: f64) -> Option<Vec2> {
    let d = match seg {
        PathSeg::Line(l) => l.p1 - l.p0,
        PathSeg::Quad(q) => q.deriv().eval(t).to_vec2(),
        PathSeg::Cubic(c) => c.deriv().eval(t).to_vec2(),
    };
    let d = if d.hypot() > 1e-9 {
        d
    } else if t > 0.5 {
        seg.eval(t) - seg.eval(t - 1e-3)
    } else {
        seg.eval(t + 1e-3) - seg.eval(t)
    };
    (d.hypot() > 1e-12).then(|| d / d.hypot())
}

/// Arc-length accuracy relative to the path's size.
fn accuracy(bounds: Rect) -> f64 {
    ((bounds.width() + bounds.height()) * 1e-6).max(1e-9)
}

fn is_last_of(segs: &[Seg], i: usize) -> bool {
    segs.get(i + 1).is_none_or(|n| n.sub != segs[i].sub)
}

fn segments(path: &BezPath, accuracy: f64) -> (Vec<Seg>, Vec<Sub>) {
    let mut segs = Vec::new();
    let mut subs: Vec<Sub> = Vec::new();
    let (mut first, mut last) = (Point::ORIGIN, Point::ORIGIN);
    let add = |seg: PathSeg, segs: &mut Vec<Seg>, subs: &mut Vec<Sub>| {
        let len = seg.arclen(accuracy);
        if let Some(sub) = subs.last_mut() {
            sub.len += len;
        }
        segs.push(Seg {
            seg,
            sub: subs.len().saturating_sub(1),
            len,
        });
    };
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                let start = subs.last().map_or(0.0, |s| s.start + s.len);
                subs.push(Sub {
                    start,
                    len: 0.0,
                    closed: false,
                });
                (first, last) = (p, p);
            }
            PathEl::LineTo(p) => {
                add(PathSeg::Line(Line::new(last, p)), &mut segs, &mut subs);
                last = p;
            }
            PathEl::QuadTo(a, p) => {
                add(
                    PathSeg::Quad(QuadBez::new(last, a, p)),
                    &mut segs,
                    &mut subs,
                );
                last = p;
            }
            PathEl::CurveTo(a, b, p) => {
                add(
                    PathSeg::Cubic(CubicBez::new(last, a, b, p)),
                    &mut segs,
                    &mut subs,
                );
                last = p;
            }
            PathEl::ClosePath => {
                if last != first {
                    add(PathSeg::Line(Line::new(last, first)), &mut segs, &mut subs);
                }
                if let Some(s) = subs.last_mut() {
                    s.closed = true;
                }
                last = first;
            }
        }
    }
    (segs, subs)
}

fn push(path: &mut BezPath, seg: PathSeg) {
    match seg {
        PathSeg::Line(l) => path.line_to(l.p1),
        PathSeg::Quad(q) => path.quad_to(q.p1, q.p2),
        PathSeg::Cubic(c) => path.curve_to(c.p1, c.p2, c.p3),
    }
}

#[cfg(test)]
mod tests;
