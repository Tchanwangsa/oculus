//! Change sets, ported from `@codemirror/state` 6.7.6's `ChangeDesc`/`ChangeSet`
//! (MIT, Marijn Haverbeke; see `NOTICE`).
//!
//! A change is a flat list of sections, each `(len, ins)`: `len` units of the
//! old document that are either kept (`ins == KEEP`) or replaced by `ins` units
//! (`0` is a deletion, a zero `len` an insertion). A `ChangeSet` also holds the
//! inserted texts, one slot per section. Sections are built only through
//! `add_section`, whose merging rules decide the normal form, so `to_json` and
//! every mapping agree with CodeMirror's exactly. Positions are UTF-16 units.
//!
//! `compose`, `map` and `map_pos` panic on mismatched lengths or out-of-range
//! positions, where CodeMirror throws (or, mapping over a longer set, hangs);
//! the `try_` variants return those as errors instead. So does a set read
//! from JSON whose sections do not line up (a zero-length kept section at the
//! end, which CodeMirror's own sets never hold and its walks throw on).

use std::fmt;
use std::ops::Deref;

use super::{PosError, Text};

/// `ins` of a kept (unchanged) section.
const KEEP: isize = -1;
/// `SectionIter::ins` once the iterator is exhausted.
const DONE: isize = -2;

/// How `map_pos` treats a position whose surroundings were changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapMode {
    /// Always map to a position.
    Simple,
    /// `None` when the position was inside a deleted range.
    TrackDel,
    /// `None` when the character before the position was deleted.
    TrackBefore,
    /// `None` when the character after the position was deleted.
    TrackAfter,
}

/// What `touches_range` found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    No,
    Yes,
    /// One change strictly contains the range on both sides.
    Cover,
}

/// A refused `ChangeSet::of` spec, `apply`, JSON form or restored history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeError {
    /// A spec range that is reversed or past the document end.
    InvalidRange { from: usize, to: usize, len: usize },
    /// Lengths that do not line up: a change set applied to, or nested in
    /// specs for, a document of another length, or composed or mapped with a
    /// set of the wrong length.
    LengthMismatch { expected: usize, got: usize },
    /// A section boundary the document refuses (inside a surrogate pair).
    Pos(PosError),
    /// A JSON form or restored history that CodeMirror could not have
    /// produced; the message says what is wrong.
    Malformed(String),
}

impl fmt::Display for ChangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChangeError::InvalidRange { from, to, len } => {
                write!(
                    f,
                    "invalid change range {from} to {to} (in doc of length {len})"
                )
            }
            ChangeError::LengthMismatch { expected, got } => {
                write!(
                    f,
                    "mismatched change set length (got {got}, expected {expected})"
                )
            }
            ChangeError::Pos(e) => e.fmt(f),
            ChangeError::Malformed(what) => f.write_str(what),
        }
    }
}

impl std::error::Error for ChangeError {}

impl From<PosError> for ChangeError {
    fn from(e: PosError) -> Self {
        ChangeError::Pos(e)
    }
}

/// One change spec for `ChangeSet::of`, in the start document's coordinates.
#[derive(Debug, Clone)]
pub enum ChangeSpec {
    Replace {
        from: usize,
        to: usize,
        insert: Text,
    },
    /// A whole change set over the same document.
    Set(ChangeSet),
}

impl ChangeSpec {
    /// Replace `from..to` with `insert` (split on `\r\n`, `\r` and `\n`).
    pub fn replace(from: usize, to: usize, insert: &str) -> Self {
        ChangeSpec::Replace {
            from,
            to,
            insert: Text::of(insert),
        }
    }

    pub fn insert(at: usize, insert: &str) -> Self {
        Self::replace(at, at, insert)
    }

    pub fn delete(from: usize, to: usize) -> Self {
        Self::replace(from, to, "")
    }
}

