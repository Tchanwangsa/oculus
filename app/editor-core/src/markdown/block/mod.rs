//! The block parser: `@lezer/markdown`'s `BlockContext` line algorithm with
//! the app's extensions in its configured order (`editor/core/language.ts`).
//!
//! Each line is matched against the open containers (`read_line`), then the
//! block parsers run in order; a line no parser takes starts a leaf block
//! that collects lines until a blank line or an `end_leaf` test stops it.
//! Offsets are bytes into the source; the source has `\n` breaks only. Every
//! function names the Lezer function it ports and keeps its order of checks,
//! since the oracle compares trees, not behaviour in the abstract.
//!
//! All parser state lives in `BlockContext` (the container stack, the
//! current line, the position), so a later incremental parse can resume at a
//! top-level block boundary by rebuilding it.

mod extensions;
mod gfm_tables;
mod lines;
mod link_refs;
mod parsers;

use extensions::open_math_delimiter;
use gfm_tables::{delimiter_line, has_pipe, parse_row};
use lines::{
    is_atx_heading, is_blockquote, is_bullet_list, is_fenced_code, is_horizontal_rule,
    is_html_block, is_ordered_list, is_setext_underline, is_task,
};
use link_refs::{LinkReference, byte_at_units};
pub(crate) use link_refs::{Parsed, parse_link_label, parse_link_title, parse_url};
use parsers::BLOCK_PARSERS;

use super::chars::{self, space};
use super::inline::{inject_marks, parse_inline};
use super::tables::NodeType as T;
use super::tree::Elt;

/// The frontmatter closer is looked for within this many UTF-16 units
/// (`frontmatter.ts`'s `SCAN_MAX`).
const FRONTMATTER_SCAN: usize = 64 * 1024;

/// Lezer's `Line`: the current line and how far container markup took it.
pub(crate) struct Line<'a> {
    pub text: &'a str,
    /// Column provided by the containers handled so far.
    pub base_indent: usize,
    /// Byte position corresponding to `base_indent`.
    pub base_pos: usize,
    /// Containers matched on this line (the Document counts).
    pub depth: usize,
    /// Container markers (quote marks) found on this line.
    pub markers: Vec<Elt>,
    /// The first non-space char after the base position.
    pub pos: usize,
    /// Column of `pos`.
    pub indent: usize,
    /// Byte at `pos`, or -1 at the end.
    pub next: i32,
    /// A char boundary and its true column, from the last `find_column`.
    column_mark: (usize, usize),
    /// The last failed horizontal-rule scan: marker, start, failing byte.
    rule_fail: std::cell::Cell<(i32, usize, usize)>,
}

impl<'a> Line<'a> {
    fn new() -> Self {
        Line {
            text: "",
            base_indent: 0,
            base_pos: 0,
            depth: 0,
            markers: Vec::new(),
            pos: 0,
            indent: 0,
            next: -1,
            column_mark: (0, 0),
            rule_fail: std::cell::Cell::new((-1, 0, 0)),
        }
    }

    /// Byte at `i`, or -1 (JS `charCodeAt` past the end is NaN, never equal).
    pub fn at(&self, i: usize) -> i32 {
        self.text.as_bytes().get(i).map_or(-1, |&b| b as i32)
    }

    fn len(&self) -> usize {
        self.text.len()
    }

    fn forward(&mut self) {
        if self.base_pos > self.pos {
            self.forward_inner();
        }
    }

    fn forward_inner(&mut self) {
        let new_pos = self.skip_space(self.base_pos);
        self.indent = self.count_indent(new_pos, self.pos, self.indent);
        self.pos = new_pos;
        self.next = self.at(new_pos);
    }

    pub fn skip_space(&self, from: usize) -> usize {
        skip_space(self.text, from)
    }

    fn reset(&mut self, text: &'a str) {
        self.text = text;
        self.base_indent = 0;
        self.base_pos = 0;
        self.pos = 0;
        self.indent = 0;
        self.forward_inner();
        self.depth = 1;
        self.markers.clear();
        self.column_mark = (0, 0);
        self.rule_fail.set((-1, 0, 0));
    }

    fn move_base(&mut self, to: usize) {
        self.base_pos = to;
        self.base_indent = self.count_indent(to, self.pos, self.indent);
    }

    fn move_base_column(&mut self, indent: usize) {
        self.base_indent = indent;
        self.base_pos = self.find_column(indent);
    }

