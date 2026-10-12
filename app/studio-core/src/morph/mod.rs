//! Magic move: two laid-out trees (A at p = 0, B at p = 1) and `p` in, one
//! laid-out tree out, which `paint` draws like any other. Used between
//! scenes (the `magic` transition) and inside one (`<Morph>`).
//!
//! Matched pairs (`matching`) blend per property (`blend`, `content`):
//! rects, so children ride along, and paint styles, colours in OKLab. Each
//! node runs on its own clock (`clock`): with `lag`, pieces start one after
//! another in reading order. Unmatched nodes fade (`fade`) at their own
//! tree's place, carried by their nearest matched ancestor's move and, with
//! the overlap fade, by their nearest matched sibling's. The output follows
//! B's paint order, each unmatched A node just before its nearest following
//! matched sibling (else last).

mod blend;
mod clock;
mod colour;
mod content;
mod fade;
mod flat;
mod matching;

pub use colour::MorphColor;
pub use flat::duplicate_key;

use crate::layout::{LNode, Typeset};
use crate::scene::{Color, Fade, Placed, Timing};
use flat::Tree;
use matching::{Matching, match_trees};
use vello_cpu::kurbo::Vec2;

/// A tree the morph cannot match: which tree, the message (with the node's
/// path) and the node's TSX position.
#[derive(Debug)]
pub struct MorphError {
    pub in_a: bool,
    pub message: String,
    pub loc: Option<(u32, u32)>,
}

/// How colours blend: the theme's default text colour (for `Text` without
/// one) and the `morphColor` token.
#[derive(Clone, Copy, Debug)]
pub struct Ink {
    pub text: Color,
    pub colour: MorphColor,
}

/// Whole trees at one `p` and timing (a magic-move transition); Morph
/// scopes in them are ignored.
pub fn morph(
    a: LNode,
    b: LNode,
    p: f32,
    timing: &Timing,
    ts: &mut Typeset,
    ink: Ink,
) -> Result<LNode, MorphError> {
    run(
        Tree::new(a, false, (p, timing.clone()), true),
        Tree::new(b, false, (p, timing.clone()), false),
        ts,
        ink,
    )
}

/// The two layouts of a scene with `<Morph>`: each Morph's subtree blends at
/// its own `p` and matches by key inside it; nodes outside every Morph
/// match by tree path and move at the first Morph's `p`.
/// Outside every Morph there is no lag: those nodes move together at the
/// first Morph's eased `p`.
pub fn morph_scene(a: LNode, b: LNode, ts: &mut Typeset, ink: Ink) -> Result<LNode, MorphError> {
    let (p, timing) = first_scope(&a).unwrap_or_default();
    let outside = (p, Timing { lag: 0.0, ..timing });
    run(
        Tree::new(a, true, outside.clone(), true),
        Tree::new(b, true, outside, false),
        ts,
        ink,
    )
}

fn first_scope(n: &LNode) -> Option<(f32, Timing)> {
    n.scope
        .as_ref()
        .map(|s| (s.p, s.timing.clone()))
        .or_else(|| n.children.iter().find_map(first_scope))
}

fn run(a: Tree, b: Tree, ts: &mut Typeset, ink: Ink) -> Result<LNode, MorphError> {
    let m = match match_trees(&a, &b) {
        Ok(m) => m,
        Err(e) => {
            reclaim(a, ts);
            reclaim(b, ts);
            return Err(e);
        }
    };
    let deep_a = matched_below(&a, &m.a_to_b);
    let deep_b = matched_below(&b, &m.b_to_a);
    let (pa, pb) = clock::clocks(&a, &b, &m);
    let mut builder = Builder {
        a,
        b,
        m,
        deep_a,
        deep_b,
        pa,
        pb,
        ts,
        ink,
    };
    let out = builder.emit_b(0, Vec2::ZERO);
    let Builder { a, b, ts, .. } = builder;
    reclaim(a, ts);
    reclaim(b, ts);
    Ok(out.unwrap_or_else(empty))
}

/// For each node, whether a matched node is somewhere below it.
fn matched_below(t: &Tree, matched: &[Option<usize>]) -> Vec<bool> {
    let mut below = vec![false; t.slots.len()];
    for i in (0..t.slots.len()).rev() {
        below[i] = t.slots[i]
            .kids
            .iter()
            .any(|&k| matched[k].is_some() || below[k]);
    }
    below
}