/// One change from `iter_changes`/`iter_changed_ranges`: `from_a..to_a` in the
/// old document became `from_b..to_b` (holding `inserted`) in the new one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub from_a: usize,
    pub to_a: usize,
    pub from_b: usize,
    pub to_b: usize,
    /// Empty for a `ChangeDesc`.
    pub inserted: Text,
}

/// A change's shape without its inserted text: enough to map positions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeDesc {
    sections: Vec<(usize, isize)>,
}

/// A change with its inserted text, so it can be applied and inverted.
#[derive(Clone)]
pub struct ChangeSet {
    desc: ChangeDesc,
    /// Inserted text per section index; may be shorter than `sections`.
    inserted: Vec<Text>,
}

impl Deref for ChangeSet {
    type Target = ChangeDesc;

    fn deref(&self) -> &ChangeDesc {
        &self.desc
    }
}

impl ChangeDesc {
    /// The desc of CodeMirror's `ChangeDesc.fromJSON`: the sections as given,
    /// not merged. An `ins` below -1 is refused.
    pub fn from_sections(sections: &[(usize, isize)]) -> Result<ChangeDesc, ChangeError> {
        if let Some(&(len, ins)) = sections.iter().find(|&&(_, ins)| ins < KEEP) {
            return Err(ChangeError::Malformed(format!(
                "change section ({len}, {ins}) inserts a negative length"
            )));
        }
        Ok(ChangeDesc {
            sections: sections.to_vec(),
        })
    }

