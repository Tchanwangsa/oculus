//! The syntax tree, laid out as Lezer's `TreeBuffer` is: one flat array of
//! nodes in pre-order, each holding its type, range, the index just past its
//! subtree, and its parent's index. Positions are UTF-16 code units.
//!
//! Lezer's anonymous balancing nodes never appear here; they are invisible to
//! every `@lezer/common` cursor too. `resolve_inner` follows `resolveNode` and
//! `checkSide` in `@lezer/common`; with nested code languages pruned it is the
//! same as Lezer's `resolveInner` on the app's tree.

use super::tables::NodeType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NodeData {
    kind: NodeType,
    from: u32,
    to: u32,
    /// Index just past this node's subtree.
    end: u32,
    /// Index of the parent; the root's is `u32::MAX`.
    parent: u32,
}

/// A parsed document. Node 0 is the `Document`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    nodes: Vec<NodeData>,
}

/// One node of a [`Tree`]; cheap to copy.
#[derive(Clone, Copy)]
pub struct Node<'t> {
    tree: &'t Tree,
    index: u32,
}

impl PartialEq for Node<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.tree, other.tree) && self.index == other.index
    }
}

impl Eq for Node<'_> {}

impl std::fmt::Debug for Node<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({}..{})", self.name(), self.from(), self.to())
    }
}

/// Lezer's `checkSide` for sides -1, 0 and 1: whether a node over
/// `from..to` holds `pos` when entered from that side.
fn check_side(side: i32, pos: usize, from: usize, to: usize) -> bool {
    match side.signum() {
        -1 => to >= pos && from < pos,
        0 => from < pos && to > pos,
        _ => from <= pos && to > pos,
    }
}

impl Tree {
    /// The root (`Document`) node.
    pub fn root(&self) -> Node<'_> {
        Node {
            tree: self,
            index: 0,
        }
    }

    /// The number of nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The node at pre-order index `index`.
    pub fn node(&self, index: usize) -> Option<Node<'_>> {
        (index < self.nodes.len()).then_some(Node {
            tree: self,
            index: index as u32,
        })
    }

    /// Every node in pre-order, the root first.
    pub fn iter(&self) -> impl Iterator<Item = Node<'_>> + '_ {
        (0..self.nodes.len() as u32).map(move |index| Node { tree: self, index })
    }

    /// The innermost node holding `pos`, entered from `side` (negative: the
    /// node may end at `pos`; positive: it may start there; zero: strictly
    /// inside). Zero-length nodes are never returned unless they are the root.
    pub fn resolve_inner(&self, pos: usize, side: i32) -> Node<'_> {
        let root = &self.nodes[0];
        let (from, to) = (root.from as usize, root.to as usize);
        let outside = from == to
            || (if side < 1 { from >= pos } else { from > pos })
            || (if side > -1 { to <= pos } else { to < pos });
        let mut index = 0u32;
        if !outside {
            'down: loop {
                let mut child = index + 1;
                while child < self.nodes[index as usize].end {
                    let c = &self.nodes[child as usize];
                    if check_side(side, pos, c.from as usize, c.to as usize) {
                        index = child;
                        continue 'down;
                    }
                    child = c.end;
                }
                break;
            }
        }
        Node { tree: self, index }
    }

    /// Builds the flat tree from nested elements, converting each byte offset
    /// with `to_utf16`.
    pub(crate) fn from_element(root: &Elt, mut to_utf16: impl FnMut(usize) -> usize) -> Tree {
        let mut nodes = Vec::new();
        push_element(&mut nodes, root, u32::MAX, &mut to_utf16);
        Tree { nodes }
    }

    /// The `Document`'s direct children as (index, from, to), in order.
    pub(crate) fn top_level(&self) -> Vec<(usize, usize, usize)> {
        self.root()
            .children()
            .map(|n| (n.index(), n.from(), n.to()))
            .collect()
    }

    /// A new document tree of length `len` from three parts: this tree's
    /// top-level blocks before node `prefix_end` (unchanged), `fresh`
    /// blocks (byte offsets mapped by `to_utf16`), then this tree's nodes
    /// from `suffix` on, moved by `shift` units.
    pub(crate) fn splice(
        &self,
        prefix_end: usize,
        fresh: &[Elt],
        mut to_utf16: impl FnMut(usize) -> usize,
        suffix: Option<(usize, isize)>,
        len: usize,
    ) -> Tree {
        let mut nodes = self.nodes[..prefix_end].to_vec();
        nodes[0] = NodeData {
            to: len as u32,
            ..nodes[0]
        };
        for elt in fresh {
            push_element(&mut nodes, elt, 0, &mut to_utf16);
        }
        if let Some((start, shift)) = suffix {
            let base = nodes.len() as i64 - start as i64;
            let moved = |i: u32| (i as i64 + base) as u32;
            let pos = |p: u32| (p as i64 + shift as i64) as u32;
            nodes.extend(self.nodes[start..].iter().map(|n| NodeData {
                kind: n.kind,
                from: pos(n.from),
                to: pos(n.to),
                end: moved(n.end),
                parent: if n.parent == 0 { 0 } else { moved(n.parent) },
            }));
        }
        nodes[0].end = nodes.len() as u32;
        Tree { nodes }
    }
}

