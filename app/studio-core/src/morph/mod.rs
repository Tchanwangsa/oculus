//! Magic move: two laid-out trees (A at p = 0, B at p = 1) and `p` in, one
//! laid-out tree out, which `paint` draws like any other. Used between
//! scenes (the `magic` transition) and inside one (`<Morph>`).
//!
//! Matched pairs (`matching`) blend per property (`blend`, `content`):
//! rects, so children ride along, and paint styles, colours in OKLab.
//! Unmatched A nodes fade out over p 0..0.4 and unmatched B nodes in over
//! 0.4..1, each at its own tree's place, carried by its nearest matched
//! ancestor's move. The output follows B's paint order, each unmatched A
//! node just before its nearest following matched sibling (else last).

mod blend;
mod colour;
mod content;
mod flat;
mod matching;

pub use colour::MorphColor;
pub use flat::duplicate_key;

use crate::easing::in_out_cubic;
use crate::layout::{LNode, Typeset};
use crate::scene::Color;
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

/// Whole trees at one `p` (a magic-move transition); Morph scopes in them
/// are ignored.
pub fn morph(a: LNode, b: LNode, p: f32, ts: &mut Typeset, ink: Ink) -> Result<LNode, MorphError> {
    run(
        Tree::new(a, false, p, true),
        Tree::new(b, false, p, false),
        ts,
        ink,
    )
}

/// The two layouts of a scene with `<Morph>`: each Morph's subtree blends at
/// its own `p` and matches by key inside it; nodes outside every Morph
/// match by tree path and move at the first Morph's `p`.
pub fn morph_scene(a: LNode, b: LNode, ts: &mut Typeset, ink: Ink) -> Result<LNode, MorphError> {
    let outside = first_scope_p(&a).unwrap_or(0.0);
    run(
        Tree::new(a, true, outside, true),
        Tree::new(b, true, outside, false),
        ts,
        ink,
    )
}

fn first_scope_p(n: &LNode) -> Option<f32> {
    n.scope
        .map(|s| s.p)
        .or_else(|| n.children.iter().find_map(first_scope_p))
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
    let mut builder = Builder {
        a,
        b,
        m,
        deep_a,
        deep_b,
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
    }
}

struct Builder<'a> {
    a: Tree,
    b: Tree,
    m: Matching,
    deep_a: Vec<bool>,
    deep_b: Vec<bool>,
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
        let p = self.b.slots[ib].p;
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
        };
        content::blend(&mut out, &mut na, &mut nb, p, self.ts, self.ink.colour);
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
                out.children.extend(self.unmatched_a(ka, shift_a));
            }
            out.children.extend(self.emit_b(kb, shift_b));
        }
        for &(_, ka) in before.iter().filter(|(anchor, _)| anchor.is_none()) {
            out.children.extend(self.unmatched_a(ka, shift_a));
        }
        Some(out)
    }

    fn unmatched_b(&mut self, ib: usize, shift: Vec2) -> Option<LNode> {
        let p = self.b.slots[ib].p;
        let k = in_out_cubic(((p - 0.4) / 0.6).clamp(0.0, 1.0));
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
        let p = self.a.slots[ia].p;
        let k = 1.0 - in_out_cubic((p / 0.4).clamp(0.0, 1.0));
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