    /// The sections as CodeMirror's `ChangeDesc.toJSON`: flat `len, ins`
    /// pairs with `-1` for kept sections.
    pub fn sections(&self) -> impl Iterator<Item = (usize, isize)> + '_ {
        self.sections.iter().copied()
    }

    /// The length of the document before the change.
    pub fn length(&self) -> usize {
        self.sections.iter().map(|&(len, _)| len).sum()
    }

    /// The length of the document after the change.
    pub fn new_length(&self) -> usize {
        self.sections
            .iter()
            .map(|&(len, ins)| if ins < 0 { len } else { ins as usize })
            .sum()
    }

    /// Whether the change leaves the document as it was.
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty() || (self.sections.len() == 1 && self.sections[0].1 < 0)
    }

    /// The kept ranges as `(pos_a, pos_b, len)`.
    pub fn iter_gaps(&self) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        let (mut pos_a, mut pos_b) = (0, 0);
        for &(len, ins) in &self.sections {
            if ins < 0 {
                out.push((pos_a, pos_b, len));
                pos_b += len;
            } else {
                pos_b += ins as usize;
            }
            pos_a += len;
        }
        out
    }

    /// The changed ranges; adjacent changes are joined unless `individual`.
    pub fn iter_changed_ranges(&self, individual: bool) -> ChangeIter<'_> {
        ChangeIter::new(&self.sections, None, individual)
    }

    /// The description of the inverse change.
    pub fn inverted_desc(&self) -> ChangeDesc {
        ChangeDesc {
            sections: self
                .sections
                .iter()
                .map(|&(len, ins)| {
                    if ins < 0 {
                        (len, ins)
                    } else {
                        (ins as usize, len as isize)
                    }
                })
                .collect(),
        }
    }

    /// This change followed by `other`, which applies to its result.
    ///
    /// # Panics
    /// If `other.length()` is not this change's `new_length()`.
    pub fn compose_desc(&self, other: &ChangeDesc) -> ChangeDesc {
        self.try_compose_desc(other)
            .unwrap_or_else(|e| panic!("{e}"))
    }

    /// `compose_desc`, refusing mismatched lengths.
    pub fn try_compose_desc(&self, other: &ChangeDesc) -> Result<ChangeDesc, ChangeError> {
        check_lengths(self.new_length(), other.length())?;
        Ok(if self.is_empty() {
            other.clone()
        } else if other.is_empty() {
            self.clone()
        } else {
            compose_sets(Side::desc(self), Side::desc(other))?.0
        })
    }

    /// This change moved over `other` (both on the same document), so it
    /// applies after it. With `before`, this change's insertions at the same
    /// position as `other`'s go first.
    ///
    /// # Panics
    /// If the two have different lengths.
    pub fn map_desc(&self, other: &ChangeDesc, before: bool) -> ChangeDesc {
        self.try_map_desc(other, before)
            .unwrap_or_else(|e| panic!("{e}"))
    }

    /// `map_desc`, refusing mismatched lengths.
    pub fn try_map_desc(
        &self,
        other: &ChangeDesc,
        before: bool,
    ) -> Result<ChangeDesc, ChangeError> {
        check_lengths(self.length(), other.length())?;
        Ok(if other.is_empty() {
            self.clone()
        } else {
            map_sets(Side::desc(self), other, before)?.0
        })
    }

    /// `pos` mapped through the change, `Simple` mode. `assoc < 0` keeps it
    /// before an insertion at `pos`, otherwise after.
    ///
    /// # Panics
    /// If `pos` is past `length()`.
    pub fn map_pos(&self, pos: usize, assoc: i32) -> usize {
        self.map_pos_mode(pos, assoc, MapMode::Simple)
            .expect("simple mapping always maps")
    }

    /// `pos` mapped through the change, `None` where `mode` says the position
    /// was deleted.
    ///
    /// # Panics
    /// If `pos` is past `length()`.
    pub fn map_pos_mode(&self, pos: usize, assoc: i32, mode: MapMode) -> Option<usize> {
        let (mut pos_a, mut pos_b) = (0, 0);
        for &(len, ins) in &self.sections {
            let end_a = pos_a + len;
            if ins < 0 {
                if end_a > pos {
                    return Some(pos_b + (pos - pos_a));
                }
                pos_b += len;
            } else {
                let ins = ins as usize;
                if mode != MapMode::Simple
                    && end_a >= pos
                    && match mode {
                        MapMode::TrackDel => pos_a < pos && end_a > pos,
                        MapMode::TrackBefore => pos_a < pos,
                        MapMode::TrackAfter => end_a > pos,
                        MapMode::Simple => false,
                    }
                {
                    return None;
                }
                if end_a > pos || (end_a == pos && assoc < 0 && len == 0) {
                    return Some(if pos == pos_a || assoc < 0 {
                        pos_b
                    } else {
                        pos_b + ins
                    });
                }
                pos_b += ins;
            }
            pos_a = end_a;
        }
        assert!(
            pos <= pos_a,
            "position {pos} is out of range for changeset of length {pos_a}"
        );
        Some(pos_b)
    }

    /// Whether the change touches `from..to` (boundaries inclusive).
    pub fn touches_range(&self, from: usize, to: usize) -> Touch {
        let mut pos = 0;
        for &(len, ins) in &self.sections {
            if pos > to {
                break;
            }
            let end = pos + len;
            if ins >= 0 && pos <= to && end >= from {
                return if pos < from && end > to {
                    Touch::Cover
                } else {
                    Touch::Yes
                };
            }
            pos = end;
        }
        Touch::No
    }
}

impl ChangeSet {
    /// The change that keeps a document of `length` as it is.
    pub fn empty(length: usize) -> ChangeSet {
        ChangeSet {
            desc: ChangeDesc {
                sections: if length > 0 {
                    vec![(length, KEEP)]
                } else {
                    Vec::new()
                },
            },
            inserted: Vec::new(),
        }
    }

