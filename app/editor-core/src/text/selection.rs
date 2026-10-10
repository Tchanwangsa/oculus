//! Selections, ported from `@codemirror/state` 6.7.6's `SelectionRange` and
//! `EditorSelection` (MIT, Marijn Haverbeke; see `NOTICE`). A selection is one or more ranges, sorted and
//! non-overlapping, with one of them the main range. `Selection::create`
//! normalises exactly as CodeMirror does (see `normalized`), so the merge rules,
//! the direction of a merged range and the main index all match.

use super::change::ChangeDesc;

/// A range with an anchor (the end that stays put when extending) and a head.
/// `assoc` is which side a cursor sticks to (-1, 0 or 1); `bidi_level` and
/// `goal_column` (a pixel offset for vertical motion) are carried for the view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionRange {
    from: usize,
    to: usize,
    inverted: bool,
    assoc: i8,
    bidi_level: Option<u8>,
    goal_column: Option<f64>,
}

impl SelectionRange {
    /// A cursor at `pos`, as `EditorSelection.cursor`.
    pub fn cursor(pos: usize, assoc: i8, bidi_level: Option<u8>, goal_column: Option<f64>) -> Self {
        SelectionRange {
            from: pos,
            to: pos,
            inverted: false,
            assoc: assoc.signum(),
            bidi_level: bidi_level.map(|l| l.min(6)),
            goal_column,
        }
    }

    /// A range from `anchor` to `head`, as `EditorSelection.range`. Without an
    /// `assoc`, a non-empty range sticks inward: -1 forward, 1 backward.
    pub fn range(
        anchor: usize,
        head: usize,
        goal_column: Option<f64>,
        bidi_level: Option<u8>,
        assoc: i8,
    ) -> Self {
        let assoc = if assoc == 0 && anchor != head {
            if head < anchor { 1 } else { -1 }
        } else {
            assoc.signum()
        };
        SelectionRange {
            from: anchor.min(head),
            to: anchor.max(head),
            inverted: head < anchor,
            assoc,
            bidi_level: bidi_level.map(|l| l.min(6)),
            goal_column,
        }
    }

    /// The range CodeMirror holds as `from`, `to`, its direction and its
    /// flags, taken verbatim: mapping can leave `from > to`, which
    /// `range(anchor, head)` would reorder (so later mappings would differ).
    /// `anchor`/`head` must be `from`/`to` in one order or the other.
    pub fn raw(
        (anchor, head): (usize, usize),
        (from, to): (usize, usize),
        goal_column: Option<f64>,
        bidi_level: Option<u8>,
        assoc: i8,
    ) -> Option<Self> {
        let inverted = if (anchor, head) == (from, to) {
            false
        } else if (anchor, head) == (to, from) {
            true
        } else {
            return None;
        };
        Some(SelectionRange {
            from,
            to,
            inverted,
            assoc: assoc.signum(),
            bidi_level: bidi_level.map(|l| l.min(6)),
            goal_column,
        })
    }

    /// `range(anchor, head)` with no goal column, bidi level or assoc.
    pub fn new(anchor: usize, head: usize) -> Self {
        Self::range(anchor, head, None, None, 0)
    }

    /// The lower boundary. Mapping a range over a replacement inside it can
    /// leave `from > to`, as in CodeMirror.
    pub fn from(&self) -> usize {
        self.from
    }

    /// The upper boundary.
    pub fn to(&self) -> usize {
        self.to
    }

    pub fn anchor(&self) -> usize {
        if self.inverted { self.to } else { self.from }
    }

    pub fn head(&self) -> usize {
        if self.inverted { self.from } else { self.to }
    }

    pub fn is_empty(&self) -> bool {
        self.from == self.to
    }

    pub fn assoc(&self) -> i8 {
        self.assoc
    }

    pub fn bidi_level(&self) -> Option<u8> {
        self.bidi_level
    }

    pub fn goal_column(&self) -> Option<f64> {
        self.goal_column
    }

    /// The range mapped through `change`. A cursor maps with `assoc`; a
    /// non-empty range shrinks away from insertions at its ends.
    pub fn map(&self, change: &ChangeDesc, assoc: i32) -> Self {
        let (from, to) = if self.is_empty() {
            let pos = change.map_pos(self.from, assoc);
            (pos, pos)
        } else {
            (change.map_pos(self.from, 1), change.map_pos(self.to, -1))
        };
        SelectionRange { from, to, ..*self }
    }

    /// The range extended to cover `from..to`, as CodeMirror's `extend`.
    pub fn extend(&self, from: usize, to: usize, assoc: i8) -> Self {
        let anchor = self.anchor();
        if from <= anchor && to >= anchor {
            return Self::range(from, to, None, None, assoc);
        }
        let head = if from.abs_diff(anchor) > to.abs_diff(anchor) {
            from
        } else {
            to
        };
        Self::range(anchor, head, None, None, assoc)
    }

    /// CodeMirror's `eq`: anchor, head and goal column; with `include_assoc`,
    /// also the assoc of a cursor.
    pub fn eq(&self, other: &Self, include_assoc: bool) -> bool {
        self.anchor() == other.anchor()
            && self.head() == other.head()
            && self.goal_column == other.goal_column
            && (!include_assoc || !self.is_empty() || self.assoc == other.assoc)
    }
}

/// One or more ranges, sorted and non-overlapping, with a main range.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    ranges: Vec<SelectionRange>,
    main: usize,
}

/// A refused selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionError {
    /// A selection needs at least one range.
    Empty,
    /// The main index is not one of the ranges.
    MainOutOfRange { main: usize, ranges: usize },
    /// A range ends past the document.
    OutOfDocument { to: usize, len: usize },
}

