//! A laid-out tree flattened into preorder slots, so matching can refer to
//! nodes by index and the builder can move each node out exactly once.

use super::MorphError;
use crate::layout::LNode;
use crate::scene::{Kind, Timing};
use std::collections::HashMap;
use vello_cpu::kurbo::Rect;

pub struct Slot {
    /// The node without its children (they are slots of their own), until
    /// the builder takes it.
    pub node: Option<LNode>,
    pub kind: Kind,
    pub key: Option<String>,
    pub loc: Option<(u32, u32)>,
    pub parent: Option<usize>,
    /// Index among its siblings.
    pub index: usize,
    pub kids: Vec<usize>,
    /// The Morph this node is inside (nearest scope root, itself included).
    pub scope: Option<u32>,
    pub scope_root: bool,
    /// It has no children and paints something, so it takes a number in
    /// its scope's reading order (`clock`).
    pub leaf: bool,
    /// Its border box, kept after the builder takes the node.
    pub rect: Rect,
    /// The product of its ancestors' opacities in this tree.
    pub above: f32,
}

pub struct Tree {
    pub slots: Vec<Slot>,
    /// Which tree this is, for error messages.
    pub is_a: bool,
    /// Each Morph's `p` and timing by scope id; `None` is outside every
    /// Morph (or the whole tree, for a transition).
    pub clocks: HashMap<Option<u32>, (f32, Timing)>,
}

impl Tree {
    /// With `scopes` off (a transition), Morph scopes are ignored and every
    /// node takes `outside`.
    pub fn new(root: LNode, scopes: bool, outside: (f32, Timing), is_a: bool) -> Tree {
        let mut t = Tree {
            slots: Vec::new(),
            is_a,
            clocks: HashMap::from([(None, outside)]),
        };
        t.push(root, None, 0, None, scopes, 1.0);
        t
    }

    fn push(
        &mut self,
        mut n: LNode,
        parent: Option<usize>,
        index: usize,
        scope: Option<u32>,
        scopes: bool,
        above: f32,
    ) -> usize {
        let own = n.scope.take().filter(|_| scopes);
        let scope_root = own.is_some();
        let scope = match own {
            Some(s) => {
                self.clocks.insert(Some(s.id), (s.p, s.timing));
                Some(s.id)
            }
            None => scope,
        };
        let kids = std::mem::take(&mut n.children);
        let below = above * n.style.opacity();
        let i = self.slots.len();
        self.slots.push(Slot {
            kind: n.kind,
            key: n.key.clone(),
            loc: n.loc,
            leaf: kids.is_empty() && paints(&n),
            rect: n.rect,
            above,
            node: Some(n),
            parent,
            index,
            kids: Vec::with_capacity(kids.len()),
            scope,
            scope_root,
        });
        for (k, c) in kids.into_iter().enumerate() {
            let ci = self.push(c, Some(i), k, scope, scopes, below);
            self.slots[i].kids.push(ci);
        }
        i
    }

    /// The `p` and timing of the scope node `i` is in.
    pub fn clock(&self, i: usize) -> &(f32, Timing) {
        let s = self.slots[i].scope;
        self.clocks.get(&s).unwrap_or(&self.clocks[&None])
    }

    /// Each slot is taken once; the builder visits every node once.
    pub fn take(&mut self, i: usize) -> Option<LNode> {
        self.slots[i].node.take()
    }

    pub fn key(&self, i: usize) -> Option<&str> {
        self.slots[i].key.as_deref()
    }

    /// `Box > Row[1] > Box[0]`, as element errors name nodes.
    pub fn path(&self, i: usize) -> String {
        let name = self.slots[i].kind.name();
        match self.slots[i].parent {
            None => name.to_owned(),
            Some(p) => format!("{} > {name}[{}]", self.path(p), self.slots[i].index),
        }
    }

    /// Every key's node; a key on two nodes is an error naming both.
    pub fn keys(&self) -> Result<HashMap<&str, usize>, MorphError> {
        let mut keys = HashMap::new();
        for i in 0..self.slots.len() {
            let Some(k) = self.key(i) else { continue };
            if let Some(first) = keys.insert(k, i) {
                return Err(duplicate(
                    k,
                    &self.path(first),
                    &self.path(i),
                    self.slots[i].loc,
                    self.is_a,
                ));
            }
        }
        Ok(keys)
    }
}

/// Whether a childless node draws anything.
fn paints(n: &LNode) -> bool {
    let s = &n.style;
    n.text.is_some()
        || n.image.is_some()
        || n.path.is_some()
        || s.fill.is_some()
        || s.border.is_some()
        || s.shadow.is_some()
}

fn duplicate(
    key: &str,
    first: &str,
    second: &str,
    loc: Option<(u32, u32)>,
    in_a: bool,
) -> MorphError {
    MorphError {
        in_a,
        message: format!(
            "{second}: key `{key}` is also on {first}; keys must be unique in a scene's tree (not just among siblings) so magic move can match them"
        ),
        loc,
    }
}

/// The first key that is on two nodes of a laid-out tree, as lint reports
/// it.
pub fn duplicate_key(root: &LNode) -> Option<MorphError> {
    fn walk<'a>(
        n: &'a LNode,
        path: String,
        seen: &mut HashMap<&'a str, String>,
    ) -> Option<MorphError> {
        if let Some(k) = n.key.as_deref() {
            if let Some(first) = seen.get(k) {
                return Some(duplicate(k, first, &path, n.loc, false));
            }
            seen.insert(k, path.clone());
        }
        for (i, c) in n.children.iter().enumerate() {
            let p = format!("{path} > {}[{i}]", c.kind.name());
            if let Some(d) = walk(c, p, seen) {
                return Some(d);
            }
        }
        None
    }
    walk(root, root.kind.name().to_owned(), &mut HashMap::new())
}