    /// The change for `specs` on a document of `length`, as CodeMirror's
    /// `ChangeSet.of`. Specs in ascending order become one set; a spec that
    /// starts before the previous one ended (or a nested set) starts a new
    /// set, mapped over and composed onto what came before.
    pub fn of(specs: &[ChangeSpec], length: usize) -> Result<ChangeSet, ChangeError> {
        struct Builder {
            length: usize,
            sections: Vec<(usize, isize)>,
            inserted: Vec<Text>,
            pos: usize,
            total: Option<ChangeSet>,
        }
        impl Builder {
            fn add_total(&mut self, set: ChangeSet) {
                self.total = Some(match self.total.take() {
                    Some(total) => {
                        let mapped = set.map(&total, false);
                        total.compose(&mapped)
                    }
                    None => set,
                });
            }

            fn flush(&mut self, force: bool) {
                if !force && self.sections.is_empty() {
                    return;
                }
                if self.pos < self.length {
                    add_section(&mut self.sections, self.length - self.pos, KEEP, false);
                }
                let set = ChangeSet {
                    desc: ChangeDesc {
                        sections: std::mem::take(&mut self.sections),
                    },
                    inserted: std::mem::take(&mut self.inserted),
                };
                self.pos = 0;
                self.add_total(set);
            }
        }

        let mut b = Builder {
            length,
            sections: Vec::new(),
            inserted: Vec::new(),
            pos: 0,
            total: None,
        };
        for spec in specs {
            match spec {
                ChangeSpec::Set(set) => {
                    if set.length() != length {
                        return Err(ChangeError::LengthMismatch {
                            expected: length,
                            got: set.length(),
                        });
                    }
                    b.flush(false);
                    b.add_total(set.clone());
                }
                ChangeSpec::Replace { from, to, insert } => {
                    let (from, to) = (*from, *to);
                    if from > to || to > length {
                        return Err(ChangeError::InvalidRange {
                            from,
                            to,
                            len: length,
                        });
                    }
                    let ins_len = insert.len();
                    if from == to && ins_len == 0 {
                        continue;
                    }
                    if from < b.pos {
                        b.flush(false);
                    }
                    if from > b.pos {
                        add_section(&mut b.sections, from - b.pos, KEEP, false);
                    }
                    add_section(&mut b.sections, to - from, ins_len as isize, false);
                    add_insert(&mut b.inserted, &b.sections, insert);
                    b.pos = to;
                }
            }
        }
        let force = b.total.is_none();
        b.flush(force);
        Ok(b.total.expect("a forced flush sets the total"))
    }

    /// The set of CodeMirror's `ChangeSet.fromJSON`, the inverse of `parts`:
    /// the sections as given, not merged. A line holding a line break is
    /// refused, since a `Text` cannot keep it inside one line.
    pub fn from_parts(parts: &[Part]) -> Result<ChangeSet, ChangeError> {
        let mut sections = Vec::with_capacity(parts.len());
        let mut inserted = Vec::new();
        for (i, part) in parts.iter().enumerate() {
            match part {
                Part::Keep(len) => sections.push((*len, KEEP)),
                Part::Replace(len, lines) => {
                    if let Some(line) = lines.iter().find(|l| l.contains(['\n', '\r'])) {
                        return Err(ChangeError::Malformed(format!(
                            "inserted line {line:?} holds a line break"
                        )));
                    }
                    let text = Text::of(&lines.join("\n"));
                    sections.push((*len, text.len() as isize));
                    if !text.is_empty() {
                        inserted.resize(i, Text::empty());
                        inserted.push(text);
                    }
                }
            }
        }
        Ok(ChangeSet {
            desc: ChangeDesc { sections },
            inserted,
        })
    }

    /// The description of this change, without its text.
    pub fn desc(&self) -> &ChangeDesc {
        &self.desc
    }

    /// `doc` with this change applied.
    pub fn apply(&self, doc: &Text) -> Result<Text, ChangeError> {
        if self.length() != doc.len() {
            return Err(ChangeError::LengthMismatch {
                expected: self.length(),
                got: doc.len(),
            });
        }
        let mut doc = doc.clone();
        for c in self.iter_changes(false) {
            doc = doc.replace(c.from_b, c.from_b + (c.to_a - c.from_a), &c.inserted)?;
        }
        Ok(doc)
    }

