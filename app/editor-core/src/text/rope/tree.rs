//! The rope's persistent B-tree. Leaves are `\n`-only string chunks; every node
//! caches a `Summary` (bytes, UTF-16 units, newlines). Nodes are `Arc`'d and never
//! mutated, so an edit rebuilds only the path it touches and a clone is a pointer
//! clone. One seek by any dimension also yields the others at the leaf start.
//!
//! The height-delta join (merge or redistribute at equal height, else descend the
//! taller tree's spine) and the seek-with-bias design are adapted from Zed's
//! `sum_tree` crate (crates/sum_tree/src/{sum_tree,cursor}.rs at 129c68ff4b),
//! Copyright 2022-2025 Zed Industries, Inc., licensed under the Apache License,
//! Version 2.0 <http://www.apache.org/licenses/LICENSE-2.0>.

use std::ops::{Add, Sub};
use std::sync::Arc;

/// Node size limits, a type so tests can build the same code at a second
/// setting. `Text` uses `Release`.
pub trait Sizes: 'static {
    /// Leaves hold at most this many bytes.
    const MAX_CHUNK: usize;
    /// Internal nodes hold at most this many children.
    const MAX_CHILDREN: usize;
    /// Every leaf of a multi-leaf tree holds at least this many bytes.
    const MIN_CHUNK: usize = Self::MAX_CHUNK / 4;
    /// Every internal node except the root has at least this many children.
    const MIN_CHILDREN: usize = Self::MAX_CHILDREN / 2;
}

/// 1 KiB leaves keep a 100 KB note two levels deep while an in-leaf scan (a
/// byte loop, no bitmaps) stays a few hundred ns.
#[derive(Debug, Clone, Copy)]
pub struct Release;

impl Sizes for Release {
    const MAX_CHUNK: usize = 1024;
    const MAX_CHILDREN: usize = 16;
}

/// Tiny leaves and fan-out 6 (so a 2-child root is underfull): a few hundred
/// bytes already make a deep tree.
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub struct Small;

#[cfg(test)]
impl Sizes for Small {
    const MAX_CHUNK: usize = 32;
    const MAX_CHILDREN: usize = 6;
}

/// The measures of a run of text. `lines` counts `\n`s, not lines.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Summary {
    pub bytes: usize,
    pub utf16: usize,
    pub lines: usize,
}

impl Summary {
    pub fn of(s: &str) -> Summary {
        let mut summary = Summary {
            bytes: s.len(),
            utf16: 0,
            lines: 0,
        };
        for &b in s.as_bytes() {
            // A UTF-8 lead byte starts one unit; a 4-byte lead adds the second
            // surrogate. Continuation bytes (10xxxxxx) count nothing.
            summary.utf16 += (b & 0xC0 != 0x80) as usize + (b >= 0xF0) as usize;
            summary.lines += (b == b'\n') as usize;
        }
        summary
    }
}

impl Add for Summary {
    type Output = Summary;
    fn add(self, o: Summary) -> Summary {
        Summary {
            bytes: self.bytes + o.bytes,
            utf16: self.utf16 + o.utf16,
            lines: self.lines + o.lines,
        }
    }
}

impl Sub for Summary {
    type Output = Summary;
    fn sub(self, o: Summary) -> Summary {
        Summary {
            bytes: self.bytes - o.bytes,
            utf16: self.utf16 - o.utf16,
            lines: self.lines - o.lines,
        }
    }
}

/// The measure a seek walks by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Dim {
    Bytes,
    Utf16,
    Lines,
}

impl Dim {
    fn of(self, s: &Summary) -> usize {
        match self {
            Dim::Bytes => s.bytes,
            Dim::Utf16 => s.utf16,
            Dim::Lines => s.lines,
        }
    }
}

/// Which leaf a seek lands on when the target sits on a leaf boundary: `Left`
/// stops on the leaf ending there, `Right` on the leaf starting there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Bias {
    Left,
    Right,
}

#[derive(Debug)]
pub(super) struct Node {
    pub summary: Summary,
    pub height: u8,
    pub kind: Kind,
}

#[derive(Debug)]
pub(super) enum Kind {
    Leaf(String),
    Internal(Vec<Arc<Node>>),
}

impl Node {
    pub fn children(&self) -> &[Arc<Node>] {
        match &self.kind {
            Kind::Internal(children) => children,
            Kind::Leaf(_) => &[],
        }
    }

    pub fn leaf_text(&self) -> &str {
        match &self.kind {
            Kind::Leaf(text) => text,
            Kind::Internal(_) => unreachable!("leaf_text on an internal node"),
        }
    }
}

pub(super) fn leaf(text: String) -> Arc<Node> {
    Arc::new(Node {
        summary: Summary::of(&text),
        height: 0,
        kind: Kind::Leaf(text),
    })
}

/// A node over `children`, which must share one height.
fn internal(children: Vec<Arc<Node>>) -> Arc<Node> {
    let height = children[0].height + 1;
    let summary = children
        .iter()
        .fold(Summary::default(), |acc, c| acc + c.summary);
    Arc::new(Node {
        summary,
        height,
        kind: Kind::Internal(children),
    })
}

