//! Laid-out layers walked as paint composes them (`paint::paint`): each
//! node's full transform, its effective opacity and the clips of its
//! ancestors, without drawing anything. A node paint skips (opacity 0, or
//! a `clipInset` that leaves nothing) is skipped with its subtree.

use crate::layout::LNode;
use crate::paint::{inset_rect, local_transform};
use vello_cpu::kurbo::{Affine, Rect};

/// One node as paint reaches it.
pub(crate) struct Visit<'a> {
    pub node: &'a LNode,
    /// `#key`, else the path of child indices from the nearest keyed
    /// ancestor, or from the layer (`/0/3`).
    pub id: &'a str,
    /// Layout px to frame px, the node's own transform included.
    pub transform: Affine,
    /// Its opacity times every ancestor's.
    pub opacity: f64,
    /// What the ancestors' `clip` and `clipInset` (and its own
    /// `clipInset`) leave visible, as a frame-px box; `None` for no clip.
    pub clip: Option<Rect>,
}

impl Visit<'_> {
    /// The border box in frame px, before clipping.
    pub fn bounds(&self) -> Rect {
        self.transform.transform_rect_bbox(self.node.rect)
    }
}

/// Calls `f` on every node of `layers` in paint order, parents first. `f`
/// gets its parent's context and returns its children's, or `None` to skip
/// them.
pub(crate) fn walk<C>(layers: &[LNode], root: &C, f: &mut impl FnMut(&Visit<'_>, &C) -> Option<C>) {
    for (i, layer) in layers.iter().enumerate() {
        let at = Place {
            parent: Affine::IDENTITY,
            opacity: 1.0,
            clip: None,
        };
        node(layer, at, format!("/{i}"), root, f);
    }
}

#[derive(Clone, Copy)]
struct Place {
    parent: Affine,
    opacity: f64,
    clip: Option<Rect>,
}

fn node<C>(
    n: &LNode,
    at: Place,
    path: String,
    ctx: &C,
    f: &mut impl FnMut(&Visit<'_>, &C) -> Option<C>,
) {
    let s = &n.style;
    let own = f64::from(s.opacity());
    if own <= 0.0 {
        return;
    }
    let t = at.parent * local_transform(n);
    let mut clip = at.clip;
    if let Some(inset) = s.clip_inset {
        let Some(r) = inset_rect(n.rect, inset) else {
            return;
        };
        clip = meet(clip, t.transform_rect_bbox(r));
    }
    let id = match &n.key {
        Some(k) => format!("#{k}"),
        None => path,
    };
    let visit = Visit {
        node: n,
        id: &id,
        transform: t,
        opacity: at.opacity * own,
        clip,
    };
    let Some(inner) = f(&visit, ctx) else {
        return;
    };
    let clip = if s.clip == Some(true) {
        meet(clip, t.transform_rect_bbox(n.rect))
    } else {
        clip
    };
    let at = Place {
        parent: t,
        opacity: visit.opacity,
        clip,
    };
    for (i, c) in n.children.iter().enumerate() {
        node(c, at, format!("{id}/{i}"), &inner, f);
    }
}

/// Two clips together.
fn meet(a: Option<Rect>, b: Rect) -> Option<Rect> {
    Some(match a {
        Some(a) => a.intersect(b),
        None => b,
    })
}