    /// The change that undoes this one; `doc` is the document before it.
    pub fn invert(&self, doc: &Text) -> Result<ChangeSet, ChangeError> {
        let mut sections = self.desc.sections.clone();
        let mut inserted = Vec::new();
        let mut pos = 0;
        for (index, section) in sections.iter_mut().enumerate() {
            let (len, ins) = *section;
            if ins >= 0 {
                *section = (ins as usize, len as isize);
                inserted.resize(index, Text::empty());
                inserted.push(if len > 0 {
                    doc.slice(pos, pos + len)?
                } else {
                    Text::empty()
                });
            }
            pos += len;
        }
        Ok(ChangeSet {
            desc: ChangeDesc { sections },
            inserted,
        })
    }

    /// This change followed by `other`, which applies to its result.
    ///
    /// # Panics
    /// If `other.length()` is not this change's `new_length()`, or if `other`
    /// splits a surrogate pair this change inserted.
    pub fn compose(&self, other: &ChangeSet) -> ChangeSet {
        self.try_compose(other).unwrap_or_else(|e| panic!("{e}"))
    }

    /// `compose`, refusing mismatched lengths and a split surrogate pair.
    pub fn try_compose(&self, other: &ChangeSet) -> Result<ChangeSet, ChangeError> {
        check_lengths(self.new_length(), other.length())?;
        if self.is_empty() {
            Ok(other.clone())
        } else if other.is_empty() {
            Ok(self.clone())
        } else {
            let (desc, inserted) = compose_sets(Side::set(self), Side::set(other))?;
            Ok(ChangeSet {
                desc,
                inserted: inserted.expect("sets compose to a set"),
            })
        }
    }

    /// This change moved over `other` (both on the same document), so it
    /// applies after it. With `before`, this change's insertions at the same
    /// position as `other`'s go first.
    ///
    /// # Panics
    /// If the two have different lengths.
    pub fn map(&self, other: &ChangeDesc, before: bool) -> ChangeSet {
        self.try_map(other, before)
            .unwrap_or_else(|e| panic!("{e}"))
    }

    /// `map`, refusing mismatched lengths.
    pub fn try_map(&self, other: &ChangeDesc, before: bool) -> Result<ChangeSet, ChangeError> {
        if other.is_empty() {
            check_lengths(self.length(), other.length())?;
            Ok(self.clone())
        } else {
            self.try_map_desc(other, before)
        }
    }

    /// `map` without the shortcut for an empty `other`: the sections are
    /// always rebuilt, as CodeMirror's `ChangeSet.mapDesc`.
    ///
    /// # Panics
    /// If the two have different lengths.
    pub fn map_desc(&self, other: &ChangeDesc, before: bool) -> ChangeSet {
        self.try_map_desc(other, before)
            .unwrap_or_else(|e| panic!("{e}"))
    }

    /// `ChangeSet::map_desc`, refusing mismatched lengths.
    pub fn try_map_desc(&self, other: &ChangeDesc, before: bool) -> Result<ChangeSet, ChangeError> {
        check_lengths(self.length(), other.length())?;
        let (desc, inserted) = map_sets(Side::set(self), other, before)?;
        Ok(ChangeSet {
            desc,
            inserted: inserted.expect("a set maps to a set"),
        })
    }

    /// The changes with their inserted text; adjacent changes are joined
    /// unless `individual`.
    pub fn iter_changes(&self, individual: bool) -> ChangeIter<'_> {
        ChangeIter::new(&self.desc.sections, Some(&self.inserted), individual)
    }

    /// CodeMirror's `ChangeSet.toJSON` as parts: `Keep(len)` for a kept
    /// section, `Replace(len, lines)` for a change (no lines for a deletion).
    pub fn parts(&self) -> Vec<Part> {
        self.desc
            .sections
            .iter()
            .enumerate()
            .map(|(i, &(len, ins))| match ins {
                KEEP => Part::Keep(len),
                0 => Part::Replace(len, Vec::new()),
                _ => Part::Replace(len, lines(&self.inserted[i])),
            })
            .collect()
    }
}

