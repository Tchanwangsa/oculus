//! Replacing part of a slot's source: the one primitive every edit goes
//! through, so the bare-argument and control-word rules hold everywhere.

use core::ops::Range;

use super::glue::glue;
use crate::{
    field::Field,
    slot::{Bounds, Slot, SlotId, SlotKind},
};

/// The source after a splice, and where the inserted text landed in it.
pub struct Spliced {
    pub source: String,
    /// Where the inserted text's first byte is.
    pub at: usize,
    /// Where the inserted text ends.
    pub end: usize,
}

/// `range` of `slot`'s content replaced with `insert`.
///
/// A bare argument (`x^2`, `\frac ab`) is one token: if it would hold
/// anything but a single letter or digit afterwards, its content gets
/// braces (`x^{23}`), and emptied it becomes `{}`, never `x^`. A spaced
/// cell keeps its spaces (`cell_space`).
pub fn splice(field: &Field, slot: SlotId, range: Range<usize>, insert: &str) -> Spliced {
    let src = field.source();
    let slot = field.stops().slot(slot);
    if slot.bounds != Bounds::Bare {
        let (range, lead, pad) = cell_space(src, slot, range, insert);
        let mut spliced = plain(src, range, &format!("{lead}{insert}{pad}"));
        spliced.at += lead.len();
        spliced.end -= pad.len();
        return spliced;
    }
    let interior = slot.interior.clone();
    let head = &src[interior.start..range.start];
    let tail = &src[range.end..interior.end];
    let glued = glue(head, insert, tail, !range.is_empty());
    let head = &head[..head.len() - glued.drop];
    let content = format!("{head}{}{tail}", glued.text);
    let one_token = {
        let mut chars = content.chars();
        chars.next().is_some_and(char::is_alphanumeric) && chars.next().is_none()
    };
    let (open, close) = if one_token { ("", "") } else { ("{", "}") };
    let replacement = format!("{open}{content}{close}");
    let at_in = open.len() + head.len() + glued.lead;
    let outer = plain(src, interior, &replacement);
    let at = outer.at + at_in;
    Spliced {
        source: outer.source,
        at,
        end: at + insert.len(),
    }
}

/// An empty cell's caret sits against what follows it (`a & |& b`, or
/// the next line in `a &`, newline, `\end`). Text typed there takes a
/// space before the `&` or `\\` after it when the cell is spaced (`a & c &
/// b`), and stays on the `&`'s line (`a & c`, newline). Deleting a cell's
/// last atom takes those spaces back. A tight cell (`a&|&b`) stays tight.
/// Returns the range to replace and the spaces to write before and after
/// `insert`.
fn cell_space(
    src: &str,
    slot: &Slot,
    range: Range<usize>,
    insert: &str,
) -> (Range<usize>, &'static str, &'static str) {
    let separator = |text: &str| text.starts_with('&') || text.starts_with(r"\\");
    let before = &src[..range.start];
    let after = &src[range.end..];
    if !matches!(slot.kind, SlotKind::Cell { .. }) || !before.ends_with(char::is_whitespace) {
        return (range, "", "");
    }
    if slot.is_empty() && !insert.is_empty() {
        let line = before.trim_end();
        if line.ends_with('&') && before[line.len()..].contains('\n') {
            return (line.len()..line.len(), " ", "");
        }
        if separator(after) {
            return (range, "", " ");
        }
    }
    let emptied = insert.is_empty() && range == slot.interior && !slot.is_empty();
    if emptied && after.starts_with(' ') && separator(&after[1..]) {
        return (range.start..range.end + 1, "", "");
    }
    if emptied && before.ends_with("& ") && after.starts_with('\n') {
        return (range.start - 1..range.end, "", "");
    }
    (range, "", "")
}

/// `range` replaced with `insert`, glued.
fn plain(src: &str, range: Range<usize>, insert: &str) -> Spliced {
    let before = &src[..range.start];
    let after = &src[range.end..];
    let glued = glue(before, insert, after, !range.is_empty());
    let from = range.start - glued.drop;
    let source = format!("{}{}{after}", &src[..from], glued.text);
    let at = from + glued.lead;
    Spliced {
        source,
        at,
        end: at + insert.len(),
    }
}
