//! The renderer's inline half: spans, links, images, and the text between
//! nodes.

use oculus_editor_core::markdown::{Node, NodeType as T};

use super::Renderer;
use crate::text::{decode_entity, encode_url, escape_html, normalize_label, unescape};
impl Renderer<'_> {
    /// The inline content of leaf `n` over `from..to`, soft breaks with the
    /// whitespace around them removed, trailing whitespace dropped.
    pub(super) fn inline_block(&mut self, n: Node, from: usize, to: usize) {
        self.line_start = true;
        let start = self.out.len();
        self.inlines(n, from, to);
        let trimmed = self.out[start..].trim_end_matches([' ', '\t']).len();
        self.out.truncate(start + trimmed);
    }

    /// The children of `n` in `from..to`, with the text between them.
    fn inlines(&mut self, n: Node, from: usize, to: usize) {
        let mut pos = from;
        for child in n.children() {
            if child.to() <= from || child.from() >= to {
                continue;
            }
            self.gap(pos, child.from());
            self.inline(child);
            pos = child.to();
        }
        self.gap(pos, to);
    }

    fn gap(&mut self, from: usize, to: usize) {
        if from >= to {
            return;
        }
        let text = self.text(from, to);
        let mut lines = text.split('\n').peekable();
        while let Some(line) = lines.next() {
            let mut line = line;
            if self.line_start {
                line = line.trim_start_matches([' ', '\t']);
            }
            if lines.peek().is_some() {
                line = line.trim_end_matches([' ', '\t']);
                self.out.push_str(&escape_html(line));
                self.out.push('\n');
                self.line_start = true;
            } else {
                self.out.push_str(&escape_html(line));
                if !line.is_empty() {
                    self.line_start = false;
                }
            }
        }
    }

    fn inline(&mut self, n: Node) {
        let wrap = |r: &mut Self, open: &str, close: &str, mark: T| {
            r.out.push_str(open);
            let (from, to) = r.between_marks(n, mark);
            r.inlines(n, from, to);
            r.out.push_str(close);
        };
        match n.kind() {
            T::Escape => {
                let s = self.node_text(n);
                self.out.push_str(&escape_html(&s[1..]));
                self.line_start = false;
            }
            T::Entity => {
                let s = self.node_text(n);
                let decoded = decode_entity(&s, self.entities).map_or(s.clone(), |d| d.0);
                self.out.push_str(&escape_html(&decoded));
                self.line_start = false;
            }
            T::HardBreak => {
                self.out.push_str("<br />\n");
                self.line_start = true;
            }
            T::Emphasis => wrap(self, "<em>", "</em>", T::EmphasisMark),
            T::StrongEmphasis => wrap(self, "<strong>", "</strong>", T::EmphasisMark),
            T::Strikethrough => wrap(self, "<del>", "</del>", T::StrikethroughMark),
            T::InlineCode => {
                let (from, to) = self.between_marks(n, T::CodeMark);
                let mut code: String = self.text(from, to).replace('\n', " ");
                if code.len() >= 2
                    && code.starts_with(' ')
                    && code.ends_with(' ')
                    && !code.chars().all(|c| c == ' ')
                {
                    code = code[1..code.len() - 1].to_string();
                }
                self.out
                    .push_str(&format!("<code>{}</code>", escape_html(&code)));
                self.line_start = false;
            }
            T::Link | T::Image => self.link(n),
            T::Autolink => {
                let url = n.children().find(|c| c.kind() == T::URL).unwrap();
                let text = self.node_text(url);
                let href = if text.contains(':') {
                    text.clone()
                } else {
                    format!("mailto:{text}")
                };
                self.out.push_str(&format!(
                    "<a href=\"{}\">{}</a>",
                    escape_html(&encode_url(&href)),
                    escape_html(&text)
                ));
                self.line_start = false;
            }
            T::URL => {
                let text = self.node_text(n);
                let href = if text.starts_with("www.") {
                    format!("http://{text}")
                } else if text.contains(':') {
                    text.clone()
                } else {
                    format!("mailto:{text}")
                };
                self.out.push_str(&format!(
                    "<a href=\"{}\">{}</a>",
                    escape_html(&encode_url(&href)),
                    escape_html(&text)
                ));
                self.line_start = false;
            }
            T::HTMLTag | T::Comment | T::ProcessingInstruction => {
                let s = self.node_text(n);
                self.out.push_str(&s);
                self.line_start = false;
            }
            T::QuoteMark
            | T::ListMark
            | T::LinkMark
            | T::EmphasisMark
            | T::CodeMark
            | T::HeaderMark
            | T::TaskMarker => {}
            _ => {
                let s = self.node_text(n);
                self.out.push_str(&escape_html(&s));
                self.line_start = false;
            }
        }
    }

    /// The range between a node's first and last child of type `mark`.
    fn between_marks(&self, n: Node, mark: T) -> (usize, usize) {
        let marks: Vec<Node> = n.children().filter(|c| c.kind() == mark).collect();
        match (marks.first(), marks.last()) {
            (Some(a), Some(b)) if marks.len() >= 2 => (a.to(), b.from()),
            _ => (n.from(), n.to()),
        }
    }

    fn link(&mut self, n: Node) {
        let image = n.kind() == T::Image;
        let marks: Vec<Node> = n.children().filter(|c| c.kind() == T::LinkMark).collect();
        let open = marks[0];
        let close = marks
            .iter()
            .find(|m| m.from() > open.from() && self.node_text(**m) == "]")
            .copied();
        let Some(close) = close else {
            self.inlines(n, n.from(), n.to());
            return;
        };
        let paren = marks
            .iter()
            .any(|m| m.from() >= close.to() && self.node_text(*m) == "(");
        let label = n.children().find(|c| c.kind() == T::LinkLabel);
        let target = if paren {
            let dest = n
                .children()
                .find(|c| c.kind() == T::URL)
                .map(|u| self.dest(u))
                .unwrap_or_default();
            let title = n
                .children()
                .find(|c| c.kind() == T::LinkTitle)
                .map(|t| self.title(t));
            Some((dest, title))
        } else {
            let key = match label {
                Some(l) if l.to() - l.from() > 2 => self.text(l.from() + 1, l.to() - 1),
                _ => self.text(open.to(), close.from()),
            };
            self.defs.get(&normalize_label(&key)).cloned()
        };
        let Some((dest, title)) = target else {
            // Not a link: its brackets are text, its content still inline.
            self.out.push_str(if image { "![" } else { "[" });
            self.inlines(n, open.to(), close.from());
            self.out.push(']');
            if let Some(l) = label {
                self.out
                    .push_str(&escape_html(&unescape(&self.node_text(l), self.entities)));
            }
            self.line_start = false;
            return;
        };
        let title_attr = title.map_or(String::new(), |t| format!(" title=\"{}\"", escape_html(&t)));
        if image {
            let start = self.out.len();
            self.inlines(n, open.to(), close.from());
            let inner = self.out.split_off(start);
            let alt = strip_tags(&inner);
            self.out.push_str(&format!(
                "<img src=\"{}\" alt=\"{}\"{} />",
                escape_html(&dest),
                alt,
                title_attr
            ));
        } else {
            self.out.push_str(&format!(
                "<a href=\"{}\"{}>",
                escape_html(&dest),
                title_attr
            ));
            self.inlines(n, open.to(), close.from());
            self.out.push_str("</a>");
        }
        self.line_start = false;
    }
}

/// The plain text of rendered inline HTML: tags dropped, an image's alt
/// text kept.
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let end = rest[i..].find('>').map_or(rest.len(), |e| i + e + 1);
        let tag = &rest[i..end];
        if tag.starts_with("<img")
            && let Some(a) = tag.find(" alt=\"")
        {
            let alt = &tag[a + 6..];
            out.push_str(&alt[..alt.find('"').unwrap_or(alt.len())]);
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}
