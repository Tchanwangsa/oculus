//! Replacing part of a slot's source: the one primitive every edit goes
//! through, so the bare-argument and control-word rules hold everywhere.

use core::ops::Range;

use super::glue::glue;
use crate::{field::Field, slot::Bounds, slot::SlotId};

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
/// braces (`x^{23}`), and emptied it becomes `{}`, never `x^`.
pub fn splice(field: &Field, slot: SlotId, range: Range<usize>, insert: &str) -> Spliced {
    let src = field.source();
    let slot = field.stops().slot(slot);
    if slot.bounds != Bounds::Bare {
        return plain(src, range, insert);
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