/// Anything the builder left (nothing, normally) returns to the pool.
fn reclaim(t: Tree, ts: &mut Typeset) {
    for s in t.slots {
        if let Some(n) = s.node {
            ts.reclaim(n);
        }
    }
}

fn empty() -> LNode {
    LNode {
        kind: crate::scene::Kind::Box,
        style: Default::default(),
        rect: Default::default(),
        content: Default::default(),
        text: None,
        image: None,
        path: None,
        children: Vec::new(),
        key: None,
        loc: None,
        scope: None,
        overlay: None,
    }
}

struct Builder<'a> {
    a: Tree,
    b: Tree,
    m: Matching,
    deep_a: Vec<bool>,
    deep_b: Vec<bool>,
    /// Each slot's eased progress (`clock`).
    pa: Vec<f32>,
    pb: Vec<f32>,
    ts: &'a mut Typeset,
    ink: Ink,
}

impl Builder<'_> {
    /// B node `ib` and what goes under it; `shift` is how far its nearest
    /// matched ancestor has moved from B's place.
    fn emit_b(&mut self, ib: usize, shift: Vec2) -> Option<LNode> {
        let Some(ia) = self.m.b_to_a[ib] else {
            return self.unmatched_b(ib, shift);
        };
        let p = self.pb[ib];
        let (mut na, mut nb) = (self.a.take(ia)?, self.b.take(ib)?);
        let mut out = LNode {
            kind: nb.kind,
            style: blend::style(&na.style, &nb.style, p, self.ink.text, self.ink.colour),
            rect: blend::rect(na.rect, nb.rect, p),
            content: blend::rect(na.content, nb.content, p),
            text: None,
            image: None,
            path: None,
            children: Vec::new(),
            key: nb.key.take(),
            loc: nb.loc,
            scope: None,
            overlay: match (na.overlay.take(), nb.overlay.take()) {
                (Some(x), Some(y)) => Some(Box::new(Placed {
                    now: y.now,
                    was: Some((x.now, p)),
                })),
                (_, y) => y,
            },
        };
        let q = p.clamp(0.0, 1.0);
        content::blend(&mut out, &mut na, &mut nb, q, self.ts, self.ink.colour);
        let shift_a = out.rect.origin() - na.rect.origin();
        let shift_b = out.rect.origin() - nb.rect.origin();
        // Each unmatched A child goes before the B partner of its nearest
        // following matched sibling, when that partner is a child here.
        let kids_a = self.a.slots[ia].kids.clone();
        let mut before: Vec<(Option<usize>, usize)> = Vec::new();
        for (i, &ka) in kids_a.iter().enumerate() {
            if self.m.a_to_b[ka].is_some() {
                continue;
            }
            let anchor = kids_a[i + 1..].iter().find_map(|&next| {
                let bj = self.m.a_to_b[next]?;
                (self.b.slots[bj].parent == Some(ib)).then_some(bj)
            });
            before.push((anchor, ka));
        }
        for kb in self.b.slots[ib].kids.clone() {
            for &(_, ka) in before.iter().filter(|(anchor, _)| *anchor == Some(kb)) {
                let ride = self.ride_a(ia, ib, ka);
                out.children.extend(self.unmatched_a(ka, shift_a + ride));
            }
            let ride = self.ride_b(ia, ib, kb);
            out.children.extend(self.emit_b(kb, shift_b + ride));
        }
        for &(_, ka) in before.iter().filter(|(anchor, _)| anchor.is_none()) {
            let ride = self.ride_a(ia, ib, ka);
            out.children.extend(self.unmatched_a(ka, shift_a + ride));
        }
        Some(out)
    }

    /// How far matched B node `sb` is from its B place now, less how far
    /// its parent pair (ia, ib) is from its own (which `shift` already
    /// carries): both are their move from A times what is left of it.
    fn behind(&self, ia: usize, ib: usize, sb: usize) -> Option<Vec2> {
        let sa = self.m.b_to_a[sb]?;
        let own = self.b.slots[sb].rect.origin() - self.a.slots[sa].rect.origin();
        let parent = self.b.slots[ib].rect.origin() - self.a.slots[ia].rect.origin();
        let left = |q: f32| f64::from(1.0 - q);
        Some(parent * left(self.pb[ib]) - own * left(self.pb[sb]))
    }

    /// With the overlap fade, an unmatched piece (a leaf: a glyph, a text)
    /// among the children of the pair (ia, ib) keeps its place beside its
    /// nearest matched sibling (the one before it, else after) and moves
    /// with it: an arriving one sits where that sibling's B place would put
    /// it, a leaving one where its A place would. So new terms slide in
    /// with their neighbours instead of waiting at their end places over
    /// the moving ones; boxes with children fade in place.
    fn ride_b(&self, ia: usize, ib: usize, kb: usize) -> Vec2 {
        let piece = self.b.slots[kb].leaf;
        if self.m.b_to_a[kb].is_some() || !piece || !self.overlap(&self.b, kb) {
            return Vec2::ZERO;
        }
        let kids = &self.b.slots[ib].kids;
        let found = |k: usize| visible(&self.b, k).then(|| self.behind(ia, ib, k))?;
        nearest(kids, kb, found).unwrap_or(Vec2::ZERO)
    }

    fn ride_a(&self, ia: usize, ib: usize, ka: usize) -> Vec2 {
        if !self.a.slots[ka].leaf || !self.overlap(&self.a, ka) {
            return Vec2::ZERO;
        }
        let kids = &self.a.slots[ia].kids;
        let ahead = |k: usize| {
            let sb = self.m.a_to_b[k].filter(|_| visible(&self.a, k))?;
            let own = self.b.slots[sb].rect.origin() - self.a.slots[k].rect.origin();
            let parent = self.b.slots[ib].rect.origin() - self.a.slots[ia].rect.origin();
            Some(own * f64::from(self.pb[sb]) - parent * f64::from(self.pb[ib]))
        };
        nearest(kids, ka, ahead).unwrap_or(Vec2::ZERO)
    }

    fn overlap(&self, t: &Tree, i: usize) -> bool {
        t.clock(i).1.fade == Fade::Overlap
    }

    fn unmatched_b(&mut self, ib: usize, shift: Vec2) -> Option<LNode> {
        let k = fade::arriving(self.b.clock(ib).1.fade, self.pb[ib]);
        if !self.deep_b[ib] {
            let mut n = copy(&mut self.b, ib, shift)?;
            blend::fade(&mut n.style, k);
            return Some(n);
        }
        let mut n = self.b.take(ib)?;
        shift_node(&mut n, shift);
        blend::fade_own(&mut n.style, k);
        for kb in self.b.slots[ib].kids.clone() {
            n.children.extend(self.emit_b(kb, shift));
        }
        Some(n)
    }

    /// An unmatched A node; matched nodes below it are drawn from B's side.
    fn unmatched_a(&mut self, ia: usize, shift: Vec2) -> Option<LNode> {
        let k = fade::leaving(self.a.clock(ia).1.fade, self.pa[ia]);
        if !self.deep_a[ia] {
            let mut n = copy(&mut self.a, ia, shift)?;
            blend::fade(&mut n.style, k);
            return Some(n);
        }
        let mut n = self.a.take(ia)?;
        shift_node(&mut n, shift);
        blend::fade_own(&mut n.style, k);
        for ka in self.a.slots[ia].kids.clone() {
            if self.m.a_to_b[ka].is_none() {
                n.children.extend(self.unmatched_a(ka, shift));
            }
        }
        Some(n)
    }
}

/// Whether a node draws something (an empty strut does not, so nothing
/// rides with it).
fn visible(t: &Tree, i: usize) -> bool {
    t.slots[i].leaf || !t.slots[i].kids.is_empty()
}

/// The first `found` among `kids` before `at`, nearest first, else after.
fn nearest<T>(kids: &[usize], at: usize, found: impl Fn(usize) -> Option<T>) -> Option<T> {
    let i = kids.iter().position(|&k| k == at)?;
    kids[..i]
        .iter()
        .rev()
        .chain(&kids[i + 1..])
        .find_map(|&k| found(k))
}

/// A whole subtree, moved by `shift`.
fn copy(t: &mut Tree, i: usize, shift: Vec2) -> Option<LNode> {
    let mut n = t.take(i)?;
    shift_node(&mut n, shift);
    for k in t.slots[i].kids.clone() {
        n.children.extend(copy(t, k, shift));
    }
    Some(n)
}

fn shift_node(n: &mut LNode, d: Vec2) {
    n.scope = None;
    if d != Vec2::ZERO {
        n.rect = n.rect + d;
        n.content = n.content + d;
    }
}

#[cfg(test)]
mod tests;
