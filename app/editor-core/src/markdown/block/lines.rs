//! Line tests: the predicates `BlockContext` and the block parsers use to
//! recognise a line as fenced code, a quote, a rule, a list marker, a heading
//! or an HTML block start, plus the small helpers they share.

use super::{BlockContext, Line};
use crate::markdown::chars::space;
use crate::markdown::html;
use crate::markdown::tables::NodeType as T;
use crate::markdown::tree::Elt;

pub(super) fn is_fenced_code(line: &Line) -> i32 {
    if line.next != b'`' as i32 && line.next != b'~' as i32 {
        return -1;
    }
    let mut pos = line.pos + 1;
    while pos < line.len() && line.at(pos) == line.next {
        pos += 1;
    }
    if pos < line.pos + 3 {
        return -1;
    }
    if line.next == b'`' as i32 && line.text.as_bytes()[pos..].contains(&b'`') {
        return -1;
    }
    pos as i32
}

pub(super) fn is_blockquote(line: &Line) -> i32 {
    if line.next != b'>' as i32 {
        -1
    } else if line.at(line.pos + 1) == 32 {
        2
    } else {
        1
    }
}

pub(super) fn is_horizontal_rule(line: &Line, cx: &BlockContext, breaking: bool) -> i32 {
    if line.next != b'*' as i32 && line.next != b'-' as i32 && line.next != b'_' as i32 {
        return -1;
    }
    // Each nested list marker on a line rescans the rest of it; a scan that
    // failed at byte `f` fails there again from any later start before `f`.
    let (marker, from, fail) = line.rule_fail.get();
    if marker == line.next && from <= line.pos && line.pos < fail {
        return -1;
    }
    let mut count = 1;
    for pos in line.pos + 1..line.len() {
        let ch = line.at(pos);
        if ch == line.next {
            count += 1;
        } else if !space(ch) {
            line.rule_fail.set((line.next, line.pos, pos));
            return -1;
        }
    }
    // Setext headings take precedence (the Setext leaf parser is configured).
    if breaking
        && line.next == b'-' as i32
        && is_setext_underline(line) > -1
        && line.depth == cx.stack.len()
    {
        return -1;
    }
    if count < 3 { -1 } else { 1 }
}

pub(super) fn is_bullet_list(line: &Line, cx: &BlockContext, breaking: bool) -> i32 {
    let ok = (line.next == b'-' as i32 || line.next == b'+' as i32 || line.next == b'*' as i32)
        && (line.pos + 1 == line.len() || space(line.at(line.pos + 1)))
        && (!breaking || cx.in_list(T::BulletList) || line.skip_space(line.pos + 2) < line.len());
    if ok { 1 } else { -1 }
}

pub(super) fn is_ordered_list(line: &Line, cx: &BlockContext, breaking: bool) -> i32 {
    let (mut pos, mut next) = (line.pos, line.next);
    loop {
        if (b'0' as i32..=b'9' as i32).contains(&next) {
            pos += 1;
        } else {
            break;
        }
        if pos == line.len() {
            return -1;
        }
        next = line.at(pos);
    }
    if pos == line.pos
        || pos > line.pos + 9
        || (next != b'.' as i32 && next != b')' as i32)
        || (pos + 1 < line.len() && !space(line.at(pos + 1)))
        || (breaking
            && !cx.in_list(T::OrderedList)
            && (line.skip_space(pos + 1) == line.len()
                || pos > line.pos + 1
                || line.next != b'1' as i32))
    {
        return -1;
    }
    (pos + 1 - line.pos) as i32
}

pub(super) fn is_atx_heading(line: &Line) -> i32 {
    if line.next != b'#' as i32 {
        return -1;
    }
    let mut pos = line.pos + 1;
    while pos < line.len() && line.at(pos) == b'#' as i32 {
        pos += 1;
    }
    if pos < line.len() && line.at(pos) != 32 {
        return -1;
    }
    let size = pos - line.pos;
    if size > 6 { -1 } else { size as i32 }
}

pub(super) fn is_setext_underline(line: &Line) -> i32 {
    if (line.next != b'-' as i32 && line.next != b'=' as i32) || line.indent >= line.base_indent + 4
    {
        return -1;
    }
    let mut pos = line.pos + 1;
    while pos < line.len() && line.at(pos) == line.next {
        pos += 1;
    }
    let end = pos;
    while pos < line.len() && space(line.at(pos)) {
        pos += 1;
    }
    if pos == line.len() { end as i32 } else { -1 }
}

pub(super) fn is_html_block(line: &Line, breaking: bool) -> Option<usize> {
    if line.next != b'<' as i32 {
        return None;
    }
    html::block_start(&line.text[line.pos..], breaking)
}

pub(super) fn get_list_indent(line: &Line, pos: usize) -> usize {
    let indent_after = line.count_indent(pos, line.pos, line.indent);
    let skipped = line.skip_space(pos);
    let indented = line.count_indent(skipped, pos, indent_after);
    if indented >= indent_after + 5 || skipped == line.len() {
        indent_after + 1
    } else {
        indented
    }
}

pub(super) fn add_code_text(marks: &mut Vec<Elt>, from: usize, to: usize) {
    if let Some(last) = marks.last_mut()
        && last.to == from
        && last.kind == T::CodeText
    {
        last.to = to;
        return;
    }
    marks.push(Elt::new(T::CodeText, from, to));
}

/// `^\[[ xX]\][ \t]` on a leaf's content.
pub(super) fn is_task(content: &str) -> bool {
    let b = content.as_bytes();
    b.len() >= 4
        && b[0] == b'['
        && matches!(b[1], b' ' | b'x' | b'X')
        && b[2] == b']'
        && matches!(b[3], b' ' | b'\t')
}