impl std::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SelectionError::Empty => write!(f, "a selection needs at least one range"),
            SelectionError::MainOutOfRange { main, ranges } => {
                write!(f, "main index {main} is not one of {ranges} ranges")
            }
            SelectionError::OutOfDocument { to, len } => {
                write!(
                    f,
                    "selection end {to} is outside a document of length {len}"
                )
            }
        }
    }
}

impl std::error::Error for SelectionError {}

impl Selection {
    /// The selection of `ranges` with `ranges[main]` as the main range,
    /// normalised as CodeMirror's `EditorSelection.create`.
    pub fn create(ranges: Vec<SelectionRange>, main: usize) -> Result<Self, SelectionError> {
        if ranges.is_empty() {
            return Err(SelectionError::Empty);
        }
        if main >= ranges.len() {
            return Err(SelectionError::MainOutOfRange {
                main,
                ranges: ranges.len(),
            });
        }
        Ok(Self::create_valid(ranges, main))
    }

    fn create_valid(ranges: Vec<SelectionRange>, main: usize) -> Self {
        let mut pos = 0;
        for range in &ranges {
            if if range.is_empty() {
                range.from <= pos
            } else {
                range.from < pos
            } {
                return Self::normalized(ranges, main);
            }
            pos = range.to;
        }
        Selection { ranges, main }
    }

    /// Sorts by `from` (stably) and merges each range into the previous one
    /// when it overlaps it, or, for a cursor, touches it. A merged range spans
    /// both, points backward only if the later range did, and drops goal
    /// column, bidi level and explicit assoc.
    fn normalized(ranges: Vec<SelectionRange>, main: usize) -> Self {
        let mut indexed: Vec<(usize, SelectionRange)> = ranges.into_iter().enumerate().collect();
        indexed.sort_by_key(|(_, r)| r.from);
        let mut main = indexed
            .iter()
            .position(|&(i, _)| i == main)
            .expect("main is one of the ranges");
        let mut ranges: Vec<SelectionRange> = indexed.into_iter().map(|(_, r)| r).collect();
        let mut i = 1;
        while i < ranges.len() {
            let (range, prev) = (ranges[i], ranges[i - 1]);
            if if range.is_empty() {
                range.from <= prev.to
            } else {
                range.from < prev.to
            } {
                let (from, to) = (prev.from, range.to.max(prev.to));
                if i <= main {
                    main -= 1;
                }
                let merged = if range.anchor() > range.head() {
                    SelectionRange::new(to, from)
                } else {
                    SelectionRange::new(from, to)
                };
                ranges.splice(i - 1..=i, [merged]);
            } else {
                i += 1;
            }
        }
        Selection { ranges, main }
    }

    /// The selection of `ranges` as given, not sorted or merged, as
    /// `EditorSelection.fromJSON` builds it (a history can hold selections
    /// that `create` would merge).
    pub fn verbatim(ranges: Vec<SelectionRange>, main: usize) -> Result<Self, SelectionError> {
        if ranges.is_empty() {
            return Err(SelectionError::Empty);
        }
        if main >= ranges.len() {
            return Err(SelectionError::MainOutOfRange {
                main,
                ranges: ranges.len(),
            });
        }
        Ok(Selection { ranges, main })
    }

    /// A one-range selection.
    pub fn single(anchor: usize, head: usize) -> Self {
        Selection {
            ranges: vec![SelectionRange::new(anchor, head)],
            main: 0,
        }
    }

    pub fn ranges(&self) -> &[SelectionRange] {
        &self.ranges
    }

    pub fn main_index(&self) -> usize {
        self.main
    }

    pub fn main(&self) -> &SelectionRange {
        &self.ranges[self.main]
    }

    /// The selection mapped through `change`, renormalised.
    pub fn map(&self, change: &ChangeDesc, assoc: i32) -> Self {
        if change.is_empty() {
            return self.clone();
        }
        Self::create_valid(
            self.ranges.iter().map(|r| r.map(change, assoc)).collect(),
            self.main,
        )
    }

    /// CodeMirror's `eq`: same main index and pairwise-equal ranges.
    pub fn eq(&self, other: &Self, include_assoc: bool) -> bool {
        self.main == other.main
            && self.ranges.len() == other.ranges.len()
            && self
                .ranges
                .iter()
                .zip(&other.ranges)
                .all(|(a, b)| a.eq(b, include_assoc))
    }

    /// Only the main range.
    pub fn as_single(&self) -> Self {
        if self.ranges.len() == 1 {
            return self.clone();
        }
        Selection {
            ranges: vec![*self.main()],
            main: 0,
        }
    }

    /// `range` added (first, before normalising), as the main range if `main`.
    pub fn add_range(&self, range: SelectionRange, main: bool) -> Self {
        let mut ranges = Vec::with_capacity(self.ranges.len() + 1);
        ranges.push(range);
        ranges.extend_from_slice(&self.ranges);
        Self::create_valid(ranges, if main { 0 } else { self.main + 1 })
    }

    /// Range `which` replaced by `range`.
    pub fn replace_range(&self, range: SelectionRange, which: usize) -> Self {
        let mut ranges = self.ranges.clone();
        ranges[which] = range;
        Self::create_valid(ranges, self.main)
    }

    /// Refuses a selection reaching past a document of `len`, as
    /// CodeMirror's `checkSelection`.
    pub fn check(&self, len: usize) -> Result<(), SelectionError> {
        match self.ranges.iter().find(|r| r.to > len) {
            Some(r) => Err(SelectionError::OutOfDocument { to: r.to, len }),
            None => Ok(()),
        }
    }
}
