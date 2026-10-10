//! The grid at the caret: the structure directly holding it, read from the
//! source. A cell of a matrix environment, or the body of a bracket group
//! whose brackets draw one: a `\left…\right` pair, or an opening bracket
//! atom (typing a bracket writes it alone) with whatever closes it later
//! in the same slot.

use core::ops::Range;

use katex::{
    build_html::DomType, parser::parse_node::AnyParseNode, symbols::Atom,
    types::ErrorLocationProvider as _,
};

use super::{layout::style, model::Model};
use crate::{
    field::{Field, Mode},
    parse::parse,
    slot::{SlotId, SlotKind},
    stops::Stops,
};

/// The environments typed as grids: matrices, not `array`, `cases` or
/// `aligned`, whose cells hold more than terms.
pub const MATRIX_ENVS: [&str; 7] = [
    "matrix",
    "pmatrix",
    "bmatrix",
    "Bmatrix",
    "vmatrix",
    "Vmatrix",
    "smallmatrix",
];

/// What the caret comes after in its cell: nothing, something that wants
/// what follows (an operator, relation, punctuation, opening, `\sin`-like
/// or `\sum`-like operator), or a term.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Follows {
    Start,
    Operator,
    Term,
}

/// The caret in a grid's cell.
#[derive(Clone, Debug)]
pub struct CellCaret {
    pub row: usize,
    pub col: usize,
    /// The cell's source before and after the caret, trimmed.
    pub before: String,
    pub after: String,
    pub follows: Follows,
    /// The cell is one binary operator or relation and nothing else.
    pub lone: bool,
}

/// The grid around the caret and the structure that holds it.
#[derive(Clone, Debug)]
pub struct GridAt {
    pub env: &'static str,
    /// A bracket group, not yet a matrix.
    pub group: bool,
    /// The slot holding the matrix or the group, and the atom's index
    /// there (a group's opening bracket, or its `\left…\right`).
    pub slot: SlotId,
    pub atom: usize,
    /// The matrix's source, or the group's: its opening bracket through
    /// its closer, or through the end of the slot.
    pub range: Range<usize>,
    /// What the edit rewrites: the matrix's content between `\begin{…}`
    /// and `\end{…}`; the group's whole range.
    pub content: Range<usize>,
    /// A matrix's cell slots, row by row (empty for a group).
    pub cells: Vec<Vec<SlotId>>,
    /// The end of the scripts written right after it (`^T`), if any.
    pub scripts_end: Option<usize>,
    pub model: Model,
    pub caret: CellCaret,
}

/// The grid the caret is directly in; maths mode, a caret, no pending
/// command.
pub fn grid_at(field: &Field) -> Option<GridAt> {
    if !field.selection().is_caret() || field.mode() != Mode::Math {
        return None;
    }
    let stops = field.stops();
    let head = field.selection().head;
    let slot = stops.stop(head).slot;
    let offset = stops.offset(head);
    let nodes = parse(field.source(), field.display()).ok()?;
    let found = Found {
        field,
        stops,
        nodes: &nodes,
    };
    match found.bracket_group(slot, offset) {
        Scan::Group(grid) => return Some(*grid),
        Scan::Unmatched => return None,
        Scan::None => {}
    }
    match stops.slot(slot).kind {
        SlotKind::Cell { row, col } => found.matrix(slot, row, col, offset),
        SlotKind::LeftRight => found.left_right(slot, offset),
        _ => None,
    }
}

enum Scan {
    Group(Box<GridAt>),
    /// The caret is in a bracket pair that draws no matrix (`[0,1)`).
    Unmatched,
    None,
}

struct Found<'a> {
    field: &'a Field,
    stops: &'a Stops,
    nodes: &'a [AnyParseNode],
}

/// Which side a bracket opens or closes; `|` and `\|` may do either.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Open,
    Close,
    Either,
}

/// A bracket atom's side and the matrix its kind draws.
fn bracket(text: &str) -> Option<(Side, &'static str)> {
    Some(match text {
        "(" | r"\lparen" => (Side::Open, "pmatrix"),
        "[" | r"\lbrack" => (Side::Open, "bmatrix"),
        r"\{" | r"\lbrace" => (Side::Open, "Bmatrix"),
        r"\lvert" => (Side::Open, "vmatrix"),
        r"\lVert" => (Side::Open, "Vmatrix"),
        "|" | r"\vert" => (Side::Either, "vmatrix"),
        r"\|" | r"\Vert" => (Side::Either, "Vmatrix"),
        ")" | r"\rparen" => (Side::Close, "pmatrix"),
        "]" | r"\rbrack" => (Side::Close, "bmatrix"),
        r"\}" | r"\rbrace" => (Side::Close, "Bmatrix"),
        r"\rvert" => (Side::Close, "vmatrix"),
        r"\rVert" => (Side::Close, "Vmatrix"),
        _ => return None,
    })
}

