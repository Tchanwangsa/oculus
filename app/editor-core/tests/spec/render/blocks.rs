//! The renderer's block half: containers, headings, code, lists, tables.

use oculus_editor_core::markdown::{Node, NodeType as T};

use super::Renderer;
use crate::text::{escape_html, unescape};
impl Renderer<'_> {
    pub(super) fn blocks(&mut self, parent: Node, tight: bool) {
        for child in parent.children() {
            self.block(child, tight);
        }
    }

    fn block(&mut self, n: Node, tight: bool) {
        match n.kind() {
            T::Paragraph => {
                if tight {
                    self.inline_block(n, n.from(), n.to());
                } else {
                    self.cr();
                    self.out.push_str("<p>");
                    self.inline_block(n, n.from(), n.to());
                    self.out.push_str("</p>");
                    self.cr();
                }
            }
            T::ATXHeading1
            | T::ATXHeading2
            | T::ATXHeading3
            | T::ATXHeading4
            | T::ATXHeading5
            | T::ATXHeading6 => {
                let level = n.kind() as usize - T::ATXHeading1 as usize + 1;
                self.heading(n, level);
            }
            T::SetextHeading1 | T::SetextHeading2 => {
                let level = if n.kind() == T::SetextHeading1 { 1 } else { 2 };
                self.heading(n, level);
            }
            T::Blockquote => {
                self.cr();
                self.out.push_str("<blockquote>");
                self.cr();
                self.blocks(n, false);
                self.cr();
                self.out.push_str("</blockquote>");
                self.cr();
            }
            T::BulletList | T::OrderedList => self.list(n),
            T::CodeBlock => {
                let mut code: String = n
                    .children()
                    .filter(|c| c.kind() == T::CodeText)
                    .map(|c| self.node_text(c))
                    .collect();
                if !code.is_empty() {
                    code.push('\n');
                }
                self.code(&code, None);
            }
            T::FencedCode => self.fenced(n),
            T::HTMLBlock | T::CommentBlock | T::ProcessingInstructionBlock => {
                self.cr();
                let raw = self.raw_lines(n);
                self.out.push_str(&raw);
                self.cr();
            }
            T::HorizontalRule => {
                self.cr();
                self.out.push_str("<hr />");
                self.cr();
            }
            T::Table => self.table(n),
            T::LinkReference | T::QuoteMark | T::ListMark => {}
            T::Task => self.task(n),
            _ => {
                // Extension blocks (maths, frontmatter): their source.
                self.cr();
                self.out.push_str(&escape_html(&self.node_text(n)));
                self.cr();
            }
        }
    }

    fn heading(&mut self, n: Node, level: usize) {
        self.cr();
        self.out.push_str(&format!("<h{level}>"));
        let start = self.out.len();
        self.inline_block(n, n.from(), n.to());
        let trimmed = self.out[start..]
            .trim_matches([' ', '\t', '\n'])
            .to_string();
        self.out.truncate(start);
        self.out.push_str(&trimmed);
        self.out.push_str(&format!("</h{level}>"));
        self.cr();
    }

    /// Lines of `n` with container prefixes (quote marks, the block's start
    /// column) taken off every line after the first.
    fn raw_lines(&self, n: Node) -> String {
        let text = self.node_text(n);
        let marks: Vec<Node> = n.children().filter(|c| c.kind() == T::QuoteMark).collect();
        let indent = {
            let mut i = n.from();
            while i > 0 && self.src[i - 1] != b'\n' as u16 {
                i -= 1;
            }
            n.from() - i
        };
        let mut out = String::new();
        let mut pos = n.from();
        for (k, line) in text.split('\n').enumerate() {
            let line_len = line.encode_utf16().count();
            let mut start = 0;
            if k > 0 {
                if let Some(m) = marks
                    .iter()
                    .rfind(|m| m.from() >= pos && m.to() <= pos + line_len)
                {
                    start = m.to() - pos;
                    if self.src.get(m.to()) == Some(&(b' ' as u16)) {
                        start += 1;
                    }
                } else {
                    start = line.bytes().take(indent).take_while(|&b| b == b' ').count();
                }
            }
            let line16: Vec<u16> = line.encode_utf16().collect();
            out.push_str(&String::from_utf16_lossy(
                &line16[start.min(line16.len())..],
            ));
            out.push('\n');
            pos += line_len + 1;
        }
        out
    }

    fn code(&mut self, code: &str, info: Option<&str>) {
        self.cr();
        match info {
            Some(lang) => self.out.push_str(&format!(
                "<pre><code class=\"language-{}\">",
                escape_html(lang)
            )),
            None => self.out.push_str("<pre><code>"),
        }
        self.out.push_str(&escape_html(code));
        self.out.push_str("</code></pre>");
        self.cr();
    }

    fn fenced(&mut self, n: Node) {
        let marks: Vec<Node> = n.children().filter(|c| c.kind() == T::CodeMark).collect();
        let info = n
            .children()
            .find(|c| c.kind() == T::CodeInfo)
            .map(|i| unescape(&self.node_text(i), self.entities));
        let lang = info.as_deref().and_then(|i| i.split_whitespace().next());
        // The fence's indentation, taken off each content line.
        let open = marks[0];
        let mut line_start = open.from();
        while line_start > 0 && self.src[line_start - 1] != b'\n' as u16 {
            line_start -= 1;
        }
        let base_marks: Vec<Node> = n.children().filter(|c| c.kind() == T::QuoteMark).collect();
        let base = base_marks
            .iter()
            .rfind(|m| m.from() >= line_start && m.to() <= open.from())
            .map_or(line_start, |m| {
                m.to() + usize::from(self.src.get(m.to()) == Some(&(b' ' as u16)))
            });
        let indent = open.from().saturating_sub(base);
        let texts: Vec<Node> = n.children().filter(|c| c.kind() == T::CodeText).collect();
        let close = marks
            .get(1)
            .filter(|m| m.from() > open.to())
            .map(|m| m.from());
        // Content lines: every line start after the opening line, before the
        // closing fence's line.
        let first_line = {
            let mut i = open.to();
            while i < self.src.len() && self.src[i] != b'\n' as u16 {
                i += 1;
            }
            i + 1
        };
        let end = match close {
            Some(c) => {
                let mut i = c;
                while i > 0 && self.src[i - 1] != b'\n' as u16 {
                    i -= 1;
                }
                i
            }
            // An unclosed fence runs to the end; the document's last line is
            // not a line in the spec's sense when it is empty.
            None if n.to() == self.src.len() && self.src.last() == Some(&(b'\n' as u16)) => n.to(),
            None => n.to() + 1,
        };
        let mut code = String::new();
        let mut ls = first_line;
        while ls < end {
            let mut le = ls;
            while le < self.src.len() && self.src[le] != b'\n' as u16 {
                le += 1;
            }
            // Code text runs merge across lines; take this line's part.
            let piece = texts.iter().find(|t| t.from() < le && t.to() > ls);
            let line = match piece {
                Some(t) => {
                    let s = self.text(t.from().max(ls), t.to().min(le));
                    let strip = s.bytes().take(indent).take_while(|&b| b == b' ').count();
                    s[strip..].to_string()
                }
                None => String::new(),
            };
            code.push_str(&line);
            code.push('\n');
            ls = le + 1;
        }
        self.code(&code, lang);
    }

    fn list(&mut self, n: Node) {
        let items: Vec<Node> = n.children().filter(|c| c.kind() == T::ListItem).collect();
        let mut loose = items
            .windows(2)
            .any(|w| self.blank_between(w[0].to(), w[1].from()));
        for item in &items {
            let blocks: Vec<Node> = item
                .children()
                .filter(|c| !matches!(c.kind(), T::ListMark | T::QuoteMark))
                .collect();
            loose |= blocks
                .windows(2)
                .any(|w| self.blank_between(w[0].to(), w[1].from()));
        }
        self.cr();
        if n.kind() == T::OrderedList {
            let mark = items[0].children().next().unwrap();
            let digits: String = self
                .node_text(mark)
                .chars()
                .filter(char::is_ascii_digit)
                .collect();
            let start: u64 = digits.parse().unwrap_or(1);
            if start == 1 {
                self.out.push_str("<ol>");
            } else {
                self.out.push_str(&format!("<ol start=\"{start}\">"));
            }
        } else {
            self.out.push_str("<ul>");
        }
        self.cr();
        for item in items {
            self.out.push_str("<li>");
            self.blocks(item, !loose);
            self.out.push_str("</li>");
            self.cr();
        }
        self.out.push_str(if n.kind() == T::OrderedList {
            "</ol>"
        } else {
            "</ul>"
        });
        self.cr();
    }

    /// Whether a blank line (whitespace and quote marks only) lies between.
    fn blank_between(&self, from: usize, to: usize) -> bool {
        let s = self.text(from, to);
        let lines: Vec<&str> = s.split('\n').collect();
        lines.len() > 2
            && lines[1..lines.len() - 1]
                .iter()
                .any(|l| l.chars().all(|c| c == ' ' || c == '\t' || c == '>'))
    }

    fn task(&mut self, n: Node) {
        let marker = n.children().next().unwrap();
        let checked = self.node_text(marker).contains(['x', 'X']);
        self.out.push_str(if checked {
            "<input checked=\"\" disabled=\"\" type=\"checkbox\" /> "
        } else {
            "<input disabled=\"\" type=\"checkbox\" /> "
        });
        self.inline_block(n, marker.to(), n.to());
    }

    fn table(&mut self, n: Node) {
        let rows: Vec<Node> = n
            .children()
            .filter(|c| matches!(c.kind(), T::TableHeader | T::TableRow))
            .collect();
        let delim = n
            .children()
            .find(|c| c.kind() == T::TableDelimiter)
            .unwrap();
        let aligns: Vec<&str> = self
            .node_text(delim)
            .trim()
            .trim_matches('|')
            .split('|')
            .map(|c| {
                let c = c.trim();
                match (c.starts_with(':'), c.ends_with(':')) {
                    (true, true) => " align=\"center\"",
                    (true, false) => " align=\"left\"",
                    (false, true) => " align=\"right\"",
                    _ => "",
                }
            })
            .collect();
        self.cr();
        self.out.push_str("<table>\n<thead>\n");
        for (r, row) in rows.iter().enumerate() {
            if r == 1 {
                self.out.push_str("<tbody>\n");
            }
            let tag = if r == 0 { "th" } else { "td" };
            self.out.push_str("<tr>\n");
            let cells: Vec<Node> = row
                .children()
                .filter(|c| c.kind() == T::TableCell)
                .collect();
            for (k, align) in aligns.iter().enumerate() {
                self.out.push_str(&format!("<{tag}{align}>"));
                if let Some(cell) = cells.get(k) {
                    let start = self.out.len();
                    self.inline_block(*cell, cell.from(), cell.to());
                    let unpiped = self.out[start..].replace("\\|", "|");
                    self.out.truncate(start);
                    self.out.push_str(&unpiped);
                }
                self.out.push_str(&format!("</{tag}>\n"));
            }
            self.out.push_str("</tr>\n");
            if r == 0 {
                self.out.push_str("</thead>\n");
            }
        }
        if rows.len() > 1 {
            self.out.push_str("</tbody>\n");
        }
        self.out.push_str("</table>\n");
    }
}
