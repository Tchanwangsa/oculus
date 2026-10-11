//! The app's block extensions: display maths (`mathSyntax.ts`) and
//! frontmatter (`frontmatter.ts`).

use super::{Block, BlockContext, FRONTMATTER_SCAN, Line};
use crate::markdown::chars;
use crate::markdown::tables::NodeType as T;
use crate::markdown::tree::Elt;

/// `mathSyntax.ts`'s `openDelimiter`: the closer for `$$` or `\[` at the
/// line's content start.
pub(super) fn open_math_delimiter(line: &Line) -> Option<&'static str> {
    if line.indent >= line.base_indent + 4 {
        return None;
    }
    if line.next == b'$' as i32 && line.at(line.pos + 1) == b'$' as i32 {
        return Some("$$");
    }
    if line.next == b'\\' as i32 && line.at(line.pos + 1) == b'[' as i32 {
        return Some("\\]");
    }
    None
}

/// `mathSyntax.ts`'s `blockMath`: `$$…$$` / `\[…\]`, one line or many; an
/// unclosed block runs to the end of its container.
pub(super) fn block_math(cx: &mut BlockContext) -> Block {
    let Some(close) = open_math_delimiter(&cx.line) else {
        return Block::No;
    };
    let from = cx.line_start + cx.line.pos;
    let mut marks = vec![Elt::new(T::MathMark, from, from + 2)];

    let rest = cx.line.text.get(cx.line.pos + 2..).unwrap_or("");
    if let Some(close_at) = rest.rfind(close)
        && chars::is_blank(&rest[close_at + 2..])
    {
        let at = from + 2 + close_at;
        marks.push(Elt::new(T::MathMark, at, at + 2));
        cx.next_line();
        cx.add_element(Elt::with(T::BlockMath, from, at + 2, marks));
        return Block::Leaf;
    }

    let mut to = cx.line_start + cx.line.len();
    while cx.next_line() && cx.line.depth >= cx.stack.len() {
        marks.extend(cx.line.markers.iter().cloned());
        let text = chars::trim_end(cx.line.text);
        to = cx.line_start + cx.line.len();
        if text.ends_with(close) && text.len() >= cx.line.pos + 2 {
            let at = cx.line_start + text.len() - 2;
            marks.push(Elt::new(T::MathMark, at, at + 2));
            to = at + 2;
            cx.next_line();
            break;
        }
    }
    cx.add_element(Elt::with(T::BlockMath, from, to, marks));
    Block::Leaf
}

/// `frontmatter.ts`: a `---` first line through the next line that is
/// exactly `---` or `...`, looked for in the first 64 KiB (UTF-16 units).
pub(super) fn frontmatter(cx: &mut BlockContext) -> Block {
    if !cx.at_doc_start || cx.line_start != 0 || cx.line.text != "---" {
        return Block::No;
    }
    let src = cx.src;
    // The byte offset where FRONTMATTER_SCAN units end (or the source end).
    let mut cut = src.len();
    if src.len() > FRONTMATTER_SCAN {
        let mut units = 0;
        for (i, c) in src.char_indices() {
            if units + c.len_utf16() > FRONTMATTER_SCAN {
                cut = i;
                break;
            }
            units += c.len_utf16();
        }
    }
    let head = &src[..cut];
    let lines: Vec<&str> = head.split('\n').collect();
    // A line cut by the scan limit is not a whole line.
    let whole = if cut == src.len() {
        lines.len()
    } else {
        lines.len() - 1
    };
    let Some(close) = (1..whole).find(|&i| lines[i] == "---" || lines[i] == "...") else {
        return Block::No;
    };
    let mut marks = vec![Elt::new(T::FrontmatterMark, 0, 3)];
    for _ in 0..close {
        cx.next_line();
    }
    let at = cx.line_start;
    marks.push(Elt::new(T::FrontmatterMark, at, at + 3));
    cx.next_line();
    cx.add_element(Elt::with(T::Frontmatter, 0, at + 3, marks));
    Block::Leaf
}
