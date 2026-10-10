//! Slots and stops: the places a caret can be in a formula.

use core::ops::Range;

/// A slot's index in [`crate::Stops::slots`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlotId(pub usize);

/// A stop's index in [`crate::Stops::stops`]: its place in ←/→ order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StopId(pub usize);

/// What a slot is to its structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SlotKind {
    /// The formula's top level, row `n`: rows are split at top-level `\\`;
    /// a formula without one is row 0.
    Row(usize),
    /// A `{…}` group's content (`\begingroup…\endgroup` too).
    Group,
    /// A script after `^`.
    Sup,
    /// A script after `_`.
    Sub,
    Numer,
    Denom,
    /// `\sqrt`'s radicand.
    Radicand,
    /// `\sqrt`'s `[…]` index.
    Index,
    /// A command's argument: an accent's or `\overline`'s base, a font's,
    /// `\operatorname`'s, `\textcolor`'s, `\boxed`'s, `\mathop`'s, `\overset`'s
    /// base, one of `\mathchoice`'s four.
    Body,
    /// What goes above: `\overset`'s and `\stackrel`'s first argument,
    /// `\xrightarrow`'s label.
    Above,
    /// What goes below: `\underset`'s first argument, `\xrightarrow`'s
    /// `[…]` label.
    Below,
    /// A `\left…\right` body.
    LeftRight,
    /// An array cell.
    Cell {
        row: usize,
        col: usize,
    },
    /// A `\text{…}`-family run.
    Text,
    /// `$…$` (or `\(…\)`) inside text.
    Math,
    /// A `\tag{…}` label.
    Tag,
}

/// How a slot's extent is fixed, which decides what an insertion into it
/// must add.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bounds {
    /// Between delimiters of its own: `{…}`, an optional argument's `[…]`,
    /// `$…$`, `\left…\right`'s delimiters. An insertion stays inside.
    Delimited,
    /// A one-token argument written without braces (`\frac ab`, `x^2`,
    /// `\hat x`): a second atom needs braces around the argument first.
    Bare,
    /// A run bounded by what is around it: a row, a cell, a side of
    /// `\over`. It grows with what is typed.
    Open,
}

/// How a slot was reached from the top.
///
/// The row, then for each level the atom's index in its slot and the
/// argument's index among the atom's slots (an array's cells count row by
/// row). Stable across re-parses of unchanged structure.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlotPath {
    pub row: usize,
    pub steps: Vec<(usize, usize)>,
}

/// An ordered run of sibling atoms the caret moves between.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub kind: SlotKind,
    pub bounds: Bounds,
    /// A text-mode run: a stop between every character, spaces included.
    pub text: bool,
    pub path: SlotPath,
    /// Byte range of the content: inside the braces of a group, the cell's
    /// own range, the row between its `\\`s. Empty for an empty slot.
    pub interior: Range<usize>,
    /// The atoms' byte ranges, in source order. A macro's output and a
    /// style switch's command are atoms with no stops inside.
    pub atoms: Vec<Range<usize>>,
    /// The slot holding the atom this slot belongs to.
    pub parent: Option<SlotId>,
    /// This slot's stops, in order (never empty).
    pub stops: Vec<StopId>,
}

impl Slot {
    /// Whether the slot holds nothing (it then has exactly one stop).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }
}

/// A caret position: a byte offset in a slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stop {
    pub offset: usize,
    pub slot: SlotId,
    /// The stop's place among its slot's stops.
    pub index: usize,
}
