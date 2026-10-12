//! A horizontal brace as KaTeX draws `\overbrace` and `\underbrace`: three
//! slices of its SVG (left end, middle tip, right end) in spans 25.1%, 50%
//! and 25.1% of the width, each sliced from a 400000-unit path 548 units
//! (0.548 em) tall and clipped to its span by clamping its points, as
//! `walk/svg.rs` does for the braces inside a formula.

use katex::svg_geometry::PATH_MAP;
use vello_cpu::kurbo::{Affine, BezPath, PathEl, Point, Rect};

/// KaTeX's brace height and its narrowest width, in em.
pub const BRACE_HEIGHT: f64 = 0.548;
pub const BRACE_MIN_WIDTH: f64 = 1.6;

/// The brace `width` em wide (at least `BRACE_MIN_WIDTH`), in em, its box
/// from (0, 0); `under` points its tip down.
pub fn brace(width: f64, under: bool) -> BezPath {
    let w = width.max(BRACE_MIN_WIDTH);
    let names = if under {
        ["leftbraceunder", "midbraceunder", "rightbraceunder"]
    } else {
        ["leftbrace", "midbrace", "rightbrace"]
    };
    // Span (left, width) and where the 400 em path aligns in it: 0 left,
    // 0.5 centre, 1 right (`xMinYMin`, `xMidYMin`, `xMaxYMin` slice).
    let spans = [
        (0.0, 0.251 * w, 0.0),
        (0.25 * w, 0.5 * w, 0.5),
        (0.749 * w, 0.251 * w, 1.0),
    ];
    let mut out = BezPath::new();
    for (name, (x0, sw, align)) in names.into_iter().zip(spans) {
        let Some(d) = PATH_MAP.get(name).and_then(|d| BezPath::from_svg(d).ok()) else {
            continue;
        };
        let s = 0.001;
        let tx = x0 + align * (sw - 400_000.0 * s);
        let t = Affine::new([s, 0.0, 0.0, s, tx, 0.0]);
        let clip = Rect::new(x0, 0.0, x0 + sw, BRACE_HEIGHT);
        out.extend(clamped(&d, t, clip));
    }
    out
}

fn clamped(d: &BezPath, t: Affine, clip: Rect) -> BezPath {
    let f = |p: Point| {
        let q = t * p;
        Point::new(q.x.clamp(clip.x0, clip.x1), q.y.clamp(clip.y0, clip.y1))
    };
    d.elements()
        .iter()
        .map(|el| match *el {
            PathEl::MoveTo(p) => PathEl::MoveTo(f(p)),
            PathEl::LineTo(p) => PathEl::LineTo(f(p)),
            PathEl::QuadTo(a, p) => PathEl::QuadTo(f(a), f(p)),
            PathEl::CurveTo(a, b, p) => PathEl::CurveTo(f(a), f(b), f(p)),
            PathEl::ClosePath => PathEl::ClosePath,
        })
        .collect()
}
