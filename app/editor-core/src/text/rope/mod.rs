//! The document, mirroring `@codemirror/state`'s `Text`. Positions at this API
//! are UTF-16 code units, as in CodeMirror; the tree underneath walks bytes and
//! converts through its cached summaries. Stored text has `\n` breaks only.
//!
//! CodeMirror clamps out-of-range positions in `replace`/`slice`/`sliceString`;
//! this API refuses them, and positions inside a surrogate pair, with `PosError`.

mod iter;
#[cfg(test)]
mod tests;
mod tree;

use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

pub use iter::{Chunks, Iter, Lines};
use tree::{Bias, Cursor, Dim, Node, Release, Sizes, Summary};

/// A position or range the document refuses. It never clamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PosError {
    /// Past the end of a document of `len` units.
    OutOfRange { pos: usize, len: usize },
    /// Between the two halves of a surrogate pair.
    InsideSurrogate { pos: usize },
    /// A range whose start is after its end.
    Reversed { from: usize, to: usize },
}

impl fmt::Display for PosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            PosError::OutOfRange { pos, len } => {
                write!(
                    f,
                    "position {pos} is past the end of a document of {len} units"
                )
            }
            PosError::InsideSurrogate { pos } => {
                write!(f, "position {pos} is inside a surrogate pair")
            }
            PosError::Reversed { from, to } => write!(f, "range {from}..{to} is reversed"),
        }
    }
}

impl std::error::Error for PosError {}

/// One line, numbered from 1. `from` and `to` are UTF-16 units; `to` is before
/// the line break (or the document end).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub number: usize,
    pub from: usize,
    pub to: usize,
    pub text: String,
}

impl Line {
    /// Length in UTF-16 units.
    pub fn len(&self) -> usize {
        self.to - self.from
    }

    pub fn is_empty(&self) -> bool {
        self.from == self.to
    }
}

/// An immutable document. Cloning is a pointer clone; edits return a new `Text`
/// sharing every subtree they did not touch. `S` sets the node sizes; only
/// tests use anything but the default.
pub struct Text<S: Sizes = Release> {
    root: Arc<Node>,
    sizes: PhantomData<S>,
}

impl<S: Sizes> Clone for Text<S> {
    fn clone(&self) -> Self {
        Self::wrap(self.root.clone())
    }
}

impl Text {
    /// The empty document (one empty line).
    pub fn empty() -> Text {
        Text::empty_in()
    }

    /// The document for `input`, splitting lines on `\r\n`, `\r` and `\n` and
    /// joining them with `\n` — CodeMirror's `Text.of(input.split(/\r\n?|\n/))`.
    pub fn of(input: &str) -> Text {
        Text::of_in(input)
    }
}

impl<S: Sizes> Text<S> {
    fn wrap(root: Arc<Node>) -> Self {
        Text {
            root,
            sizes: PhantomData,
        }
    }

    /// `Text::empty` at any node sizes.
    pub(crate) fn empty_in() -> Self {
        Self::wrap(tree::leaf(String::new()))
    }

    /// `Text::of` at any node sizes.
    pub(crate) fn of_in(input: &str) -> Self {
        if !input.contains('\r') {
            return Self::from_normalised(input);
        }
        let mut text = String::with_capacity(input.len());
        let mut rest = input;
        while let Some(i) = rest.find('\r') {
            text.push_str(&rest[..i]);
            text.push('\n');
            rest = &rest[i + 1..];
            if let Some(after) = rest.strip_prefix('\n') {
                rest = after;
            }
        }
        text.push_str(rest);
        Self::from_normalised(&text)
    }

    fn from_normalised(text: &str) -> Self {
        Self::wrap(tree::build::<S>(text))
    }

    /// Length in UTF-16 units.
    pub fn len(&self) -> usize {
        self.root.summary.utf16
    }

    pub fn is_empty(&self) -> bool {
        self.root.summary.bytes == 0
    }

    /// The number of lines; at least 1.
    pub fn lines(&self) -> usize {
        self.root.summary.lines + 1
    }

    /// Line `number`, counted from 1; `None` outside `1..=lines()`.
    pub fn line(&self, number: usize) -> Option<Line> {
        if number == 0 || number > self.lines() {
            return None;
        }
        let start = self.after_newline(number - 1);
        let end = if number == self.lines() {
            self.root.summary
        } else {
            // The break itself is one byte, one unit and one newline.
            self.after_newline(number)
                - Summary {
                    bytes: 1,
                    utf16: 1,
                    lines: 1,
                }
        };
        let mut text = String::with_capacity(end.bytes - start.bytes);
        self.push_bytes(start.bytes, end.bytes, &mut text);
        Some(Line {
            number,
            from: start.utf16,
            to: end.utf16,
            text,
        })
    }

    /// The line containing `pos`. A position at a line's end belongs to that
    /// line, not the next, as in CodeMirror's `lineAt`.
    pub fn line_at(&self, pos: usize) -> Result<Line, PosError> {
        let at = self.at_utf16(pos)?;
        Ok(self
            .line(at.lines + 1)
            .expect("line of an in-range position"))
    }

