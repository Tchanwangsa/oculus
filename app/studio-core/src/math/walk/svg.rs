//! KaTeX's SVG: root signs, tall and stretchy delimiters, braces, arrows,
//! `\cancel` strokes, and the frames of `\boxed` and `\colorbox`. An `<svg>`
//! fills its holder span (`width: 100%`, `height: inherit`, absolutely
//! placed at the holder's top), maps its `viewBox` by
//! `preserveAspectRatio` and is clipped to its viewport and to every
//! `overflow: hidden` box around it (`hide-tail`, `katex-stretchy` and the
//! brace and half-arrow pieces). KaTeX's paths run far past what shows (a
//! root sign's tail to x = 400000); clipping is done by clamping every
//! point into the clip rect, which is exact for those axis-aligned runs.
//! Each path becomes one piece, in em relative to its clip rect.

use super::super::Shape;
use super::{Ctx, Laid, Piece, css, inside, rule};
use katex::dom_tree::{DomSpan, HtmlDomNode, SvgChildNode, SvgNode};
use katex::namespace::AttrMap;
use katex::svg_geometry::PATH_MAP;
use katex::types::CssProperty;
use vello_cpu::kurbo::{Affine, BezPath, PathEl, Point, Rect};

/// Absolutely placed pieces of a stretchy element: class, left edge and
/// width as fractions of the holder, and whether `right: 0` places it.
const PIECES: [(&str, f64, f64, bool); 5] = [
    ("brace-left", 0.0, 0.251, false),
    ("brace-center", 0.25, 0.5, false),
    ("brace-right", 0.0, 0.251, true),
    ("halfarrow-left", 0.0, 0.502, false),
    ("halfarrow-right", 0.0, 0.502, true),
];

/// Classes whose `width: 100%` fills their vlist, clipping what overflows.
const FILLS: [&str; 2] = ["hide-tail", "katex-stretchy"];

fn piece_of(s: &DomSpan) -> Option<(f64, f64, bool)> {
    PIECES
        .iter()
        .find(|(class, ..)| s.classes.contains(class))
        .map(|&(_, left, width, right)| (left, width, right))
}

/// A span that holds SVG (directly or in pieces), or is a stretchy box.
pub fn is_holder(s: &DomSpan) -> bool {
    FILLS.iter().any(|c| s.classes.contains(c))
        || s.children.iter().any(|c| match c {
            HtmlDomNode::SvgNode(_) => true,
            HtmlDomNode::DomSpan(p) => piece_of(p).is_some(),
            _ => false,
        })
}

