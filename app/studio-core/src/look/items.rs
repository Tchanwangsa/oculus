//! One laid-out tree flattened into what it shows: each node that paints
//! (or is a Math), where it lands in frame px, how opaque it is, what it
//! is called, and two hashes of what it paints, so samples can be compared
//! without rasterising.

use super::limits::BACKGROUND;
use super::walk::{Visit, walk};
use crate::layout::LNode;
use crate::paint::{fit, trim};
use crate::scene::{Align, Dim, Kind, PathSpec, Position};
use std::collections::hash_map::DefaultHasher;
use std::fmt::{self, Write};
use std::hash::{Hash, Hasher};
use vello_cpu::kurbo::{Rect, Shape};

/// One node that paints, or a Math's Box.
#[derive(Clone, Debug)]
pub(crate) struct Item {
    /// What beats and warnings call it: the Math it is a piece of, else
    /// its key, else its nearest keyed ancestor's, else `Kind:line`.
    pub entity: String,
    /// `entity` is a key, not `Kind:line`.
    pub keyed: bool,
    pub key: Option<String>,
    pub kind: Kind,
    /// Its TSX line and column.
    pub loc: Option<(u32, u32)>,
    /// Frame px, transformed, cut to its ancestors' clips (so a part
    /// clipped away never counts as off-frame).
    pub bbox: Rect,
    pub opacity: f64,
    /// It draws something itself (a fill, border, shadow, text, path or
    /// image), rather than only holding children.
    pub paints: bool,
    /// Its colours.
    pub ink: u64,
    /// Its shape: text, path, trim, reveal, image.
    pub form: u64,
    /// A Math's Box: how many pieces it has, hidden ones included.
    pub pieces: Option<usize>,
    /// A Math's piece: the index of its Math's item.
    pub math: Option<usize>,
    /// Under a camera: a frame-filling ancestor that zooms, which crops
    /// on purpose.
    pub framed: bool,
}

#[derive(Clone, Default)]
struct Ctx {
    anchor: Option<String>,
    math: Option<usize>,
    framed: bool,
}

/// Every item of `layers`, parents before children.
pub(crate) fn items(layers: &[LNode]) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    let frame = layers.first().map_or(0.0, |l| l.rect.area());
    walk(layers, &Ctx::default(), &mut |v, ctx| {
        let n = v.node;
        let math_box = is_math(n);
        let (entity, keyed) = match (ctx.math, &n.key, &ctx.anchor) {
            (Some(m), _, _) => (out[m].entity.clone(), out[m].keyed),
            (None, Some(k), _) => (k.clone(), true),
            (None, None, Some(a)) => (a.clone(), true),
            (None, None, None) => (unkeyed(n), false),
        };
        let paints = paints(n);
        let index = out.len();
        if paints || math_box {
            let whole = inked(v);
            let bbox = v.clip.map_or(whole, |c| whole.intersect(c));
            out.push(Item {
                entity,
                keyed,
                key: n.key.clone(),
                kind: n.kind,
                loc: n.loc,
                bbox,
                opacity: v.opacity,
                paints,
                ink: ink(n),
                form: form(n),
                pieces: math_box.then(|| n.children.len().saturating_sub(1)),
                math: ctx.math,
                framed: ctx.framed,
            });
        }
        let camera = n.style.scale.is_some_and(|z| z != 1.0) && n.rect.area() >= BACKGROUND * frame;
        Some(Ctx {
            anchor: n.key.clone().or_else(|| ctx.anchor.clone()),
            math: if math_box { Some(index) } else { ctx.math },
            framed: ctx.framed || camera,
        })
    });
    out
}

/// What a node inks, in frame px: its border box for a fill, border,
/// shadow, text or image, and a path's own outline (as trimmed, stroke
/// included), not the box it is fitted into, since a full-frame path box
/// is common.
fn inked(v: &Visit<'_>) -> Rect {
    let n = v.node;
    let s = &n.style;
    let boxed = s.fill.is_some()
        || s.shadow.is_some()
        || s.border.as_ref().is_some_and(|b| b.width > 0.0)
        || n.image.is_some()
        || n.text.is_some();
    let own = boxed.then(|| v.bounds());
    let path = n
        .path
        .as_deref()
        .and_then(|p| outline(p, n.content))
        .map(|r| v.transform.transform_rect_bbox(r));
    match (own, path) {
        (Some(a), Some(b)) => a.union(b),
        (a, b) => a.or(b).unwrap_or(Rect::ZERO),
    }
}

