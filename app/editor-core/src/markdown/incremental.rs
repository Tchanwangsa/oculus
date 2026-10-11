//! Incremental reparse: after a change, keep the top-level blocks the change
//! cannot have affected, parse from just before it, and stop as soon as the
//! parse reaches a line where an old top-level block began, past the change,
//! in the same state. The result always equals a fresh parse.
//!
//! A top-level block's parse reads up to one line past the line where the
//! next top-level block starts (the table delimiter peek); so the parse
//! restarts at the start line of the last top-level block whose start line
//! plus two lies before the change. Every top-level block starts on a line
//! the parser began with only the `Document` open, so restarting there with
//! a fresh stack is exact, and so is reusing the old tail from such a line,
//! except the first line: only there can frontmatter open, so an old block
//! from line 1 is never reused elsewhere. Frontmatter reads ahead up to
//! `FRONTMATTER_SCAN` units, so a change there with a `---` first line
//! restarts at 0 and resyncs after the change as usual, unless the change
//! comes after an existing frontmatter block, which then stays.

use super::NodeType;
use super::block::BlockContext;
use super::chars::{self, Utf16Map};
use super::tree::Tree;
use crate::text::{ChangeSet, Text};

/// The frontmatter scan (`frontmatter.ts`'s `SCAN_MAX`), in UTF-16 units.
const FRONTMATTER_SCAN: usize = 64 * 1024;

/// The tree of `doc`, the result of applying `changes` to the document
/// `old` was parsed from. Equal to `parse(doc)`.
pub fn reparse(old: &Tree, doc: &Text, changes: &ChangeSet) -> Tree {
    assert_eq!(
        old.root().to(),
        changes.length(),
        "the tree is not of the changed document"
    );
    assert_eq!(
        doc.len(),
        changes.new_length(),
        "the document is not the changed one"
    );
    if changes.is_empty() {
        return old.clone();
    }
    // The changed region as one span: old from_a..to_a became from_a..to_b.
    let (mut from_a, mut to_a, mut to_b) = (usize::MAX, 0, 0);
    for change in changes.iter_changes(false) {
        from_a = from_a.min(change.from_a);
        to_a = to_a.max(change.to_a);
        to_b = to_b.max(change.to_b);
    }
    let shift = doc.len() as isize - old.root().to() as isize;

    let top = old.top_level();
    // Frontmatter reads its whole window: with a `---` first line, a change
    // in it (or to the window's length) can change the first block, so the
    // parse restarts at 0, unless the old first block is frontmatter:
    // `restart_point` keeps that only when a later block starts before the
    // change, so the change is past its closing fence, which stays the first.
    let frontmatter_kept = top.first().is_some_and(|&(index, _, _)| {
        old.node(index)
            .is_some_and(|n| n.kind() == NodeType::Frontmatter)
    });
    let (prefix_end, restart) = if from_a <= FRONTMATTER_SCAN
        && !frontmatter_kept
        && doc.line(1).is_some_and(|line| line.text == "---")
    {
        (1, 0)
    } else {
        restart_point(&top, doc, from_a)
    };
    let tail = doc.slice_string(restart, doc.len()).expect("a line start");

    // Old top-level blocks starting at or after the change, as reuse points.
    let mut next = top.partition_point(|&(_, from, _)| from < to_a);
    // The old line start of a new line at `to_b` is known only when the
    // change was a pure insertion after a line break (or at 0).
    let edge_is_line_start = from_a == to_a
        && (from_a == 0
            || doc
                .slice_string(from_a - 1, from_a)
                .is_ok_and(|s| s == "\n"));
    let mut cursor = (0usize, restart);
    let mut reuse = None;
    let partial = BlockContext::resume(&tail, restart == 0).parse_until(|line_start, text| {
        let bytes = tail.as_bytes();
        cursor.1 += bytes[cursor.0..line_start]
            .iter()
            .map(|&b| chars::units(b))
            .sum::<usize>();
        cursor.0 = line_start;
        let pos = cursor.1;
        // The old block must not come from the document's first line.
        if pos < to_b || (pos == to_b && !edge_is_line_start) || pos as isize - shift <= 0 {
            return false;
        }
        let line_end = pos + text.encode_utf16().count();
        let new_pos = |i: usize| (top[i].1 as isize + shift) as usize;
        while next < top.len() && new_pos(next) < pos {
            next += 1;
        }
        if next < top.len() && new_pos(next) <= line_end {
            reuse = Some(top[next].0);
            return true;
        }
        false
    });

    let parsed = partial.stopped_at.unwrap_or(tail.len());
    let map = Utf16Map::new(&tail[..parsed]);
    old.splice(
        prefix_end,
        &partial.children,
        |byte| restart + map.get(byte),
        reuse.map(|index| (index, shift)),
        doc.len(),
    )
}

/// Where to restart: the index of the first old node not kept (the first
/// reparsed top-level block, or 1 with none kept) and the UTF-16 position
/// of its line start.
fn restart_point(top: &[(usize, usize, usize)], doc: &Text, from_a: usize) -> (usize, usize) {
    let mut k = top.partition_point(|&(_, from, _)| from < from_a);
    while k > 0 {
        k -= 1;
        let (index, from, _) = top[k];
        let line = doc.line_at(from).expect("a position before the change");
        if doc
            .line(line.number + 2)
            .is_some_and(|after| after.from <= from_a)
        {
            return (index, line.from);
        }
    }
    (1, 0)
}