impl Found<'_> {
    fn src(&self) -> &str {
        self.field.source()
    }

    /// The innermost bracket typed on its own in `slot` that is still
    /// open at the caret. Each closer closes the innermost open bracket;
    /// `|` closes the innermost open `|`, else opens one.
    fn bracket_group(&self, slot: SlotId, offset: usize) -> Scan {
        let s = self.stops.slot(slot);
        if s.text {
            return Scan::None;
        }
        let kinds: Vec<Option<(Side, &str)>> = s
            .atoms
            .iter()
            .map(|atom| bracket(self.src()[atom.clone()].trim()))
            .collect();
        let mut closer: Vec<Option<usize>> = vec![None; kinds.len()];
        let mut opener = vec![false; kinds.len()];
        let mut open: Vec<usize> = Vec::new();
        for (i, kind) in kinds.iter().enumerate() {
            match kind {
                Some((Side::Open, _)) => {
                    opener[i] = true;
                    open.push(i);
                }
                Some((Side::Close, _)) => {
                    if let Some(o) = open.pop() {
                        closer[o] = Some(i);
                    }
                }
                Some((Side::Either, env)) => {
                    let same = open
                        .iter()
                        .rposition(|&o| kinds[o].is_some_and(|(_, e)| e == *env));
                    if let Some(at) = same {
                        closer[open[at]] = Some(i);
                        open.truncate(at);
                    } else {
                        opener[i] = true;
                        open.push(i);
                    }
                }
                None => {}
            }
        }
        let inside = (0..kinds.len()).rev().find(|&o| {
            opener[o]
                && s.atoms[o].end <= offset
                && closer[o].is_none_or(|k| s.atoms[k].start >= offset)
        });
        let Some(o) = inside else {
            return Scan::None;
        };
        let Some((_, env)) = kinds[o] else {
            return Scan::None;
        };
        let close = closer[o];
        if let Some(k) = close
            && kinds[k].is_none_or(|(_, e)| e != env)
        {
            return Scan::Unmatched;
        }
        let body_end = close.map_or(s.interior.end, |k| s.atoms[k].start);
        let range = s.atoms[o].start..close.map_or(s.interior.end, |k| s.atoms[k].end);
        let body = s.atoms[o].end..body_end;
        let atoms: Vec<Range<usize>> = s.atoms[o + 1..close.unwrap_or(s.atoms.len())].to_vec();
        let mut model = Model::group(&self.src()[body.clone()]);
        model.style = style(self.field, slot, &range, &model);
        let caret = self.caret(&atoms, body, offset, 0, 0);
        Scan::Group(Box::new(GridAt {
            env,
            group: true,
            slot,
            atom: o,
            range: range.clone(),
            content: range.clone(),
            cells: Vec::new(),
            scripts_end: self.scripts_end(slot, range.end),
            model,
            caret,
        }))
    }

    /// A cell of a matrix environment.
    fn matrix(&self, cell: SlotId, row: usize, col: usize, offset: usize) -> Option<GridAt> {
        let (slot, atom) = self.stops.owner(cell)?;
        let array = read_array(self.field, slot, atom)?;
        let env = *MATRIX_ENVS.iter().find(|env| **env == array.name)?;
        let s = self.stops.slot(cell);
        let caret = self.caret(&s.atoms, s.interior.clone(), offset, row, col);
        let range = self.stops.slot(slot).atoms[atom].clone();
        Some(GridAt {
            env,
            group: false,
            slot,
            atom,
            scripts_end: self.scripts_end(slot, range.end),
            range,
            content: array.content,
            cells: array.cells,
            model: array.model,
            caret,
        })
    }

    /// The body of a `\left…\right` pair whose delimiters draw a matrix.
    fn left_right(&self, body: SlotId, offset: usize) -> Option<GridAt> {
        let (slot, atom) = self.stops.owner(body)?;
        let range = self.stops.slot(slot).atoms[atom].clone();
        let text = &self.src()[range.clone()];
        let left = delimiter(text.strip_prefix(r"\left")?);
        let right = delimiter(&text[text.rfind(r"\right")? + r"\right".len()..]);
        let (Some((Side::Open | Side::Either, env)), Some((Side::Close | Side::Either, renv))) =
            (bracket(left), bracket(right))
        else {
            return None;
        };
        if env != renv {
            return None;
        }
        let s = self.stops.slot(body);
        let mut model = Model::group(&self.src()[s.interior.clone()]);
        model.style = style(self.field, slot, &range, &model);
        let caret = self.caret(&s.atoms, s.interior.clone(), offset, 0, 0);
        Some(GridAt {
            env,
            group: true,
            slot,
            atom,
            range: range.clone(),
            content: range.clone(),
            cells: Vec::new(),
            scripts_end: self.scripts_end(slot, range.end),
            model,
            caret,
        })
    }

    /// The caret facts for a cell holding `atoms` over `cell`.
    fn caret(
        &self,
        atoms: &[Range<usize>],
        cell: Range<usize>,
        offset: usize,
        row: usize,
        col: usize,
    ) -> CellCaret {
        let src = self.src();
        let at = offset.clamp(cell.start, cell.end);
        let before_atoms: Vec<&Range<usize>> =
            atoms.iter().filter(|atom| atom.end <= offset).collect();
        let follows = match before_atoms.len() {
            0 => Follows::Start,
            mut k => {
                // Scripts go with their base: `\sum_i` wants an operand.
                while k > 1 && src[before_atoms[k - 1].clone()].starts_with(['^', '_']) {
                    k -= 1;
                }
                match self.class(before_atoms[k - 1].clone()) {
                    Class::Term => Follows::Term,
                    _ => Follows::Operator,
                }
            }
        };
        let lone = atoms.len() == 1 && matches!(self.class(atoms[0].clone()), Class::Bin);
        CellCaret {
            row,
            col,
            before: src[cell.start..at].trim().to_owned(),
            after: src[at..cell.end].trim().to_owned(),
            follows,
            lone,
        }
    }

    /// What the atom at exactly `range` is to Space.
    fn class(&self, range: Range<usize>) -> Class {
        let src = self.src();
        let mut stack: Vec<&AnyParseNode> = self.nodes.iter().collect();
        while let Some(node) = stack.pop() {
            let at = node
                .loc()
                .filter(|loc| &*loc.input == src)
                .map(|loc| loc.start..loc.end);
            if at.as_ref() == Some(&range) {
                let class = match node {
                    AnyParseNode::Atom(atom) => match atom.family {
                        Atom::Bin | Atom::Rel => Class::Bin,
                        Atom::Punct | Atom::Open => Class::Operator,
                        _ => Class::Term,
                    },
                    AnyParseNode::Op(_) | AnyParseNode::OperatorName(_) => Class::Operator,
                    AnyParseNode::Mclass(mclass) => match mclass.mclass {
                        DomType::Mbin | DomType::Mrel => Class::Bin,
                        DomType::Mpunct | DomType::Mopen | DomType::Mop => Class::Operator,
                        _ => Class::Term,
                    },
                    _ => Class::Term,
                };
                if class != Class::Term {
                    return class;
                }
            }
            if at.is_none_or(|at| at.start <= range.start && range.end <= at.end) {
                stack.extend(node.children());
            }
        }
        Class::Term
    }

    /// The end of a scripts atom right after `end` in `slot` (`^T`).
    fn scripts_end(&self, slot: SlotId, end: usize) -> Option<usize> {
        let src = self.src();
        let s = self.stops.slot(slot);
        let next = s.atoms.iter().find(|atom| atom.start >= end)?;
        (src[end..next.start].trim().is_empty() && src[next.clone()].starts_with(['^', '_']))
            .then_some(next.end)
    }
}