    /// The column at `to`, counting from byte `from` at column `indent`;
    /// one column per UTF-16 unit, tabs to the next multiple of 4.
    pub fn count_indent(&self, to: usize, from: usize, mut indent: usize) -> usize {
        let bytes = self.text.as_bytes();
        let to = to.min(bytes.len());
        if from >= to {
            return indent;
        }
        for &b in &bytes[from..to] {
            indent += if b == b'\t' {
                4 - indent % 4
            } else {
                chars::units(b)
            };
        }
        indent
    }

    /// The byte position of column `goal` (or the line's end). Columns are
    /// UTF-16 units, so a goal reached by an astral char's first unit gives
    /// the byte after its lead, standing for JS's position between the
    /// surrogates (`Utf16Map` maps it back).
    pub fn find_column(&mut self, goal: usize) -> usize {
        // Resume from the last answer when it lies before `goal`: Lezer
        // scans from 0 each time, which is quadratic in nested list markers.
        let (mut i, mut indent) = match self.column_mark {
            (at, col) if col < goal => (at, col),
            _ => (0, 0),
        };
        for c in self.text[i..].chars() {
            if indent >= goal {
                break;
            }
            if c == '\t' {
                indent += 4 - indent % 4;
            } else if c.len_utf16() == 2 {
                indent += 1;
                if indent >= goal {
                    return i + 1;
                }
                indent += 1;
            } else {
                indent += 1;
            }
            i += c.len_utf8();
        }
        self.column_mark = (i, indent);
        i
    }

    /// The line with the base prefix blanked out, keeping its length.
    fn scrub(&self) -> String {
        if self.base_indent == 0 {
            return self.text.to_string();
        }
        let mut out = " ".repeat(self.base_pos);
        out.push_str(&self.text[self.base_pos..]);
        out
    }
}

pub(crate) fn skip_space(s: &str, mut i: usize) -> usize {
    let b = s.as_bytes();
    while i < b.len() && space(b[i] as i32) {
        i += 1;
    }
    i
}

fn skip_space_back(s: &str, mut i: usize, to: usize) -> usize {
    let b = s.as_bytes();
    while i > to && space(b[i - 1] as i32) {
        i -= 1;
    }
    i
}

/// An open container: Lezer's `CompositeBlock`.
#[derive(Debug, Clone)]
pub(crate) struct Composite {
    pub kind: T,
    /// List item: content indent; list: its marker char.
    pub value: i32,
    pub from: usize,
    pub end: usize,
    pub children: Vec<Elt>,
}

impl Composite {
    fn into_elt(self, end: usize) -> Elt {
        let end = self.children.last().map_or(end, |c| end.max(c.to));
        Elt::with(self.kind, self.from, end, self.children)
    }
}

/// A leaf block being collected: Lezer's `LeafBlock`.
pub(crate) struct LeafBlock {
    pub start: usize,
    pub content: String,
    pub marks: Vec<Elt>,
    pub parsers: Vec<LeafParser>,
}

/// The leaf parsers, in configured order: LinkReference, Table,
/// SetextHeading, TaskList.
pub(crate) enum LeafParser {
    LinkReference(LinkReference),
    Table(Option<Option<Vec<Elt>>>),
    Setext,
    Task,
}

enum Block {
    /// The parser does not apply.
    No,
    /// It consumed a leaf block.
    Leaf,
    /// It opened a container; the line goes on.
    Container,
}

/// Where `advance` left the parse.
enum Step {
    Going,
    Done,
    /// Stopped by the caller at the start of a line, with only the
    /// `Document` open.
    Stopped,
}

/// A parse that ran from some line to a stop point or the end.
pub(crate) struct Partial {
    /// The `Document`'s children, with offsets into the parsed source.
    pub children: Vec<Elt>,
    /// The line start where the caller stopped it, if it did.
    pub stopped_at: Option<usize>,
}

pub(crate) struct BlockContext<'a> {
    src: &'a str,
    /// Whether `src` starts the document (frontmatter can only open there).
    at_doc_start: bool,
    pub line: Line<'a>,
    at_end: bool,
    /// Start of the current line (Lezer's `lineStart` and
    /// `absoluteLineStart`: one range, so they are equal).
    pub line_start: usize,
    line_end: usize,
    pub stack: Vec<Composite>,
}

impl<'a> BlockContext<'a> {
    pub fn new(src: &'a str) -> Self {
        Self::resume(src, true)
    }

    /// A parse of `src` from its first line with only the `Document` open:
    /// the state at the start of any line where a top-level block begins.
    pub fn resume(src: &'a str, at_doc_start: bool) -> Self {
        let mut cx = BlockContext {
            src,
            at_doc_start,
            line: Line::new(),
            at_end: false,
            line_start: 0,
            line_end: 0,
            stack: vec![Composite {
                kind: T::Document,
                value: 0,
                from: 0,
                end: 0,
                children: Vec::new(),
            }],
        };
        cx.read_line();
        cx
    }