/// A path's painted outline in layout px: the whole path when filled, its
/// trimmed pieces when stroked, widened by half the stroke.
fn outline(p: &PathSpec, content: Rect) -> Option<Rect> {
    let union = |a: Option<Rect>, b: Rect| Some(a.map_or(b, |a| a.union(b)));
    let whole = (!p.d.elements().is_empty()).then(|| p.d.bounding_box());
    let mut out = if p.fill.is_some() { whole } else { None };
    if let Some(stroke) = p.stroke.as_ref().filter(|s| s.width > 0.0) {
        let (start, end) = p.trim.unwrap_or((0.0, 1.0));
        let drawn = if start <= 0.0 && end >= 1.0 {
            whole
        } else {
            trim(&p.d, start, end)
                .iter()
                .filter(|piece| !piece.path.elements().is_empty())
                .map(|piece| piece.path.bounding_box())
                .reduce(|a, b| a.union(b))
        };
        if let Some(d) = drawn {
            let half = stroke.width / 2.0;
            out = union(out, d.inflate(half, half));
        }
    }
    out.map(|r| fit(p.view_box, content).transform_rect_bbox(r))
}

/// `Box:12`, after the node's TSX line, for a node with no key near it.
fn unkeyed(n: &LNode) -> String {
    match n.loc {
        Some((line, _)) => format!("{}:{line}", n.kind.name()),
        None => n.kind.name().to_owned(),
    }
}

/// The Box a `<Math>` became (`math::build`): one empty, unkeyed,
/// zero-width strut and absolutely placed pieces. A magic move may put a
/// leaving piece before the strut and keeps each piece's own TSX line, so
/// neither order nor line is checked.
fn is_math(n: &LNode) -> bool {
    let strut = |c: &LNode| {
        let s = &c.style;
        c.kind == Kind::Box
            && c.children.is_empty()
            && c.key.is_none()
            && s.width == Some(Dim::Px(0.0))
            && s.align_self == Some(Align::FlexStart)
            && s.flex_shrink == Some(0.0)
            && s.position != Some(Position::Absolute)
    };
    n.kind == Kind::Box
        && n.children.iter().filter(|c| strut(c)).count() == 1
        && n.children
            .iter()
            .all(|c| strut(c) || c.style.position == Some(Position::Absolute))
}

fn paints(n: &LNode) -> bool {
    let s = &n.style;
    s.fill.is_some()
        || s.shadow.is_some()
        || s.border.as_ref().is_some_and(|b| b.width > 0.0)
        || n.image.is_some()
        || n.path.is_some()
        || n.text
            .as_ref()
            .is_some_and(|(k, _)| !k.text().trim().is_empty())
}

fn ink(n: &LNode) -> u64 {
    let s = &n.style;
    let mut h = Hashing(DefaultHasher::new());
    let _ = write!(h, "{:?}{:?}{:?}{:?}", s.fill, s.border, s.shadow, s.color);
    if let Some(p) = &n.path {
        let _ = write!(h, "{:?}{:?}", p.fill, p.stroke);
    }
    h.0.finish()
}

fn form(n: &LNode) -> u64 {
    let s = &n.style;
    let mut h = Hashing(DefaultHasher::new());
    let _ = write!(h, "{:?}{:?}", s.reveal, s.blur);
    if let Some((key, _)) = &n.text {
        key.text().hash(&mut h.0);
    }
    if let Some(p) = &n.path {
        let b = p.d.bounding_box();
        let r = |v: f64| (v * 10.0).round() as i64;
        let _ = write!(
            h,
            "{} {} {} {} {} {:?}",
            p.d.elements().len(),
            r(b.x0),
            r(b.y0),
            r(b.x1),
            r(b.y1),
            p.trim.map(|(a, b)| (r(a * 100.0), r(b * 100.0)))
        );
    }
    if let Some(img) = &n.image {
        (img.width(), img.height()).hash(&mut h.0);
    }
    h.0.finish()
}

/// Formats straight into a hasher.
struct Hashing(DefaultHasher);

impl Write for Hashing {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}