/// What an atom is to Space: a binary operator or relation (which alone
/// in a cell rejoins the cell before), something else that wants an
/// operand, or a term.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    /// A binary operator or relation.
    Bin,
    Operator,
    Term,
}

/// The delimiter token at the start of `text`, after spaces: a control
/// word, a control symbol or one character.
fn delimiter(text: &str) -> &str {
    let text = text.trim_start();
    let Some(rest) = text.strip_prefix('\\') else {
        return text.chars().next().map_or("", |c| &text[..c.len_utf8()]);
    };
    let letters = rest
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(rest.len());
    let len = if letters > 0 {
        letters
    } else {
        rest.chars().next().map_or(0, char::len_utf8)
    };
    &text[..=len]
}

/// An array environment read from the source: its name, its content
/// between `\begin{…}` and `\end{…}`, its cell slots row by row, and its
/// model (with the style its new separators take).
pub struct Array<'a> {
    pub name: &'a str,
    pub content: Range<usize>,
    pub cells: Vec<Vec<SlotId>>,
    pub model: Model,
}

/// Atom `atom` of `slot` as an array environment.
pub fn read_array(field: &Field, slot: SlotId, atom: usize) -> Option<Array<'_>> {
    let stops = field.stops();
    let range = stops.slot(slot).atoms.get(atom)?.clone();
    let text = &field.source()[range.clone()];
    let name = text.strip_prefix(r"\begin{")?.split('}').next()?;
    let start = range.start + r"\begin{".len() + name.len() + 1;
    let end = range.start + text.rfind(r"\end{")?;
    let mut cells: Vec<Vec<SlotId>> = Vec::new();
    let mut ranges = Vec::new();
    for id in stops.atom_slots(slot, atom) {
        let SlotKind::Cell { row, .. } = stops.slot(id).kind else {
            return None;
        };
        while cells.len() <= row {
            cells.push(Vec::new());
        }
        cells[row].push(id);
        ranges.push((row, stops.slot(id).interior.clone()));
    }
    let mut model = Model::read(field.source(), start..end, &ranges);
    model.style = style(field, slot, &range, &model);
    Some(Array {
        name,
        content: start..end,
        cells,
        model,
    })
}
