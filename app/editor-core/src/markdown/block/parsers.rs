//! The block parsers in the order the app configures them, and the leaf
//! parsers they start (indented code, fences, quotes, lists, ATX headings,
//! HTML blocks).

use super::extensions::{block_math, frontmatter};
use super::lines::{
    add_code_text, get_list_indent, is_atx_heading, is_blockquote, is_bullet_list, is_fenced_code,
    is_horizontal_rule, is_html_block, is_ordered_list,
};
use super::{Block, BlockContext, skip_space_back};
use crate::markdown::chars::space;
use crate::markdown::html;
use crate::markdown::inline::parse_inline;
use crate::markdown::tables::NodeType as T;
use crate::markdown::tree::Elt;

type BlockParser = for<'a, 'b> fn(&'b mut BlockContext<'a>) -> Block;

/// `LinkReference`, `Table`, `SetextHeading` and `TaskList` have no block
/// parser, only leaf parsers.
pub(super) const BLOCK_PARSERS: [BlockParser; 10] = [
    indented_code,
    block_math,
    fenced_code,
    blockquote,
    frontmatter,
    horizontal_rule,
    bullet_list,
    ordered_list,
    atx_heading,
    html_block,
];

fn indented_code(cx: &mut BlockContext) -> Block {
    let base = cx.line.base_indent + 4;
    if cx.line.indent < base {
        return Block::No;
    }
    let start = cx.line.find_column(base);
    let from = cx.line_start + start;
    let mut to = cx.line_start + cx.line.len();
    let mut marks = Vec::new();
    let mut pending: Vec<Elt> = Vec::new();
    add_code_text(&mut marks, from, to);
    while cx.next_line() && cx.line.depth >= cx.stack.len() {
        if cx.line.pos == cx.line.len() {
            add_code_text(&mut pending, cx.line_start - 1, cx.line_start);
            pending.extend(cx.line.markers.iter().cloned());
        } else if cx.line.indent < base {
            break;
        } else {
            for m in pending.drain(..) {
                if m.kind == T::CodeText {
                    add_code_text(&mut marks, m.from, m.to);
                } else {
                    marks.push(m);
                }
            }
            add_code_text(&mut marks, cx.line_start - 1, cx.line_start);
            marks.extend(cx.line.markers.iter().cloned());
            to = cx.line_start + cx.line.len();
            let code_start = cx.line_start + cx.line.find_column(cx.line.base_indent + 4);
            if code_start < to {
                add_code_text(&mut marks, code_start, to);
            }
        }
    }
    pending.retain(|m| m.kind != T::CodeText);
    if !pending.is_empty() {
        pending.append(&mut cx.line.markers);
        cx.line.markers = pending;
    }
    cx.add_element(Elt::with(T::CodeBlock, from, to, marks));
    Block::Leaf
}

fn fenced_code(cx: &mut BlockContext) -> Block {
    let fence_end = is_fenced_code(&cx.line);
    if fence_end < 0 {
        return Block::No;
    }
    let fence_end = fence_end as usize;
    let line = &cx.line;
    let from = cx.line_start + line.pos;
    let ch = line.next;
    let len = fence_end - line.pos;
    let info_from = line.skip_space(fence_end);
    let info_to = skip_space_back(line.text, line.len(), info_from);
    let mut marks = vec![Elt::new(T::CodeMark, from, from + len)];
    if info_from < info_to {
        marks.push(Elt::new(
            T::CodeInfo,
            cx.line_start + info_from,
            cx.line_start + info_to,
        ));
    }
    let (mut first, mut empty, mut has_line) = (true, true, false);
    while cx.next_line() && cx.line.depth >= cx.stack.len() {
        let line = &cx.line;
        let mut i = line.pos;
        if line.indent < line.base_indent + 4 {
            while i < line.len() && line.at(i) == ch {
                i += 1;
            }
        }
        if i - line.pos >= len && line.skip_space(i) == line.len() {
            marks.extend(line.markers.iter().cloned());
            if empty && has_line {
                add_code_text(&mut marks, cx.line_start - 1, cx.line_start);
            }
            marks.push(Elt::new(
                T::CodeMark,
                cx.line_start + line.pos,
                cx.line_start + i,
            ));
            cx.next_line();
            break;
        }
        has_line = true;
        if !first {
            add_code_text(&mut marks, cx.line_start - 1, cx.line_start);
            empty = false;
        }
        marks.extend(line.markers.iter().cloned());
        let text_start = cx.line_start + line.base_pos;
        let text_end = cx.line_start + line.len();
        if text_start < text_end {
            add_code_text(&mut marks, text_start, text_end);
            empty = false;
        }
        first = false;
    }
    let to = cx.prev_line_end();
    cx.add_element(Elt::with(T::FencedCode, from, to, marks));
    Block::Leaf
}