/// Same sections and inserted texts (`inserted`'s padding does not count).
impl PartialEq for ChangeSet {
    fn eq(&self, other: &Self) -> bool {
        self.desc == other.desc && self.parts() == other.parts()
    }
}

impl Eq for ChangeSet {}

impl fmt::Debug for ChangeSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.parts()).finish()
    }
}

/// One section of `ChangeSet::parts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Keep(usize),
    Replace(usize, Vec<String>),
}

fn lines(text: &Text) -> Vec<String> {
    text.to_string().split('\n').map(str::to_owned).collect()
}

/// Iterates the changes of a desc or set; see `iter_changes`.
pub struct ChangeIter<'a> {
    sections: &'a [(usize, isize)],
    inserted: Option<&'a [Text]>,
    individual: bool,
    i: usize,
    pos_a: usize,
    pos_b: usize,
}

impl<'a> ChangeIter<'a> {
    fn new(sections: &'a [(usize, isize)], inserted: Option<&'a [Text]>, individual: bool) -> Self {
        ChangeIter {
            sections,
            inserted,
            individual,
            i: 0,
            pos_a: 0,
            pos_b: 0,
        }
    }
}

impl Iterator for ChangeIter<'_> {
    type Item = Change;

    fn next(&mut self) -> Option<Change> {
        while self.i < self.sections.len() {
            let (mut len, mut ins) = self.sections[self.i];
            self.i += 1;
            if ins < 0 {
                self.pos_a += len;
                self.pos_b += len;
                continue;
            }
            let (mut end_a, mut end_b) = (self.pos_a, self.pos_b);
            let mut text = Text::empty();
            loop {
                end_a += len;
                end_b += ins as usize;
                if ins > 0
                    && let Some(inserted) = self.inserted
                {
                    text = text.append(&inserted[self.i - 1]);
                }
                if self.individual || self.i == self.sections.len() || self.sections[self.i].1 < 0 {
                    break;
                }
                (len, ins) = self.sections[self.i];
                self.i += 1;
            }
            let change = Change {
                from_a: self.pos_a,
                to_a: end_a,
                from_b: self.pos_b,
                to_b: end_b,
                inserted: text,
            };
            self.pos_a = end_a;
            self.pos_b = end_b;
            return Some(change);
        }
        None
    }
}

/// Appends a section, merging it into the last one where CodeMirror does:
/// equal kept/deleted kinds add up, insertions at one point add up, and
/// `force_join` glues any piece onto the last section.
fn add_section(sections: &mut Vec<(usize, isize)>, len: usize, ins: isize, force_join: bool) {
    if len == 0 && ins <= 0 {
        return;
    }
    if let Some(last) = sections.last_mut() {
        if ins <= 0 && ins == last.1 {
            last.0 += len;
            return;
        }
        if len == 0 && last.0 == 0 {
            last.1 += ins;
            return;
        }
        if force_join {
            last.0 += len;
            last.1 += ins;
            return;
        }
    }
    sections.push((len, ins));
}

/// Records `value` as the last section's inserted text, appending to it when
/// that section already has text.
fn add_insert(values: &mut Vec<Text>, sections: &[(usize, isize)], value: &Text) {
    if value.is_empty() {
        return;
    }
    let index = sections.len() - 1;
    if index < values.len() {
        let last = values.last_mut().expect("index < len");
        *last = last.append(value);
    } else {
        values.resize(index, Text::empty());
        values.push(value.clone());
    }
}

/// A desc or set walked by `SectionIter`.
#[derive(Clone, Copy)]
struct Side<'a> {
    sections: &'a [(usize, isize)],
    inserted: Option<&'a [Text]>,
}

impl<'a> Side<'a> {
    fn desc(desc: &'a ChangeDesc) -> Self {
        Side {
            sections: &desc.sections,
            inserted: None,
        }
    }

    fn set(set: &'a ChangeSet) -> Self {
        Side {
            sections: &set.desc.sections,
            inserted: Some(&set.inserted),
        }
    }
}

