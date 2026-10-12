//! Every keyed node's box in frame space (its rect through every transform
//! above and on it), how visible it is, and the size of the maths it holds,
//! so an overlay can find its target's box and scale its defaults to the
//! formula. A target's box is the union of the outermost nodes it covers,
//! counting a fading node by its opacity, so the box grows and shrinks
//! smoothly as a morph fades terms in and out (or `Write` writes them).

use super::OverlayError;
use crate::layout::LNode;
use crate::paint::local_transform;
use crate::scene::Kind;
use crate::suggest::closest;
use vello_cpu::kurbo::{Affine, Rect};

struct Entry {
    key: String,
    rect: Rect,
    /// Its opacity times its ancestors'.
    alpha: f64,
    /// A covered ancestor's entry, if any: only the outermost count.
    outer: Option<usize>,
    /// Px per em of the maths or text it is, in frame space.
    em: Option<f64>,
}

pub struct Index {
    entries: Vec<Entry>,
}

/// A target's box and em, in frame space, and how visible its most
/// visible part is (an overlay on terms that have faded out fades too).
#[derive(Clone, Copy, Debug)]
pub struct Found {
    pub rect: Rect,
    pub em: Option<f64>,
    pub alpha: f64,
}

/// `key` is `target` or below it: `eq/lhs` covers `eq/lhs`, `eq/lhs.1` and
/// `eq/lhs/x`, but not `eq/lhs2` or `eq/x#2` for `eq/x`.
pub fn covers(target: &str, key: &str) -> bool {
    key.strip_prefix(target)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['.', '/']))
}

/// The uniform scale of `t` (its area factor's square root).
pub fn scale_of(t: Affine) -> f64 {
    let [a, b, c, d, _, _] = t.as_coeffs();
    (a * d - b * c).abs().sqrt()
}

impl Index {
    pub fn new(root: &LNode) -> Index {
        let mut entries = Vec::new();
        walk(root, Affine::IDENTITY, 1.0, None, &mut entries);
        Index { entries }
    }

    pub fn find(&self, target: &str) -> Option<Found> {
        let all = || self.entries.iter().filter(|e| covers(target, &e.key));
        let em = all().filter_map(|e| e.em).reduce(f64::max);
        let covered: Vec<&Entry> = all()
            .filter(|e| {
                // Only the outermost: a covered ancestor stands for them.
                let mut up = e.outer;
                while let Some(i) = up {
                    if covers(target, &self.entries[i].key) {
                        return false;
                    }
                    up = self.entries[i].outer;
                }
                true
            })
            .collect();
        if covered.is_empty() {
            return None;
        }
        let union = |it: &mut dyn Iterator<Item = &&Entry>| -> Option<Rect> {
            it.map(|e| e.rect).reduce(|a, b| a.union(b))
        };
        let opaque = union(&mut covered.iter().filter(|e| e.alpha >= 0.999));
        let mut rect = opaque.or_else(|| union(&mut covered.iter().filter(|e| e.alpha > 0.0)));
        let rect_all = union(&mut covered.iter());
        if opaque.is_some() {
            for e in covered.iter().filter(|e| e.alpha < 0.999) {
                rect = rect.map(|r| lerp_rect(r, r.union(e.rect), e.alpha));
            }
        }
        let alpha = covered.iter().map(|e| e.alpha).fold(0.0, f64::max);
        Some(Found {
            rect: rect.or(rect_all)?,
            em,
            alpha,
        })
    }

    /// `target` matched no key: the error names the closest key.
    pub fn missing(&self, what: &str, target: &str, loc: Option<(u32, u32)>) -> OverlayError {
        let keys = self.entries.iter().map(|e| e.key.as_str());
        let hint = match closest(target, keys) {
            Some(k) => format!("; did you mean `{k}`?"),
            None => String::new(),
        };
        OverlayError {
            message: format!(
                "<{what}> target `{target}` matches no keyed node in this frame{hint} A target is a key or a key prefix (`eq/lhs` covers `eq/lhs.1`, …); a Math's pieces are keyed `<its key>/<term>`"
            ),
            loc,
        }
    }
}

fn lerp_rect(a: Rect, b: Rect, k: f64) -> Rect {
    let l = |x: f64, y: f64| x + (y - x) * k;
    Rect::new(l(a.x0, b.x0), l(a.y0, b.y0), l(a.x1, b.x1), l(a.y1, b.y1))
}

/// `outer` is the nearest keyed ancestor's entry.
fn walk(n: &LNode, parent: Affine, alpha: f64, outer: Option<usize>, out: &mut Vec<Entry>) {
    let t = parent * local_transform(n);
    let alpha = alpha * f64::from(n.style.opacity().clamp(0.0, 1.0));
    let mut outer = outer;
    if let Some(key) = &n.key {
        out.push(Entry {
            key: key.clone(),
            rect: t.transform_rect_bbox(n.rect),
            alpha,
            outer,
            em: em(n).map(|e| e * scale_of(t)),
        });
        outer = Some(out.len() - 1);
    }
    for c in &n.children {
        walk(c, t, alpha, outer, out);
    }
}

/// A Math piece's em (its rect over its viewBox, both in em of the
/// formula's size) or a Text's font size.
fn em(n: &LNode) -> Option<f64> {
    match n.kind {
        Kind::Path if n.key.as_deref().is_some_and(|k| k.contains('/')) => {
            let p = n.path.as_ref()?;
            (p.view_box.height() > 0.0).then(|| n.rect.height() / p.view_box.height())
        }
        Kind::Text => n.style.font_size.map(f64::from),
        _ => None,
    }
}