    /// The document between `from` and `to`, sharing subtrees with this one.
    pub fn slice(&self, from: usize, to: usize) -> Result<Self, PosError> {
        let (a, b) = self.byte_range(from, to)?;
        Ok(self.slice_bytes(a, b))
    }

    /// The text between `from` and `to`, with `\n` breaks.
    pub fn slice_string(&self, from: usize, to: usize) -> Result<String, PosError> {
        let (a, b) = self.byte_range(from, to)?;
        let mut out = String::with_capacity(b - a);
        self.push_bytes(a, b, &mut out);
        Ok(out)
    }

    /// This document with `from..to` replaced by `text`.
    pub fn replace(&self, from: usize, to: usize, text: &Self) -> Result<Self, PosError> {
        let (a, b) = self.byte_range(from, to)?;
        if text.root.height == 0 {
            return Ok(self.splice_bytes(a, b, text.root.leaf_text()));
        }
        let total = self.root.summary.bytes;
        Ok(self
            .slice_bytes(0, a)
            .append(text)
            .append(&self.slice_bytes(b, total)))
    }

    /// This document followed by `other`.
    pub fn append(&self, other: &Self) -> Self {
        if other.is_empty() {
            return self.clone();
        }
        if self.is_empty() {
            return other.clone();
        }
        // A lone leaf may be under `MIN_CHUNK`; splicing re-chunks it with its
        // neighbour so every leaf of a multi-leaf tree stays at least that big.
        if self.root.height == 0 && self.root.summary.bytes < S::MIN_CHUNK {
            return other.splice_bytes(0, 0, self.root.leaf_text());
        }
        let end = self.root.summary.bytes;
        if other.root.height == 0 && other.root.summary.bytes < S::MIN_CHUNK {
            return self.splice_bytes(end, end, other.root.leaf_text());
        }
        Self::wrap(tree::join::<S>(&self.root, &other.root))
    }