/// A root over `children`, dropping a single-child level.
fn root_of(mut children: Vec<Arc<Node>>) -> Arc<Node> {
    if children.len() == 1 {
        children.pop().unwrap()
    } else {
        internal(children)
    }
}

/// Splits `s` into leaf-sized pieces at char boundaries. A string longer than
/// `MAX_CHUNK` gives pieces of near-equal size, each at least `MAX_CHUNK / 2 - 6`.
fn split_chunks<S: Sizes>(s: &str) -> Vec<&str> {
    if s.len() <= S::MAX_CHUNK {
        return vec![s];
    }
    // `MAX_CHUNK - 4` leaves room for moving each cut back to a char boundary.
    let n = s.len().div_ceil(S::MAX_CHUNK - 4);
    let mut pieces = Vec::with_capacity(n);
    let mut start = 0;
    for i in 1..n {
        let mut cut = s.len() * i / n;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        pieces.push(&s[start..cut]);
        start = cut;
    }
    pieces.push(&s[start..]);
    pieces
}

/// A balanced tree over `s`, which must already be `\n`-normalised.
pub(super) fn build<S: Sizes>(s: &str) -> Arc<Node> {
    let mut level: Vec<Arc<Node>> = split_chunks::<S>(s)
        .into_iter()
        .map(|p| leaf(p.to_owned()))
        .collect();
    while level.len() > 1 {
        let groups = level.len().div_ceil(S::MAX_CHILDREN);
        let mut next = Vec::with_capacity(groups);
        let mut rest = level.into_iter();
        let total = rest.len();
        let mut taken = 0;
        for g in 1..=groups {
            let end = total * g / groups;
            next.push(internal(rest.by_ref().take(end - taken).collect()));
            taken = end;
        }
        level = next;
    }
    level.pop().unwrap()
}

/// One node if `children` fit, else two halves of at least `MIN_CHILDREN` each.
fn pack<S: Sizes>(mut children: Vec<Arc<Node>>) -> Vec<Arc<Node>> {
    if children.len() <= S::MAX_CHILDREN {
        return vec![internal(children)];
    }
    let right = children.split_off(children.len().div_ceil(2));
    vec![internal(children), internal(right)]
}

/// Concatenates two trees. Either may be an underfull root; the result is valid.
pub(super) fn join<S: Sizes>(l: &Arc<Node>, r: &Arc<Node>) -> Arc<Node> {
    if l.summary.bytes == 0 {
        return r.clone();
    }
    if r.summary.bytes == 0 {
        return l.clone();
    }
    root_of(join_rec::<S>(l, r))
}

/// One or two nodes at height `max(l.height, r.height)` holding `l` then `r`.
fn join_rec<S: Sizes>(l: &Arc<Node>, r: &Arc<Node>) -> Vec<Arc<Node>> {
    if l.height == r.height {
        if l.height == 0 {
            return vec![l.clone(), r.clone()];
        }
        let (lc, rc) = (l.children(), r.children());
        if lc.len() >= S::MIN_CHILDREN && rc.len() >= S::MIN_CHILDREN {
            return vec![l.clone(), r.clone()];
        }
        return pack::<S>(lc.iter().chain(rc).cloned().collect());
    }
    if l.height > r.height {
        let lc = l.children();
        let mut children = lc[..lc.len() - 1].to_vec();
        children.extend(join_rec::<S>(&lc[lc.len() - 1], r));
        pack::<S>(children)
    } else {
        let rc = r.children();
        let mut children = join_rec::<S>(l, &rc[0]);
        children.extend(rc[1..].iter().cloned());
        pack::<S>(children)
    }
}

/// The leaves of `node` that end at or before `byte`, which must be a leaf
/// boundary of `node` (or 0, or past its end).
pub(super) fn take_before<S: Sizes>(node: &Arc<Node>, byte: usize) -> Option<Arc<Node>> {
    if byte == 0 {
        return None;
    }
    if byte >= node.summary.bytes {
        return Some(node.clone());
    }
    let children = node.children();
    assert!(!children.is_empty(), "take_before cuts inside a leaf");
    let mut start = 0;
    for (i, child) in children.iter().enumerate() {
        let end = start + child.summary.bytes;
        if end == byte {
            return Some(root_of(children[..=i].to_vec()));
        }
        if end > byte {
            let rest = take_before::<S>(child, byte - start)?;
            return Some(if i == 0 {
                rest
            } else {
                join::<S>(&root_of(children[..i].to_vec()), &rest)
            });
        }
        start = end;
    }
    unreachable!("byte is inside the node")
}

/// The leaves of `node` that start at or after `byte`, a leaf boundary.
pub(super) fn take_after<S: Sizes>(node: &Arc<Node>, byte: usize) -> Option<Arc<Node>> {
    if byte >= node.summary.bytes {
        return None;
    }
    if byte == 0 {
        return Some(node.clone());
    }
    let children = node.children();
    assert!(!children.is_empty(), "take_after cuts inside a leaf");
    let mut start = 0;
    for (i, child) in children.iter().enumerate() {
        let end = start + child.summary.bytes;
        if start == byte {
            return Some(root_of(children[i..].to_vec()));
        }
        if end > byte {
            let rest = take_after::<S>(child, byte - start)?;
            let after = &children[i + 1..];
            return Some(if after.is_empty() {
                rest
            } else {
                join::<S>(&rest, &root_of(after.to_vec()))
            });
        }
        start = end;
    }
    unreachable!("byte is inside the node")
}