fn blockquote(cx: &mut BlockContext) -> Block {
    let size = is_blockquote(&cx.line);
    if size < 0 {
        return Block::No;
    }
    let pos = cx.line.pos;
    cx.start_context(T::Blockquote, pos, 0);
    let from = cx.line_start + pos;
    cx.add_node(T::QuoteMark, from, Some(from + 1));
    cx.line.move_base(pos + size as usize);
    Block::Container
}

fn horizontal_rule(cx: &mut BlockContext) -> Block {
    if is_horizontal_rule(&cx.line, cx, false) < 0 {
        return Block::No;
    }
    let from = cx.line_start + cx.line.pos;
    cx.next_line();
    cx.add_node(T::HorizontalRule, from, None);
    Block::Leaf
}

fn bullet_list(cx: &mut BlockContext) -> Block {
    let size = is_bullet_list(&cx.line, cx, false);
    if size < 0 {
        return Block::No;
    }
    if cx.stack.last().unwrap().kind != T::BulletList {
        cx.start_context(T::BulletList, cx.line.base_pos, cx.line.next);
    }
    let new_base = get_list_indent(&cx.line, cx.line.pos + 1);
    cx.start_context(
        T::ListItem,
        cx.line.base_pos,
        new_base as i32 - cx.line.base_indent as i32,
    );
    let from = cx.line_start + cx.line.pos;
    cx.add_node(T::ListMark, from, Some(from + size as usize));
    cx.line.move_base_column(new_base);
    Block::Container
}

fn ordered_list(cx: &mut BlockContext) -> Block {
    let size = is_ordered_list(&cx.line, cx, false);
    if size < 0 {
        return Block::No;
    }
    let size = size as usize;
    if cx.stack.last().unwrap().kind != T::OrderedList {
        let value = cx.line.at(cx.line.pos + size - 1);
        cx.start_context(T::OrderedList, cx.line.base_pos, value);
    }
    let new_base = get_list_indent(&cx.line, cx.line.pos + size);
    cx.start_context(
        T::ListItem,
        cx.line.base_pos,
        new_base as i32 - cx.line.base_indent as i32,
    );
    let from = cx.line_start + cx.line.pos;
    cx.add_node(T::ListMark, from, Some(from + size));
    cx.line.move_base_column(new_base);
    Block::Container
}

const ATX: [T; 6] = [
    T::ATXHeading1,
    T::ATXHeading2,
    T::ATXHeading3,
    T::ATXHeading4,
    T::ATXHeading5,
    T::ATXHeading6,
];

fn atx_heading(cx: &mut BlockContext) -> Block {
    let size = is_atx_heading(&cx.line);
    if size < 0 {
        return Block::No;
    }
    let size = size as usize;
    let line = &cx.line;
    let off = line.pos;
    let from = cx.line_start + off;
    let end_of_space = skip_space_back(line.text, line.len(), off);
    let mut after = end_of_space;
    while after > off && line.at(after - 1) == line.next {
        after -= 1;
    }
    if after == end_of_space || after == off || !space(line.at(after - 1)) {
        after = line.len();
    }
    let mut children = vec![Elt::new(T::HeaderMark, from, from + size)];
    let (a, b) = ((off + size + 1).min(line.len()), after.min(line.len()));
    let content = if a < b { &line.text[a..b] } else { "" };
    children.extend(parse_inline(content, from + size + 1));
    if after < line.len() {
        children.push(Elt::new(
            T::HeaderMark,
            from + after - off,
            from + end_of_space - off,
        ));
    }
    let to = from + line.len() - off;
    cx.next_line();
    cx.add_element(Elt::with(ATX[size - 1], from, to, children));
    Block::Leaf
}

fn html_block(cx: &mut BlockContext) -> Block {
    let Some(style) = is_html_block(&cx.line, false) else {
        return Block::No;
    };
    let from = cx.line_start + cx.line.pos;
    let mut marks = Vec::new();
    // Styles 5 and 6 end at a blank line, which they do not take.
    let mut trailing = style < 5;
    while !html::block_end(style, cx.line.text) && cx.next_line() {
        if cx.line.depth < cx.stack.len() {
            trailing = false;
            break;
        }
        marks.extend(cx.line.markers.iter().cloned());
    }
    if trailing {
        cx.next_line();
    }
    let kind = match style {
        1 => T::CommentBlock,
        2 => T::ProcessingInstructionBlock,
        _ => T::HTMLBlock,
    };
    let to = cx.prev_line_end();
    cx.add_element(Elt::with(kind, from, to, marks));
    Block::Leaf
}
