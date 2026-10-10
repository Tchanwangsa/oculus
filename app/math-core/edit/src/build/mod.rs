//! Laying out slots and stops from a parse tree (source mapping on).
//!
//! A slot's elements are parse nodes; they become its atoms after
//! [`Builder::atoms`] flattens what has no place of its own in the run
//! (a SupSub's base and scripts, a style switch's run, wrappers that share
//! their child's range) and merges atoms whose ranges overlap (a macro's
//! output, which all maps to its invocation). Each atom's own slots come
//! from [`arguments::slots`]. Stops are pushed depth first, so the list is
//! ←/→ order and source order at once.

mod arguments;
mod source;

use core::{cmp::Reverse, mem, ops::Range};

use katex::{
    parser::parse_node::AnyParseNode,
    source_map::{SourceRange, continues_cluster},
    types::Mode,
};

use crate::slot::{Bounds, Slot, SlotId, SlotKind, SlotPath, Stop, StopId};
use source::Source;

/// A slot before layout: what it is, where its content goes, and the parse
/// nodes it holds.
struct SlotSpec<'n> {
    kind: SlotKind,
    bounds: Bounds,
    interior: Range<usize>,
    text: bool,
    elements: Vec<&'n AnyParseNode>,
}

/// What an atom is, for finding its slots.
enum Item<'n> {
    /// One parse node.
    Node(&'n AnyParseNode),
    /// A base's scripts written next to each other (`_1^2`): each `^`/`_`
    /// with its argument, in source order.
    Scripts(Vec<(&'n AnyParseNode, SlotKind)>),
    /// The `\tag{…}` invocation; its nodes come from the macro's body.
    Tag(Vec<&'n AnyParseNode>),
    /// No slots: a style switch's command, or merged macro output.
    Opaque,
}

struct Atom<'n> {
    range: Range<usize>,
    item: Item<'n>,
}

/// The slots and stops of a parsed formula.
pub fn build(source: &str, nodes: &[AnyParseNode]) -> (Vec<Slot>, Vec<Stop>) {
    let mut builder = Builder {
        src: Source(source),
        slots: Vec::new(),
        stops: Vec::new(),
    };
    let atoms = builder.atoms(nodes.iter());
    let mut row = Vec::new();
    let mut start = 0;
    let mut rows = Vec::new();
    for atom in atoms {
        if matches!(atom.item, Item::Node(AnyParseNode::Cr(_))) {
            rows.push((start..atom.range.start, mem::take(&mut row)));
            start = atom.range.end;
        } else {
            row.push(atom);
        }
    }
    rows.push((start..source.len(), row));
    for (n, (interior, atoms)) in rows.into_iter().enumerate() {
        let spec = SlotSpec {
            kind: SlotKind::Row(n),
            bounds: Bounds::Open,
            interior,
            text: false,
            elements: Vec::new(),
        };
        let path = SlotPath {
            row: n,
            steps: Vec::new(),
        };
        builder.lay_out(&spec, &atoms, None, &path);
    }
    (builder.slots, builder.stops)
}

struct Builder<'s> {
    src: Source<'s>,
    slots: Vec<Slot>,
    stops: Vec<Stop>,
}

impl<'n> Builder<'_> {
    fn lay_out(
        &mut self,
        spec: &SlotSpec<'n>,
        atoms: &[Atom<'n>],
        parent: Option<SlotId>,
        path: &SlotPath,
    ) {
        let mut interior = spec.interior.clone();
        if spec.bounds != Bounds::Delimited
            && let Some(last) = atoms.last()
        {
            // An operator's `\limits` belongs to the run, though the cover
            // of the nodes (a cell's range, a bare argument's) stops before
            // it.
            interior.end = interior.end.max(last.range.end);
        }
        let id = SlotId(self.slots.len());
        self.slots.push(Slot {
            kind: spec.kind,
            bounds: spec.bounds,
            text: spec.text,
            path: path.clone(),
            interior: interior.clone(),
            atoms: atoms.iter().map(|atom| atom.range.clone()).collect(),
            parent,
            stops: Vec::new(),
        });
        self.stop(id, interior.start);
        for (i, atom) in atoms.iter().enumerate() {
            // An `\over` taken whole as a bare argument (`\frac]\over`) has
            // no sides to type in: a letter in either would leave the
            // argument and meet a second `\over`.
            let children = if spec.bounds == Bounds::Bare && self.infix(atom) {
                Vec::new()
            } else {
                self.child_specs(atom)
            };
            for (j, child) in children.iter().enumerate() {
                let child_atoms = self.atoms(child.elements.iter().copied());
                let mut child_path = path.clone();
                child_path.steps.push((i, j));
                self.lay_out(child, &child_atoms, Some(id), &child_path);
            }
            if spec.text && self.plain(&atom.range) {
                let inner = self.src.text(atom.range.clone()).char_indices().skip(1);
                for (at, _) in inner {
                    let at = atom.range.start + at;
                    if !self.cluster_mark_at(at) {
                        self.stop(id, at);
                    }
                }
            }
            let joined = atoms.get(i + 1).is_some_and(|next| {
                next.range.start == atom.range.end && self.cluster_mark_at(next.range.start)
            });
            if !joined {
                self.stop(id, atom.range.end);
            }
        }
    }

    fn infix(&self, atom: &Atom<'n>) -> bool {
        let Item::Node(AnyParseNode::Genfrac(frac)) = atom.item else {
            return false;
        };
        self.src
            .range(&frac.numer)
            .is_some_and(|numer| numer.start == atom.range.start)
    }

    fn stop(&mut self, slot: SlotId, offset: usize) {
        let stops = &mut self.slots[slot.0].stops;
        self.stops.push(Stop {
            offset,
            slot,
            index: stops.len(),
        });
        stops.push(StopId(self.stops.len() - 1));
    }

    /// Whether a combining mark starts at `at`: a caret never splits a
    /// cluster (the renderer keeps it one glyph).
    fn cluster_mark_at(&self, at: usize) -> bool {
        let point = Some(SourceRange { start: at, end: at });
        continues_cluster(point, point, &self.src.0[at..])
    }

    /// Whether a text atom is plain characters, so a caret may sit between
    /// any two of them (`---`, a Thai syllable), not a control sequence.
    fn plain(&self, range: &Range<usize>) -> bool {
        !self
            .src
            .text(range.clone())
            .contains(['\\', '{', '}', '$', '&', '#', '^', '_', '~', '%'])
    }

    /// The slots of one atom, in source order; none when any of them does
    /// not fit inside the atom (a macro's output posing as a structure).
    fn child_specs(&self, atom: &Atom<'n>) -> Vec<SlotSpec<'n>> {
        let specs = match &atom.item {
            Item::Node(node) => self
                .src
                .range(node)
                .and_then(|range| arguments::slots(self.src, node, range)),
            Item::Scripts(scripts) => scripts
                .iter()
                .map(|(node, kind)| arguments::argument(self.src, atom.range.clone(), node, *kind))
                .collect(),
            Item::Tag(nodes) => {
                arguments::label(self.src, atom.range.clone(), nodes).map(|spec| vec![spec])
            }
            Item::Opaque => None,
        };
        let Some(mut specs) = specs else {
            return Vec::new();
        };
        let fits = specs.iter().all(|spec| {
            within(&spec.interior, &atom.range)
                && spec.elements.iter().all(|element| {
                    self.src
                        .range(element)
                        .is_none_or(|range| within(&range, &spec.interior))
                })
        });
        if !fits {
            return Vec::new();
        }
        specs.sort_by_key(|spec| (spec.interior.start, spec.interior.end));
        specs
    }

    /// A run's atoms: its elements flattened, sorted, and merged where
    /// they overlap. Atoms with no source (zero width) have no place.
    fn atoms(&self, elements: impl Iterator<Item = &'n AnyParseNode>) -> Vec<Atom<'n>> {
        let mut flat = Vec::new();
        for element in elements {
            self.flatten(element, &mut flat);
        }
        flat.retain(|atom| atom.range.start < atom.range.end);
        flat.sort_by_key(|atom| (atom.range.start, Reverse(atom.range.end)));
        let mut out: Vec<Atom<'n>> = Vec::with_capacity(flat.len());
        for mut atom in flat {
            match out.last_mut() {
                Some(last) if atom.range.start < last.range.end => {
                    last.range.end = last.range.end.max(atom.range.end);
                    last.item = Item::Opaque;
                }
                _ => {
                    atom.range.end = self.limits_end(atom.range.end);
                    out.push(atom);
                }
            }
        }
        out
    }

    /// Where an operator's `\limits`/`\nolimits` after `end` ends: they
    /// make no node of their own but must stay with their operator.
    fn limits_end(&self, mut end: usize) -> usize {
        loop {
            let at = end + self.src.0[end..].len() - self.src.0[end..].trim_start().len();
            match self.src.command_at(at) {
                Some(r"\limits" | r"\nolimits") => end = self.src.token_end(at),
                _ => return end,
            }
        }
    }

    fn flatten(&self, node: &'n AnyParseNode, out: &mut Vec<Atom<'n>>) {
        let Some(range) = self.src.range(node) else {
            // Made up by the parser (`aligned`'s spacing `{}`): nothing to
            // stand between.
            return;
        };
        if let Some(child) = self.wrapped_child(node, &range) {
            return self.flatten(child, out);
        }
        match node {
            AnyParseNode::SupSub(supsub) => {
                if let Some(base) = &supsub.base {
                    self.flatten(base, out);
                }
                let mut scripts = Vec::new();
                for script in [&supsub.sup, &supsub.sub].into_iter().flatten() {
                    self.script(script, &range, &mut scripts);
                }
                scripts.sort_by_key(|atom| atom.range.start);
                let mut merged: Vec<Atom<'n>> = Vec::with_capacity(scripts.len());
                for atom in scripts {
                    if let (Some(last), Item::Scripts(more)) = (merged.last_mut(), &atom.item)
                        && let Item::Scripts(before) = &mut last.item
                    {
                        before.extend(more.iter().copied());
                        last.range.end = atom.range.end;
                    } else {
                        merged.push(atom);
                    }
                }
                out.extend(merged);
            }
            AnyParseNode::OrdGroup(group)
                if !self.src.wrapped(&range, '{', '}') && !self.begingroup(&range) =>
            {
                for child in &group.body {
                    self.flatten(child, out);
                }
            }
            AnyParseNode::Styling(styling) => {
                let written = self.src.text(range.clone());
                if written.starts_with('$') || written.starts_with(r"\(") {
                    out.push(Atom {
                        range,
                        item: Item::Node(node),
                    });
                } else if self.src.command_at(range.start).is_some() {
                    self.switch(range, &styling.body, out);
                } else {
                    // An array cell, or a macro's output.
                    for child in &styling.body {
                        self.flatten(child, out);
                    }
                }
            }
            AnyParseNode::Sizing(sizing) if self.src.command_at(range.start).is_some() => {
                self.switch(range, &sizing.body, out);
            }
            AnyParseNode::Color(color) if self.src.command_at(range.start) == Some(r"\color") => {
                self.switch(range, &color.body, out);
            }
            AnyParseNode::Font(font) => match &*font.body {
                // `\bf ab`: an old-style switch, its run a group with its range.
                AnyParseNode::OrdGroup(group)
                    if self.src.range(&font.body).as_ref() == Some(&range) =>
                {
                    self.switch(range, &group.body, out);
                }
                _ => out.push(Atom {
                    range,
                    item: Item::Node(node),
                }),
            },
            AnyParseNode::Tag(tag) => {
                for child in &tag.body {
                    self.flatten(child, out);
                }
                let label: Vec<&AnyParseNode> = tag.tag.iter().collect();
                let cover = label
                    .iter()
                    .filter_map(|node| self.src.range(node))
                    .reduce(|a, b| a.start.min(b.start)..a.end.max(b.end));
                if let Some(range) = cover {
                    out.push(Atom {
                        range,
                        item: Item::Tag(label),
                    });
                }
            }
            _ => out.push(Atom {
                range,
                item: Item::Node(node),
            }),
        }
    }

    /// A style switch (`\displaystyle`, `\large`, `\color{red}`, `\bf`)
    /// runs to the end of its group: its command is an atom of its own and
    /// its run joins the slot it sits in, so the caret moves through one
    /// run rather than two that end together. When its run does not
    /// follow the command (a macro), it is one opaque atom.
    fn switch(&self, range: Range<usize>, body: &'n [AnyParseNode], out: &mut Vec<Atom<'n>>) {
        let starts = body
            .iter()
            .filter_map(|child| self.src.range(child))
            .map(|r| r.start);
        let first = starts.clone().min().unwrap_or(range.end);
        let command_end = self
            .src
            .command_at(range.start)
            .map_or(range.start, |word| range.start + word.len());
        if first < command_end {
            out.push(Atom {
                range,
                item: Item::Opaque,
            });
            return;
        }
        out.push(Atom {
            range: range.start..first,
            item: Item::Opaque,
        });
        for child in body {
            self.flatten(child, out);
        }
    }

    /// One of a SupSub's scripts. Written after `^`/`_`, it is an atom
    /// holding its slot; a primes group (`x''^2`) is its primes plus the
    /// script after them; primes alone and Unicode scripts (`x²`) are
    /// plain atoms.
    fn script(&self, script: &'n AnyParseNode, supsub: &Range<usize>, out: &mut Vec<Atom<'n>>) {
        let Some(range) = self.src.range(script) else {
            return;
        };
        if let Some(atom) = self.script_atom(script, &range, supsub) {
            out.push(atom);
            return;
        }
        if let AnyParseNode::OrdGroup(group) = script
            && !self.src.wrapped(&range, '{', '}')
        {
            for child in &group.body {
                let atom = self
                    .src
                    .range(child)
                    .and_then(|range| self.script_atom(child, &range, supsub));
                match atom {
                    Some(atom) => out.push(atom),
                    None => self.flatten(child, out),
                }
            }
            return;
        }
        self.flatten(script, out);
    }

    fn script_atom(
        &self,
        script: &'n AnyParseNode,
        range: &Range<usize>,
        supsub: &Range<usize>,
    ) -> Option<Atom<'n>> {
        if range == supsub {
            return None;
        }
        let (op, ch) = self.src.script_op(range.start, supsub.start)?;
        let kind = if ch == '^' {
            SlotKind::Sup
        } else {
            SlotKind::Sub
        };
        Some(Atom {
            range: op..range.end,
            item: Item::Scripts(vec![(script, kind)]),
        })
    }

    /// The child a wrapper shares its range with: `\dfrac`'s Styling over
    /// its Genfrac, `\boldsymbol`'s Mclass over its Font, a matrix's
    /// LeftRight over its Array.
    fn wrapped_child(
        &self,
        node: &'n AnyParseNode,
        range: &Range<usize>,
    ) -> Option<&'n AnyParseNode> {
        let children = match node {
            AnyParseNode::Styling(styling) => &styling.body,
            AnyParseNode::Mclass(mclass) => &mclass.body,
            AnyParseNode::LeftRight(left_right) => &left_right.body,
            _ => return None,
        };
        match children.as_slice() {
            // `\overset{a}{b}`'s Mclass over its SupSub is a structure of
            // two arguments, not a wrapper.
            [AnyParseNode::SupSub(_)] => None,
            [child] if self.src.range(child).as_ref() == Some(range) => Some(child),
            _ => None,
        }
    }

    fn begingroup(&self, range: &Range<usize>) -> bool {
        self.src.command_at(range.start) == Some(r"\begingroup")
    }
}

/// Whether `inner` lies inside `outer`.
const fn within(inner: &Range<usize>, outer: &Range<usize>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// Whether a run is text: its atoms are text-mode, or, when it is empty,
/// the node holding it is.
fn is_text(container: Option<&AnyParseNode>, elements: &[&AnyParseNode]) -> bool {
    elements.first().map_or_else(
        || {
            container.is_some_and(|node| {
                matches!(node, AnyParseNode::Text(_) | AnyParseNode::Hbox(_))
                    || node.mode() == Mode::Text
            })
        },
        |first| first.mode() == Mode::Text,
    )
}