/// A cursor over sections that can stop part-way through one: `off` units of
/// the current section are consumed (old units via `forward`, new via
/// `forward2`), and `len`/`ins` are what is left.
struct SectionIter<'a> {
    side: Side<'a>,
    /// Index of the next section; the current one is `i - 1`.
    i: usize,
    len: usize,
    ins: isize,
    off: usize,
}

impl<'a> SectionIter<'a> {
    fn new(side: Side<'a>) -> Self {
        let mut it = SectionIter {
            side,
            i: 0,
            len: 0,
            ins: 0,
            off: 0,
        };
        it.next();
        it
    }

    fn next(&mut self) {
        if self.i < self.side.sections.len() {
            (self.len, self.ins) = self.side.sections[self.i];
            self.i += 1;
        } else {
            self.len = 0;
            self.ins = DONE;
        }
        self.off = 0;
    }

    fn done(&self) -> bool {
        self.ins == DONE
    }

    /// What is left of the current section in the new document.
    fn len2(&self) -> usize {
        if self.ins < 0 {
            self.len
        } else {
            self.ins as usize
        }
    }

    fn text(&self) -> Text {
        let index = self.i - 1;
        match self.side.inserted {
            Some(inserted) if index < inserted.len() => inserted[index].clone(),
            _ => Text::empty(),
        }
    }

    /// `len` units of the current insertion, from `off`.
    fn text_bit(&self, len: usize) -> Result<Text, PosError> {
        let index = self.i - 1;
        match self.side.inserted {
            Some(inserted) if index < inserted.len() => {
                inserted[index].slice(self.off, self.off + len)
            }
            _ => Ok(Text::empty()),
        }
    }

    fn forward(&mut self, len: usize) {
        if len == self.len {
            self.next();
        } else {
            self.len -= len;
            self.off += len;
        }
    }

    fn forward2(&mut self, len: usize) {
        if self.ins == KEEP {
            self.forward(len);
        } else if len as isize == self.ins {
            self.next();
        } else {
            self.ins -= len as isize;
            self.off += len;
        }
    }
}

type Built = (ChangeDesc, Option<Vec<Text>>);

/// `LengthMismatch` unless `expected == got`. Checked before every walk:
/// `map_sets` loops forever on a longer `set_b`, as CodeMirror's `mapSet` does.
fn check_lengths(expected: usize, got: usize) -> Result<(), ChangeError> {
    if expected == got {
        Ok(())
    } else {
        Err(ChangeError::LengthMismatch { expected, got })
    }
}

fn built(sections: Vec<(usize, isize)>, insert: Option<Vec<Text>>) -> Built {
    (ChangeDesc { sections }, insert)
}

/// Where CodeMirror throws "Mismatched change set lengths" mid-walk: lengths
/// that agree in total but sections that do not line up.
fn misaligned() -> ChangeError {
    ChangeError::Malformed("mismatched change set lengths: the sections do not line up".into())
}