    /// Parses to the end and returns the `Document` element.
    pub fn parse(mut self) -> Elt {
        while let Step::Going = self.advance(&mut |_| false) {}
        while self.stack.len() > 1 {
            self.finish_context();
        }
        let doc = self.stack.pop().unwrap();
        doc.into_elt(self.line_start)
    }

    /// Parses until `stop` says so at the start of a line where only the
    /// `Document` is open (`stop` sees the line start and its text), or to
    /// the end.
    pub fn parse_until(mut self, mut stop: impl FnMut(usize, &str) -> bool) -> Partial {
        let mut stop = |cx: &BlockContext| stop(cx.line_start, cx.line.text);
        let stopped_at = loop {
            match self.advance(&mut stop) {
                Step::Going => {}
                Step::Done => break None,
                Step::Stopped => break Some(self.line_start),
            }
        };
        while self.stack.len() > 1 {
            self.finish_context();
        }
        let doc = self.stack.pop().unwrap();
        Partial {
            children: doc.children,
            stopped_at,
        }
    }

    fn block(&mut self) -> &mut Composite {
        self.stack.last_mut().unwrap()
    }

    /// One step of `advance`. `stop` is asked at each line start the step
    /// reaches with only the `Document` open.
    fn advance(&mut self, stop: &mut dyn FnMut(&BlockContext) -> bool) -> Step {
        loop {
            let mut mark_i = 0;
            loop {
                let next_end =
                    (self.line.depth < self.stack.len()).then(|| self.stack.last().unwrap().end);
                while mark_i < self.line.markers.len()
                    && next_end.is_none_or(|end| self.line.markers[mark_i].from < end)
                {
                    let mark = &self.line.markers[mark_i];
                    let node = Elt::new(mark.kind, mark.from, mark.to);
                    mark_i += 1;
                    self.block().children.push(node);
                }
                if next_end.is_none() {
                    break;
                }
                self.finish_context();
            }
            if self.stack.len() == 1 && stop(self) {
                return Step::Stopped;
            }
            if self.line.pos < self.line.len() {
                break;
            }
            if !self.next_line() {
                return Step::Done;
            }
        }
        'start: loop {
            for parser in BLOCK_PARSERS {
                match parser(self) {
                    Block::No => continue,
                    Block::Leaf => return Step::Going,
                    Block::Container => {
                        self.line.forward();
                        continue 'start;
                    }
                }
            }
            break;
        }
        if self.line.pos == self.line.len() {
            return if self.next_line() {
                Step::Going
            } else {
                Step::Done
            };
        }
        let mut leaf = LeafBlock {
            start: self.line_start + self.line.pos,
            content: self.line.text[self.line.pos..].to_string(),
            marks: Vec::new(),
            parsers: Vec::new(),
        };
        if leaf.content.starts_with('[') {
            leaf.parsers
                .push(LeafParser::LinkReference(LinkReference::new(&leaf)));
        }
        if has_pipe(&leaf.content, 0) {
            leaf.parsers.push(LeafParser::Table(None));
        }
        leaf.parsers.push(LeafParser::Setext);
        if is_task(&leaf.content) && self.stack.last().unwrap().kind == T::ListItem {
            leaf.parsers.push(LeafParser::Task);
        }
        'lines: while self.next_line() {
            if self.line.pos == self.line.len() {
                break;
            }
            if self.line.indent < self.line.base_indent + 4 && self.end_leaf(&leaf) {
                break 'lines;
            }
            let mut parsers = std::mem::take(&mut leaf.parsers);
            for i in 0..parsers.len() {
                if self.leaf_next_line(&mut parsers, i, &mut leaf) {
                    return Step::Going;
                }
            }
            leaf.parsers = parsers;
            leaf.content.push('\n');
            leaf.content.push_str(&self.line.scrub());
            leaf.marks.extend(self.line.markers.iter().cloned());
        }
        self.finish_leaf(leaf);
        Step::Going
    }

    /// Lezer's `nextLine`.
    pub fn next_line(&mut self) -> bool {
        self.line_start += self.line.len();
        if self.line_end >= self.src.len() {
            self.line_start = self.line_end;
            self.at_end = true;
            self.read_line();
            false
        } else {
            self.line_start = self.line_end + 1;
            self.read_line();
            true
        }
    }

    /// The text of the line starting at `start`.
    fn scan_line(&self, start: usize) -> &'a str {
        if start >= self.src.len() {
            return "";
        }
        let rest = &self.src[start..];
        &rest[..rest.find('\n').unwrap_or(rest.len())]
    }

    fn peek_line(&self) -> &'a str {
        self.scan_line(self.line_end + 1)
    }

    /// Lezer's `readLine`: loads the line and matches it against the stack.
    fn read_line(&mut self) {
        let text = self.scan_line(self.line_start);
        self.line_end = self.line_start + text.len();
        self.line.reset(text);
        while self.line.depth < self.stack.len() {
            let depth = self.line.depth;
            let marks = self.line.markers.len();
            if !self.skip_markup(depth) {
                if self.line.markers.len() > marks {
                    self.stack[depth].end = self.line.markers.last().unwrap().to;
                }
                self.line.forward();
                break;
            }
            self.line.forward();
            self.line.depth += 1;
        }
    }

    /// `DefaultSkipMarkup` for the container at `depth`.
    fn skip_markup(&mut self, depth: usize) -> bool {
        let line_start = self.line_start;
        match self.stack[depth].kind {
            T::Blockquote => {
                let line = &mut self.line;
                if line.next != b'>' as i32 {
                    return false;
                }
                line.markers.push(Elt::new(
                    T::QuoteMark,
                    line_start + line.pos,
                    line_start + line.pos + 1,
                ));
                let size = if space(line.at(line.pos + 1)) { 2 } else { 1 };
                line.move_base(line.pos + size);
                self.stack[depth].end = line_start + line.len();
                true
            }
            T::ListItem => {
                let value = self.stack[depth].value as i64;
                let line = &mut self.line;
                let goal = line.base_indent as i64 + value;
                if (line.indent as i64) < goal && line.next > -1 {
                    return false;
                }
                line.move_base_column(goal.max(0) as usize);
                true
            }
            T::BulletList | T::OrderedList => self.skip_for_list(depth),
            _ => true,
        }
    }

    fn skip_for_list(&self, depth: usize) -> bool {
        let line = &self.line;
        let bl = &self.stack[depth];
        if line.pos == line.len()
            || (depth != self.stack.len() - 1
                && line.indent as i64
                    >= self.stack[line.depth + 1].value as i64 + line.base_indent as i64)
        {
            return true;
        }
        if line.indent >= line.base_indent + 4 {
            return false;
        }
        let size = if bl.kind == T::OrderedList {
            is_ordered_list(line, self, false)
        } else {
            is_bullet_list(line, self, false)
        };
        size > 0
            && (bl.kind != T::BulletList || is_horizontal_rule(line, self, false) < 0)
            && line.at(line.pos + size as usize - 1) == bl.value
    }

    pub fn prev_line_end(&self) -> usize {
        if self.at_end {
            self.line_start
        } else {
            self.line_start - 1
        }
    }

    fn start_context(&mut self, kind: T, start: usize, value: i32) {
        let block = Composite {
            kind,
            value,
            from: self.line_start + start,
            end: self.line_start + self.line.len(),
            children: Vec::new(),
        };
        self.stack.push(block);
    }

    /// `addNode` with a type: a childless node ending at `to` or the end of
    /// the previous line.
    fn add_node(&mut self, kind: T, from: usize, to: Option<usize>) {
        let to = to.unwrap_or_else(|| self.prev_line_end());
        self.block().children.push(Elt::new(kind, from, to));
    }

    pub fn add_element(&mut self, elt: Elt) {
        self.block().children.push(elt);
    }

    pub fn add_leaf_element(&mut self, leaf: &LeafBlock, mut elt: Elt) {
        let children = inject_marks(std::mem::take(&mut elt.children), &leaf.marks);
        self.add_element(Elt::with(elt.kind, elt.from, elt.to, children));
    }

    fn finish_context(&mut self) {
        let cx = self.stack.pop().unwrap();
        let end = cx.end;
        let elt = cx.into_elt(end);
        self.block().children.push(elt);
    }

    fn finish_leaf(&mut self, mut leaf: LeafBlock) {
        let parsers = std::mem::take(&mut leaf.parsers);
        for parser in parsers {
            let done = match parser {
                LeafParser::LinkReference(r) => r.finish(self, &leaf),
                LeafParser::Table(Some(Some(rows))) => {
                    let to = leaf.start + leaf.content.len();
                    self.add_leaf_element(&leaf, Elt::with(T::Table, leaf.start, to, rows));
                    true
                }
                LeafParser::Table(_) | LeafParser::Setext => false,
                LeafParser::Task => {
                    let mut children = vec![Elt::new(T::TaskMarker, leaf.start, leaf.start + 3)];
                    children.extend(parse_inline(&leaf.content[3..], leaf.start + 3));
                    let to = leaf.start + leaf.content.len();
                    self.add_leaf_element(&leaf, Elt::with(T::Task, leaf.start, to, children));
                    true
                }
            };
            if done {
                return;
            }
        }
        let inline = inject_marks(parse_inline(&leaf.content, leaf.start), &leaf.marks);
        let to = leaf.start + leaf.content.len();
        self.add_element(Elt::with(T::Paragraph, leaf.start, to, inline));
    }

    /// `nextLine` of leaf parser `i`; true when it took the leaf.
    fn leaf_next_line(
        &mut self,
        parsers: &mut [LeafParser],
        i: usize,
        leaf: &mut LeafBlock,
    ) -> bool {
        match &mut parsers[i] {
            LeafParser::LinkReference(r) => r.next_line(self, leaf),
            LeafParser::Table(rows) => {
                self.table_next_line(rows, leaf);
                false
            }
            LeafParser::Setext => self.setext_next_line(leaf),
            LeafParser::Task => false,
        }
    }

    /// `TableParser.nextLine`: `None` before the second line, `Some(None)`
    /// when this is not a table, else the rows so far.
    fn table_next_line(&mut self, rows: &mut Option<Option<Vec<Elt>>>, leaf: &LeafBlock) {
        let line = &self.line;
        match rows {
            None => {
                *rows = Some(None);
                let line_text = &line.text[line.pos..];
                if (line.next == b'-' as i32
                    || line.next == b':' as i32
                    || line.next == b'|' as i32)
                    && delimiter_line(line_text)
                {
                    let mut first_row = Vec::new();
                    let first_count = parse_row(&leaf.content, 0, Some(&mut first_row), leaf.start);
                    if first_count == parse_row(line_text, 0, None, 0) {
                        *rows = Some(Some(vec![
                            Elt::with(
                                T::TableHeader,
                                leaf.start,
                                leaf.start + leaf.content.len(),
                                first_row,
                            ),
                            Elt::new(
                                T::TableDelimiter,
                                self.line_start + line.pos,
                                self.line_start + line.len(),
                            ),
                        ]));
                    }
                }
            }
            Some(Some(rows)) => {
                let mut content = Vec::new();
                parse_row(line.text, line.pos, Some(&mut content), self.line_start);
                rows.push(Elt::with(
                    T::TableRow,
                    self.line_start + line.pos,
                    self.line_start + line.len(),
                    content,
                ));
            }
            Some(None) => {}
        }
    }

    /// `SetextHeadingParser.nextLine`.
    fn setext_next_line(&mut self, leaf: &LeafBlock) -> bool {
        let underline = if self.line.depth < self.stack.len() {
            -1
        } else {
            is_setext_underline(&self.line)
        };
        if underline < 0 {
            return false;
        }
        let next = self.line.next;
        let mark = Elt::new(
            T::HeaderMark,
            self.line_start + self.line.pos,
            self.line_start + underline as usize,
        );
        self.next_line();
        let kind = if next == b'=' as i32 {
            T::SetextHeading1
        } else {
            T::SetextHeading2
        };
        let mut children = parse_inline(&leaf.content, leaf.start);
        children.push(mark);
        let to = self.prev_line_end();
        self.add_leaf_element(leaf, Elt::with(kind, leaf.start, to, children));
        true
    }

    /// The `endLeafBlock` tests, in configured order: the seven defaults,
    /// then Table, then BlockMath.
    fn end_leaf(&self, leaf: &LeafBlock) -> bool {
        let line = &self.line;
        is_atx_heading(line) >= 0
            || is_fenced_code(line) >= 0
            || is_blockquote(line) >= 0
            || is_bullet_list(line, self, true) >= 0
            || is_ordered_list(line, self, true) >= 0
            || is_horizontal_rule(line, self, true) >= 0
            || is_html_block(line, true).is_some()
            || self.table_end_leaf(leaf)
            || open_math_delimiter(line).is_some()
    }

    fn table_end_leaf(&self, leaf: &LeafBlock) -> bool {
        let line = &self.line;
        if leaf
            .parsers
            .iter()
            .any(|p| matches!(p, LeafParser::Table(_)))
            || !has_pipe(line.text, line.base_pos)
        {
            return false;
        }
        let next = self.peek_line();
        // `base_pos` counts UTF-16 units of this line's ASCII prefix; on the
        // next line it is a unit offset too.
        let next_start = byte_at_units(next, line.base_pos);
        delimiter_line(next)
            && parse_row(line.text, line.base_pos, None, 0) == parse_row(next, next_start, None, 0)
    }

    fn in_list(&self, kind: T) -> bool {
        self.stack.iter().rev().any(|b| b.kind == kind)
    }
}