/// A position on one leaf: the root-to-leaf path and the summary before the leaf.
#[derive(Clone)]
pub(super) struct Cursor<'a> {
    stack: Vec<(&'a Node, usize)>,
    leaf: &'a Node,
    pub start: Summary,
}

impl<'a> Cursor<'a> {
    /// The leaf where `dim` reaches `target`. With `Left` that is the first leaf
    /// whose end is at or past `target`; with `Right`, the first whose end is past
    /// it. A target beyond the end lands on the last leaf.
    pub fn seek(root: &'a Node, dim: Dim, target: usize, bias: Bias) -> Cursor<'a> {
        let mut stack = Vec::with_capacity(root.height as usize);
        let mut node = root;
        let mut start = Summary::default();
        while let Kind::Internal(children) = &node.kind {
            let mut index = children.len() - 1;
            for (i, child) in children[..children.len() - 1].iter().enumerate() {
                let end = dim.of(&(start + child.summary));
                if end > target || (end == target && bias == Bias::Left) {
                    index = i;
                    break;
                }
                start = start + child.summary;
            }
            stack.push((node, index));
            node = &children[index];
        }
        Cursor {
            stack,
            leaf: node,
            start,
        }
    }

    pub fn text(&self) -> &'a str {
        self.leaf.leaf_text()
    }

    pub fn end(&self) -> Summary {
        self.start + self.leaf.summary
    }

    /// Moves to the next leaf; false (and unchanged) at the last one.
    pub fn next(&mut self) -> bool {
        let Some(depth) = self
            .stack
            .iter()
            .rposition(|(n, i)| i + 1 < n.children().len())
        else {
            return false;
        };
        self.start = self.end();
        self.stack.truncate(depth + 1);
        let entry = self.stack.last_mut().unwrap();
        entry.1 += 1;
        let node: &'a Node = entry.0;
        let mut child: &'a Node = &node.children()[entry.1];
        while let Kind::Internal(children) = &child.kind {
            self.stack.push((child, 0));
            child = &children[0];
        }
        self.leaf = child;
        true
    }

    /// Moves to the previous leaf; false (and unchanged) at the first one.
    pub fn prev(&mut self) -> bool {
        let Some(depth) = self.stack.iter().rposition(|&(_, i)| i > 0) else {
            return false;
        };
        self.stack.truncate(depth + 1);
        let entry = self.stack.last_mut().unwrap();
        entry.1 -= 1;
        let node: &'a Node = entry.0;
        let mut child: &'a Node = &node.children()[entry.1];
        while let Kind::Internal(children) = &child.kind {
            self.stack.push((child, children.len() - 1));
            child = &children[children.len() - 1];
        }
        self.leaf = child;
        self.start = self.start - child.summary;
        true
    }
}

/// Panics unless `root` keeps every invariant: one leaf depth, child counts,
/// leaf sizes, cached summaries, and no `\r` in the text.
#[cfg(test)]
pub(super) fn check<S: Sizes>(root: &Node) {
    check_shared::<S>(root, &mut std::collections::HashSet::new());
}

/// `check`, skipping non-root nodes already in `seen` and adding the ones it
/// verifies. Nodes are immutable, so this is sound while every tree checked
/// with the same `seen` stays alive (no address is reused).
#[cfg(test)]
pub(super) fn check_shared<S: Sizes>(root: &Node, seen: &mut std::collections::HashSet<usize>) {
    fn walk<S: Sizes>(
        node: &Node,
        is_root: bool,
        seen: &mut std::collections::HashSet<usize>,
    ) -> Summary {
        let id = node as *const Node as usize;
        if !is_root && seen.contains(&id) {
            return node.summary;
        }
        match &node.kind {
            Kind::Leaf(text) => {
                assert_eq!(node.height, 0);
                assert!(text.len() <= S::MAX_CHUNK, "leaf of {} bytes", text.len());
                assert!(
                    is_root || text.len() >= S::MIN_CHUNK,
                    "leaf of {} bytes",
                    text.len()
                );
                assert!(!text.contains('\r'));
                assert_eq!(node.summary, Summary::of(text));
            }
            Kind::Internal(children) => {
                assert!(children.len() <= S::MAX_CHILDREN);
                assert!(children.len() >= if is_root { 2 } else { S::MIN_CHILDREN });
                let mut sum = Summary::default();
                for child in children {
                    assert_eq!(child.height + 1, node.height);
                    sum = sum + walk::<S>(child, false, seen);
                }
                assert_eq!(node.summary, sum);
            }
        }
        if !is_root {
            seen.insert(id);
        }
        node.summary
    }
    walk::<S>(root, true, seen);
}