/// `set_a` moved to apply after `set_b` (both on the same document); see
/// CodeMirror's `mapSet`. `inserted` is the section index of the change in A
/// whose text is already placed, for changes processed piece by piece.
fn map_sets(set_a: Side<'_>, set_b: &ChangeDesc, before: bool) -> Result<Built, ChangeError> {
    let mut sections = Vec::new();
    let mut insert = set_a.inserted.map(|_| Vec::new());
    let mut a = SectionIter::new(set_a);
    let mut b = SectionIter::new(Side::desc(set_b));
    // CodeMirror starts this at -1 and compares with its pair index; with
    // section indices offset by one, 0 is "none yet".
    let mut inserted = 0usize;
    loop {
        if (a.done() && b.len > 0) || (b.done() && a.len > 0) {
            return Err(misaligned());
        } else if a.ins == KEEP && b.ins == KEEP {
            // Move across ranges skipped by both sets.
            let len = a.len.min(b.len);
            add_section(&mut sections, len, KEEP, false);
            a.forward(len);
            b.forward(len);
        } else if b.ins >= 0
            && (a.ins < 0
                || inserted == a.i
                || (a.off == 0 && (b.len < a.len || (b.len == a.len && !before))))
        {
            // A change in B that comes first: skip it, placing any change in
            // A it covers.
            let mut len = b.len;
            add_section(&mut sections, b.ins as usize, KEEP, false);
            while len > 0 {
                let piece = a.len.min(len);
                if a.ins >= 0 && inserted < a.i && a.len <= piece {
                    add_section(&mut sections, 0, a.ins, false);
                    if let Some(insert) = &mut insert {
                        add_insert(insert, &sections, &a.text());
                    }
                    inserted = a.i;
                }
                a.forward(piece);
                len -= piece;
            }
            b.next();
        } else if a.ins >= 0 {
            // A change in A, up to the start of the next non-deletion change
            // in B (if they overlap).
            let (mut len, mut left) = (0, a.len);
            while left > 0 {
                if b.ins == KEEP {
                    let piece = left.min(b.len);
                    len += piece;
                    left -= piece;
                    b.forward(piece);
                } else if b.ins == 0 && b.len < left {
                    left -= b.len;
                    b.next();
                } else {
                    break;
                }
            }
            add_section(
                &mut sections,
                len,
                if inserted < a.i { a.ins } else { 0 },
                false,
            );
            if inserted < a.i
                && let Some(insert) = &mut insert
            {
                add_insert(insert, &sections, &a.text());
            }
            inserted = a.i;
            a.forward(a.len - left);
        } else if a.done() && b.done() {
            return Ok(built(sections, insert));
        } else {
            return Err(misaligned());
        }
    }
}

/// `set_a` followed by `set_b`; see CodeMirror's `composeSets`. `open` glues
/// the pieces of one change that spans several iterations into one section.
fn compose_sets(set_a: Side<'_>, set_b: Side<'_>) -> Result<Built, ChangeError> {
    let mut sections = Vec::new();
    let mut insert = set_a.inserted.and(set_b.inserted).map(|_| Vec::new());
    let mut a = SectionIter::new(set_a);
    let mut b = SectionIter::new(set_b);
    let mut open = false;
    loop {
        if a.done() && b.done() {
            return Ok(built(sections, insert));
        } else if a.ins == 0 {
            // Deletion in A.
            add_section(&mut sections, a.len, 0, open);
            a.next();
        } else if b.len == 0 && !b.done() {
            // Insertion in B.
            add_section(&mut sections, 0, b.ins, open);
            if let Some(insert) = &mut insert {
                add_insert(insert, &sections, &b.text());
            }
            b.next();
        } else if a.done() || b.done() {
            return Err(misaligned());
        } else {
            let len = a.len2().min(b.len);
            let section_len = sections.len();
            if a.ins == KEEP {
                let ins_b = if b.ins == KEEP {
                    KEEP
                } else if b.off > 0 {
                    0
                } else {
                    b.ins
                };
                add_section(&mut sections, len, ins_b, open);
                if ins_b != 0
                    && let Some(insert) = &mut insert
                {
                    add_insert(insert, &sections, &b.text());
                }
            } else if b.ins == KEEP {
                add_section(
                    &mut sections,
                    if a.off > 0 { 0 } else { a.len },
                    len as isize,
                    open,
                );
                if let Some(insert) = &mut insert {
                    add_insert(insert, &sections, &a.text_bit(len)?);
                }
            } else {
                add_section(
                    &mut sections,
                    if a.off > 0 { 0 } else { a.len },
                    if b.off > 0 { 0 } else { b.ins },
                    open,
                );
                if b.off == 0
                    && let Some(insert) = &mut insert
                {
                    add_insert(insert, &sections, &b.text());
                }
            }
            open = (a.ins > len as isize || (b.ins >= 0 && b.len > len))
                && (open || sections.len() > section_len);
            a.forward2(len);
            b.forward(len);
        }
    }
}