pub fn holder<'a>(s: &'a DomSpan, cx: Ctx<'a>) -> Result<Laid<'a>, String> {
    let st = &s.style;
    let size = cx.size;
    let fills = FILLS.iter().any(|c| s.classes.contains(c));
    let explicit = css::get(st, CssProperty::Width, size, cx.avail);
    let min = css::or_zero(st, CssProperty::MinWidth, size);
    let width = explicit
        .or(if fills { cx.avail } else { None })
        .unwrap_or(0.0)
        .max(min);
    let height = css::get(st, CssProperty::Height, size, None);
    let first_svg = s.children.iter().find_map(|c| match c {
        HtmlDomNode::SvgNode(svg) => Some(svg),
        _ => None,
    });
    let height = height
        .or_else(|| first_svg.and_then(|svg| attr_len(&svg.attributes, "height", size)))
        .unwrap_or(0.0);
    let rect = Rect::new(0.0, -height, width, 0.0);
    let mut out = Laid::empty(width);
    out.fills = fills && explicit.is_none();
    // A holder that is not positioned (`\cancel`'s) leaves an svg's
    // `width: 100%` to the vlist row around it.
    let viewport = if explicit.is_some() || fills {
        rect
    } else {
        out.fills = true;
        Rect::new(0.0, -height, cx.avail.unwrap_or(0.0), 0.0)
    };
    let clip = if fills {
        rect
    } else {
        Rect::new(f64::MIN, f64::MIN, f64::MAX, f64::MAX)
    };
    if let Some(bg) = st.get(CssProperty::BackgroundColor) {
        let mut p = rule::piece("box", rect, cx);
        p.colour = Some(bg);
        out.pieces.push(p);
    }
    for c in &s.children {
        match c {
            HtmlDomNode::SvgNode(svg) => out.pieces.extend(paths(svg, viewport, clip, cx)?),
            HtmlDomNode::DomSpan(p) => {
                let Some((left, w, right)) = piece_of(p) else {
                    continue;
                };
                let pcx = inside(p, cx);
                let pw = w * width;
                let x0 = if right { width - pw } else { left * width };
                let ph = css::get(&p.style, CssProperty::Height, pcx.size, None).unwrap_or(height);
                let vp = Rect::new(x0, -height, x0 + pw, -height + ph);
                let clip = clip.intersect(vp);
                for g in &p.children {
                    if let HtmlDomNode::SvgNode(svg) = g {
                        out.pieces.extend(paths(svg, vp, clip, pcx)?);
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(b) = css::get(st, CssProperty::BorderWidth, size, None).filter(|b| *b > 0.0) {
        let colour = st.get(CssProperty::BorderColor).or(cx.colour);
        let sides = [
            Rect::new(rect.x0, rect.y0, rect.x1, rect.y0 + b),
            Rect::new(rect.x0, rect.y1 - b, rect.x1, rect.y1),
            Rect::new(rect.x0, rect.y0 + b, rect.x0 + b, rect.y1 - b),
            Rect::new(rect.x1 - b, rect.y0 + b, rect.x1, rect.y1 - b),
        ];
        for r in sides {
            let mut p = rule::piece("box", r, cx);
            p.colour = colour;
            out.pieces.push(p);
        }
    }
    Ok(out)
}

/// An `<svg>` with no holder: its own `width` and `height`, at the top of
/// the line box's baseline, taking no space.
pub fn bare<'a>(svg: &'a SvgNode, cx: Ctx<'a>) -> Result<Laid<'a>, String> {
    let w = attr_len(&svg.attributes, "width", cx.size).unwrap_or(0.0);
    let h = attr_len(&svg.attributes, "height", cx.size).unwrap_or(0.0);
    let vp = Rect::new(0.0, -h, w, 0.0);
    let mut out = Laid::empty(0.0);
    out.pieces = paths(svg, vp, vp, cx)?;
    Ok(out)
}

fn attr_len(attrs: &AttrMap, name: &str, size: f64) -> Option<f64> {
    match css::parse(attrs.get(name)?)? {
        css::Len::Em(n) => Some(n * size),
        css::Len::Pct(_) => None,
    }
}

/// The `width` an svg's own `style` attribute sets, which beats katex.css.
fn style_width(attrs: &AttrMap, size: f64) -> Option<f64> {
    let style = attrs.get("style")?;
    let v = style.split(';').find_map(|d| {
        let (k, v) = d.split_once(':')?;
        (k.trim() == "width").then_some(v)
    })?;
    match css::parse(v)? {
        css::Len::Em(n) => Some(n * size),
        css::Len::Pct(_) => None,
    }
}

/// `viewBox` -> viewport, by `preserveAspectRatio`.
fn view_transform(attrs: &AttrMap, vp: Rect) -> Affine {
    let Some(vb) = attrs.get("viewBox").and_then(|v| {
        let n: Vec<f64> = v
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        (n.len() == 4 && n[2] > 0.0 && n[3] > 0.0)
            .then(|| Rect::new(n[0], n[1], n[0] + n[2], n[1] + n[3]))
    }) else {
        return Affine::translate(vp.origin().to_vec2());
    };
    let par = attrs
        .get("preserveAspectRatio")
        .map_or("xMidYMid meet", String::as_str);
    let mut words = par.split_whitespace();
    let align = words.next().unwrap_or("xMidYMid");
    let slice = words.next() == Some("slice");
    let (sx, sy) = (vp.width() / vb.width(), vp.height() / vb.height());
    if align == "none" {
        return Affine::new([sx, 0.0, 0.0, sy, vp.x0 - vb.x0 * sx, vp.y0 - vb.y0 * sy]);
    }
    let s = if slice { sx.max(sy) } else { sx.min(sy) };
    let frac = |axis: &str| {
        if align.contains(&format!("{axis}Min")) {
            0.0
        } else if align.contains(&format!("{axis}Max")) {
            1.0
        } else {
            0.5
        }
    };
    let tx = vp.x0 + frac("x") * (vp.width() - vb.width() * s) - vb.x0 * s;
    let ty = vp.y0 + frac("Y") * (vp.height() - vb.height() * s) - vb.y0 * s;
    Affine::new([s, 0.0, 0.0, s, tx, ty])
}

fn paths<'a>(
    svg: &'a SvgNode,
    vp: Rect,
    clip: Rect,
    cx: Ctx<'a>,
) -> Result<Vec<Piece<'a>>, String> {
    let mut vp = vp;
    if let Some(w) = style_width(&svg.attributes, cx.size) {
        vp.x1 = vp.x0 + w;
    }
    let clip = clip.intersect(vp);
    if clip.width() <= 0.0 || clip.height() <= 0.0 {
        return Ok(Vec::new());
    }
    let t = view_transform(&svg.attributes, vp);
    let mut out = Vec::new();
    for c in &svg.children {
        match c {
            SvgChildNode::Path(p) => {
                let data = p
                    .alternate
                    .as_deref()
                    .or_else(|| PATH_MAP.get(p.path_name.as_str()).copied());
                let Some(data) = data else {
                    return Err(format!("KaTeX drew an unknown SVG path `{}`", p.path_name));
                };
                let d = BezPath::from_svg(data).map_err(|e| {
                    format!("KaTeX's SVG path `{}` does not parse: {e}", p.path_name)
                })?;
                let label = if p.path_name.starts_with("sqrt") {
                    "sqrt"
                } else {
                    p.path_name.as_str()
                };
                out.push(Piece {
                    label: label.to_owned(),
                    groups: cx.groups,
                    rect: clip,
                    shape: Shape::Path {
                        d: clamp(&d, t, clip),
                        stroke: None,
                    },
                    colour: cx.colour,
                });
            }
            SvgChildNode::Line(l) => {
                let a = &l.attributes;
                let coord = |name: &str, along: f64, origin: f64| -> f64 {
                    match a.get(name).and_then(|v| css::parse(v)) {
                        Some(css::Len::Pct(f)) => origin + f * along,
                        Some(css::Len::Em(n)) => origin + n * cx.size,
                        None => origin,
                    }
                };
                let p0 = Point::new(
                    coord("x1", vp.width(), vp.x0),
                    coord("y1", vp.height(), vp.y0),
                );
                let p1 = Point::new(
                    coord("x2", vp.width(), vp.x0),
                    coord("y2", vp.height(), vp.y0),
                );
                let width = attr_len(a, "stroke-width", cx.size).unwrap_or(0.04 * cx.size);
                let mut d = BezPath::new();
                d.move_to(p0 - clip.origin().to_vec2());
                d.line_to(p1 - clip.origin().to_vec2());
                out.push(Piece {
                    label: "cancel".to_owned(),
                    groups: cx.groups,
                    rect: clip,
                    shape: Shape::Path {
                        d,
                        stroke: Some(width),
                    },
                    colour: cx.colour,
                });
            }
        }
    }
    Ok(out)
}

/// `d` through `t`, every point clamped into `clip`, relative to its
/// top-left corner.
fn clamp(d: &BezPath, t: Affine, clip: Rect) -> BezPath {
    let f = |p: Point| {
        let q = t * p;
        Point::new(
            q.x.clamp(clip.x0, clip.x1) - clip.x0,
            q.y.clamp(clip.y0, clip.y1) - clip.y0,
        )
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
