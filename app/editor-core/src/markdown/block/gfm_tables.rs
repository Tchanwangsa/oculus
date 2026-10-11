//! GFM tables: row and delimiter-line parsing for the `Table` leaf parser.

use crate::markdown::chars;
use crate::markdown::inline::parse_inline;
use crate::markdown::tables::NodeType as T;
use crate::markdown::tree::Elt;

/// `parseRow`: the cell count of `line` from `start`, pushing the cells and
/// pipes into `elts` when given (`offset` is `line`'s document position).
pub(super) fn parse_row(
    line: &str,
    start: usize,
    mut elts: Option<&mut Vec<Elt>>,
    offset: usize,
) -> usize {
    let b = line.as_bytes();
    let (mut count, mut first, mut cell_start, mut cell_end, mut esc) =
        (0, true, None::<usize>, 0, false);
    let push_cell = |elts: &mut Vec<Elt>, cs: usize, ce: usize| {
        elts.push(Elt::with(
            T::TableCell,
            offset + cs,
            offset + ce,
            parse_inline(&line[cs..ce], offset + cs),
        ));
    };
    let mut i = start;
    while i < b.len() {
        let next = b[i];
        if next == b'|' && !esc {
            if !first || cell_start.is_some() {
                count += 1;
            }
            first = false;
            if let Some(elts) = elts.as_deref_mut() {
                if let Some(cs) = cell_start {
                    push_cell(elts, cs, cell_end);
                }
                elts.push(Elt::new(T::TableDelimiter, i + offset, i + offset + 1));
            }
            cell_start = None;
        } else if esc || (next != b' ' && next != b'\t') {
            if cell_start.is_none() {
                cell_start = Some(i);
            }
            cell_end = i + 1;
        }
        esc = !esc && next == b'\\';
        i += 1;
    }
    if let Some(cs) = cell_start {
        count += 1;
        if let Some(elts) = elts {
            push_cell(elts, cs, cell_end);
        }
    }
    count
}

/// `hasPipe`: an unescaped `|` in `s` from `start`.
pub(super) fn has_pipe(s: &str, start: usize) -> bool {
    let b = s.as_bytes();
    let mut i = start;
    while i < b.len() {
        match b[i] {
            b'|' => return true,
            b'\\' => i += 1,
            _ => {}
        }
        i += 1;
    }
    false
}

/// `/^[>\s]*\|?(\s*:?-+:?\s*\|)+(\s*:?-+:?\s*)?$/`: past the `[>\s]*`
/// prefix and an optional leading pipe, the `|`-separated parts must all be
/// cells except the last, which may be empty, with at least one pipe.
pub(super) fn delimiter_line(s: &str) -> bool {
    let mut i = 0;
    while let Some(c) = chars::char_at(s, i) {
        if c != '>' && !chars::js_space(c) {
            break;
        }
        i += c.len_utf8();
    }
    let mut rest = &s[i..];
    if let Some(r) = rest.strip_prefix('|') {
        rest = r;
    }
    let parts: Vec<&str> = rest.split('|').collect();
    if parts.len() < 2 {
        return false;
    }
    let (last, cells) = parts.split_last().unwrap();
    cells.iter().all(|c| delimiter_cell(c)) && (last.is_empty() || delimiter_cell(last))
}

/// `\s*:?-+:?\s*`, the whole of `s`.
fn delimiter_cell(s: &str) -> bool {
    let t = s.trim_matches(chars::js_space);
    let t = t.strip_prefix(':').unwrap_or(t);
    let t = t.strip_suffix(':').unwrap_or(t);
    !t.is_empty() && t.bytes().all(|b| b == b'-')
}
