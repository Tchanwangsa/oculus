//! The renderer: the reference renderer's logic over Lezer's tree, split into
//! block and inline halves.

mod blocks;
mod inlines;

use std::collections::HashMap;

use oculus_editor_core::markdown::{Node, NodeType as T, Tree, parse};
use oculus_editor_core::text::Text;

use crate::text::{encode_url, normalize_label, unescape};
struct Renderer<'t> {
    src: Vec<u16>,
    entities: &'t HashMap<String, String>,
    defs: HashMap<String, (String, Option<String>)>,
    out: String,
    /// Inline output is at the start of a line (after a soft break).
    line_start: bool,
}

impl<'t> Renderer<'t> {
    fn new(source: &str, tree: &'t Tree, entities: &'t HashMap<String, String>) -> Self {
        let mut r = Renderer {
            src: source.encode_utf16().collect(),
            entities,
            defs: HashMap::new(),
            out: String::new(),
            line_start: false,
        };
        for node in tree.iter().filter(|n| n.kind() == T::LinkReference) {
            let label = node
                .children()
                .find(|c| c.kind() == T::LinkLabel)
                .map(|l| r.text(l.from() + 1, l.to() - 1));
            let dest = node
                .children()
                .find(|c| c.kind() == T::URL)
                .map(|u| r.dest(u))
                .unwrap_or_default();
            let title = node
                .children()
                .find(|c| c.kind() == T::LinkTitle)
                .map(|t| r.title(t));
            if let Some(label) = label {
                r.defs
                    .entry(normalize_label(&label))
                    .or_insert((dest, title));
            }
        }
        r
    }

    fn text(&self, from: usize, to: usize) -> String {
        String::from_utf16_lossy(&self.src[from.min(to)..to])
    }

    fn node_text(&self, n: Node) -> String {
        self.text(n.from(), n.to())
    }

    fn dest(&self, url: Node) -> String {
        let raw = self.node_text(url);
        let raw = raw
            .strip_prefix('<')
            .and_then(|r| r.strip_suffix('>'))
            .unwrap_or(&raw);
        encode_url(&unescape(raw, self.entities))
    }

    fn title(&self, t: Node) -> String {
        let raw = self.node_text(t);
        unescape(&raw[1..raw.len() - 1], self.entities)
    }

    fn cr(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
    }
}

pub(super) fn render(markdown: &str, entities: &HashMap<String, String>) -> String {
    let doc = Text::of(markdown);
    let tree = parse(&doc);
    let source = doc.to_string();
    let mut r = Renderer::new(&source, &tree, entities);
    r.blocks(tree.root(), false);
    r.out
}
