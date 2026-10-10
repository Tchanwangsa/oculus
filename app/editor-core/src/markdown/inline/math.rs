//! The app's inline maths (`mathSyntax.ts`): `$…$` and `\(…\)`.

use super::{InlineContext, Part};
use crate::markdown::tables::NodeType as T;
use crate::markdown::tree::Elt;

fn is_space(c: i32) -> bool {
    c == 32 || c == 9 || c == 10 || c == 13
}

/// `$…$` with pandoc's rules: no space just inside either `$`, no digit
/// right after the closer, `$$` never inline.
pub(super) fn dollar_math(cx: &mut InlineContext, next: i32, pos: usize) -> Option<usize> {
    const DOLLAR: i32 = b'$' as i32;
    if next != DOLLAR {
        return None;
    }
    if cx.char(pos + 1) == DOLLAR || (pos > 0 && cx.char(pos - 1) == DOLLAR) {
        return None;
    }
    let first = cx.char(pos + 1);
    if first < 0 || is_space(first) {
        return None;
    }
    let mut i = pos + 1;
    while i < cx.end() {
        let c = cx.char(i);
        if c == b'\n' as i32 {
            return None;
        }
        if c == b'\\' as i32 {
            i += 2;
            continue;
        }
        if c == DOLLAR
            && !is_space(cx.char(i - 1))
            && !(b'0' as i32..=b'9' as i32).contains(&cx.char(i + 1))
        {
            if cx.char(i + 1) == DOLLAR {
                return None;
            }
            return cx.append(Part::Elt(Elt::with(
                T::InlineMath,
                pos,
                i + 1,
                vec![
                    Elt::new(T::MathMark, pos, pos + 1),
                    Elt::new(T::MathMark, i, i + 1),
                ],
            )));
        }
        i += 1;
    }
    None
}

/// `\(…\)` on one line, empty included; runs before `Escape`.
pub(super) fn paren_math(cx: &mut InlineContext, next: i32, pos: usize) -> Option<usize> {
    if next != b'\\' as i32 || cx.char(pos + 1) != b'(' as i32 {
        return None;
    }
    let mut i = pos + 2;
    while i + 1 < cx.end() {
        let c = cx.char(i);
        if c == b'\n' as i32 {
            return None;
        }
        if c == b'\\' as i32 && cx.char(i + 1) == b')' as i32 {
            return cx.append(Part::Elt(Elt::with(
                T::InlineMath,
                pos,
                i + 2,
                vec![
                    Elt::new(T::MathMark, pos, pos + 2),
                    Elt::new(T::MathMark, i, i + 2),
                ],
            )));
        }
        i += 1;
    }
    None
}