/// Appends `root`'s subtree in pre-order under `parent`, setting each new
/// node's `end`.
fn push_element(
    nodes: &mut Vec<NodeData>,
    root: &Elt,
    parent: u32,
    to_utf16: &mut impl FnMut(usize) -> usize,
) {
    let first = nodes.len();
    // (element, parent index) pairs still to visit, the next on top.
    let mut stack: Vec<(&Elt, u32)> = vec![(root, parent)];
    while let Some((elt, parent)) = stack.pop() {
        let index = nodes.len() as u32;
        nodes.push(NodeData {
            kind: elt.kind,
            from: to_utf16(elt.from) as u32,
            to: to_utf16(elt.to) as u32,
            end: index + 1,
            parent,
        });
        stack.extend(elt.children.iter().rev().map(|child| (child, index)));
    }
    // Children follow their parent, so one backward pass sets every end.
    for i in (first + 1..nodes.len()).rev() {
        let (parent, end) = (nodes[i].parent as usize, nodes[i].end);
        nodes[parent].end = nodes[parent].end.max(end);
    }
}

impl<'t> Node<'t> {
    fn data(&self) -> &'t NodeData {
        &self.tree.nodes[self.index as usize]
    }

    fn at(&self, index: u32) -> Node<'t> {
        Node {
            tree: self.tree,
            index,
        }
    }

    /// Pre-order index in the tree.
    pub fn index(&self) -> usize {
        self.index as usize
    }

    pub fn kind(&self) -> NodeType {
        self.data().kind
    }

    /// Lezer's node name.
    pub fn name(&self) -> &'static str {
        self.data().kind.name()
    }

    pub fn from(&self) -> usize {
        self.data().from as usize
    }

    pub fn to(&self) -> usize {
        self.data().to as usize
    }

    pub fn parent(&self) -> Option<Node<'t>> {
        let parent = self.data().parent;
        (parent != u32::MAX).then(|| self.at(parent))
    }

    pub fn first_child(&self) -> Option<Node<'t>> {
        (self.data().end > self.index + 1).then(|| self.at(self.index + 1))
    }

    pub fn last_child(&self) -> Option<Node<'t>> {
        let mut child = self.first_child()?;
        while let Some(next) = child.next_sibling() {
            child = next;
        }
        Some(child)
    }

    pub fn next_sibling(&self) -> Option<Node<'t>> {
        let parent = self.parent()?;
        let next = self.data().end;
        (next < parent.data().end).then(|| self.at(next))
    }

    pub fn prev_sibling(&self) -> Option<Node<'t>> {
        let mut child = self.parent()?.first_child()?;
        if child == *self {
            return None;
        }
        while let Some(next) = child.next_sibling() {
            if next == *self {
                return Some(child);
            }
            child = next;
        }
        None
    }

    /// The direct children, in order.
    pub fn children(&self) -> impl Iterator<Item = Node<'t>> + 't {
        let first = self.first_child();
        std::iter::successors(first, |n| n.next_sibling())
    }
}

/// A node under construction, with byte offsets; Lezer's `Element`.
#[derive(Debug, Clone)]
pub(crate) struct Elt {
    pub kind: NodeType,
    pub from: usize,
    pub to: usize,
    pub children: Vec<Elt>,
}

impl Elt {
    pub fn new(kind: NodeType, from: usize, to: usize) -> Elt {
        Elt {
            kind,
            from,
            to,
            children: Vec::new(),
        }
    }

    pub fn with(kind: NodeType, from: usize, to: usize, children: Vec<Elt>) -> Elt {
        Elt {
            kind,
            from,
            to,
            children,
        }
    }
}

impl Drop for Elt {
    /// Drops the subtree breadth-first: a document nested 100k containers
    /// deep must not recurse once per level.
    fn drop(&mut self) {
        let mut stack = std::mem::take(&mut self.children);
        while let Some(mut elt) = stack.pop() {
            stack.append(&mut elt.children);
        }
    }
}