    /// The document's text as runs that never contain a break, with each line
    /// break yielded on its own as `"\n"` (CodeMirror's `iter()` with
    /// `lineBreak`). How runs are cut depends on the tree, not on the text.
    pub fn iter(&self) -> Iter<'_> {
        Iter::new(Chunks::new(&self.root, 0, self.root.summary.bytes, true))
    }

    /// `iter`, last run first; each run's text still reads forwards.
    pub fn iter_rev(&self) -> Iter<'_> {
        Iter::new(Chunks::new(&self.root, 0, self.root.summary.bytes, false))
    }

    /// `iter` over `from..to`; backwards over `to..from` when `from > to`, as in
    /// CodeMirror's `iterRange`.
    pub fn iter_range(&self, from: usize, to: usize) -> Result<Iter<'_>, PosError> {
        let (lo, hi) = self.byte_range(from.min(to), from.max(to))?;
        Ok(Iter::new(Chunks::new(&self.root, lo, hi, from <= to)))
    }

    /// The text of lines `from..to` (numbers from 1, `to` exclusive), as
    /// CodeMirror's `iterLines`: it yields the range's text split on `\n`, so an
    /// empty range yields one empty line. `None` if `from` is not a line or
    /// `to > lines() + 1`.
    pub fn iter_lines(&self, from: usize, to: usize) -> Option<Lines<'_>> {
        if from == 0 || from > self.lines() || to > self.lines() + 1 {
            return None;
        }
        let start = self.after_newline(from - 1).bytes;
        let end = if to == self.lines() + 1 {
            self.root.summary.bytes
        } else if to <= 1 {
            0
        } else {
            self.line_end_bytes(to - 1)
        };
        Some(Lines::new(Iter::new(Chunks::new(
            &self.root,
            start,
            end.max(start),
            true,
        ))))
    }

    /// The stored chunks in order, line breaks included. Their boundaries
    /// depend on the tree, not on the text.
    pub fn chunks(&self) -> Chunks<'_> {
        Chunks::new(&self.root, 0, self.root.summary.bytes, true)
    }

    /// The prefix summary at UTF-16 position `pos`.
    fn at_utf16(&self, pos: usize) -> Result<Summary, PosError> {
        if pos > self.len() {
            return Err(PosError::OutOfRange {
                pos,
                len: self.len(),
            });
        }
        let cursor = Cursor::seek(&self.root, Dim::Utf16, pos, Bias::Left);
        let mut at = cursor.start;
        for c in cursor.text().chars() {
            if at.utf16 >= pos {
                break;
            }
            at.utf16 += c.len_utf16();
            at.bytes += c.len_utf8();
            at.lines += (c == '\n') as usize;
        }
        if at.utf16 == pos {
            Ok(at)
        } else {
            Err(PosError::InsideSurrogate { pos })
        }
    }

    fn byte_range(&self, from: usize, to: usize) -> Result<(usize, usize), PosError> {
        let a = self.at_utf16(from)?;
        let b = self.at_utf16(to)?;
        if from > to {
            return Err(PosError::Reversed { from, to });
        }
        Ok((a.bytes, b.bytes))
    }

    /// The prefix summary just after the `k`th `\n` (the start of line `k + 1`).
    fn after_newline(&self, k: usize) -> Summary {
        if k == 0 {
            return Summary::default();
        }
        let cursor = Cursor::seek(&self.root, Dim::Lines, k, Bias::Left);
        let text = cursor.text();
        let nth = k - cursor.start.lines;
        let (i, _) = text
            .match_indices('\n')
            .nth(nth - 1)
            .expect("newline in the seeked leaf");
        cursor.start + Summary::of(&text[..=i])
    }

    /// The byte offset of line `number`'s end, before its break.
    fn line_end_bytes(&self, number: usize) -> usize {
        if number == self.lines() {
            self.root.summary.bytes
        } else {
            self.after_newline(number).bytes - 1
        }
    }

    fn push_bytes(&self, from: usize, to: usize, out: &mut String) {
        for chunk in Chunks::new(&self.root, from, to, true) {
            out.push_str(chunk);
        }
    }

    fn slice_bytes(&self, a: usize, b: usize) -> Self {
        if b - a <= S::MAX_CHUNK {
            let mut text = String::with_capacity(b - a);
            self.push_bytes(a, b, &mut text);
            return Self::from_normalised(&text);
        }
        let first = Cursor::seek(&self.root, Dim::Bytes, a, Bias::Right);
        let last = Cursor::seek(&self.root, Dim::Bytes, b, Bias::Left);
        let (inner_from, inner_to) = (first.end().bytes, last.start.bytes);
        if inner_from >= inner_to {
            // At most two leaves: copying is cheaper than sharing.
            let mut text = String::with_capacity(b - a);
            self.push_bytes(a, b, &mut text);
            return Self::from_normalised(&text);
        }
        let before = tree::take_before::<S>(&self.root, inner_to).expect("non-empty prefix");
        let inner =
            Self::wrap(tree::take_after::<S>(&before, inner_from).expect("non-empty middle"));
        let head = Self::from_normalised(&first.text()[a - first.start.bytes..]);
        let tail = Self::from_normalised(&last.text()[..b - last.start.bytes]);
        head.append(&inner).append(&tail)
    }

    /// Replaces bytes `from..to` with `insert` (already `\n`-normalised). The
    /// leaves around the edit are rebuilt together with the insert; an undersized
    /// result absorbs a neighbouring leaf so no leaf falls under `MIN_CHUNK`.
    fn splice_bytes(&self, from: usize, to: usize, insert: &str) -> Self {
        let root = &self.root;
        let total = root.summary.bytes;
        let mut l = Cursor::seek(root, Dim::Bytes, from, Bias::Left).start.bytes;
        let mut r = Cursor::seek(root, Dim::Bytes, to, Bias::Right).end().bytes;
        if (from - l) + insert.len() + (r - to) < S::MIN_CHUNK {
            if l > 0 {
                l = Cursor::seek(root, Dim::Bytes, l, Bias::Left).start.bytes;
            } else if r < total {
                r = Cursor::seek(root, Dim::Bytes, r, Bias::Right).end().bytes;
            }
        }
        let mut middle = String::with_capacity((from - l) + insert.len() + (r - to));
        self.push_bytes(l, from, &mut middle);
        middle.push_str(insert);
        self.push_bytes(to, r, &mut middle);

        let mut out = tree::take_before::<S>(root, l);
        let pieces = [
            (!middle.is_empty()).then(|| tree::build::<S>(&middle)),
            tree::take_after::<S>(root, r),
        ];
        for piece in pieces.into_iter().flatten() {
            out = Some(match out {
                Some(left) => tree::join::<S>(&left, &piece),
                None => piece,
            });
        }
        out.map_or_else(Self::empty_in, Self::wrap)
    }
}

impl Default for Text {
    fn default() -> Text {
        Text::empty()
    }
}

/// Content equality, as CodeMirror's `eq`: tree shape does not matter.
impl<S: Sizes> PartialEq for Text<S> {
    fn eq(&self, other: &Self) -> bool {
        if Arc::ptr_eq(&self.root, &other.root) {
            return true;
        }
        if self.root.summary != other.root.summary {
            return false;
        }
        // Chunk boundaries differ between trees, so compare as byte streams.
        let (mut a, mut b) = (
            self.chunks().map(str::as_bytes),
            other.chunks().map(str::as_bytes),
        );
        let (mut x, mut y): (&[u8], &[u8]) = (&[], &[]);
        loop {
            if x.is_empty() {
                x = a.next().unwrap_or(&[]);
            }
            if y.is_empty() {
                y = b.next().unwrap_or(&[]);
            }
            if x.is_empty() || y.is_empty() {
                return x.is_empty() && y.is_empty();
            }
            let n = x.len().min(y.len());
            if x[..n] != y[..n] {
                return false;
            }
            (x, y) = (&x[n..], &y[n..]);
        }
    }
}

impl<S: Sizes> Eq for Text<S> {}

impl<S: Sizes> fmt::Display for Text<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.chunks().try_for_each(|chunk| f.write_str(chunk))
    }
}

impl<S: Sizes> fmt::Debug for Text<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Text({:?})", self.to_string())
    }
}
