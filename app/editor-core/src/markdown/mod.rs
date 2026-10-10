//! The note grammar: a port of `@lezer/markdown` 1.7.2 configured as the app
//! configures it (`editor/core/language.ts`: CommonMark plus Table,
//! Strikethrough, TaskList, Autolink, maths, frontmatter). Fenced code is a
//! leaf here; the nested code languages stay in JS. `oracle/markdown.ts`
//! checks every tree against Lezer's.
//!
//! The parser reads the document's text and works in bytes; the tree it
//! returns counts UTF-16 units, as every position at this crate's API does.
//!
//! Ported from `@lezer/markdown` (MIT, Copyright (C) 2020 Marijn Haverbeke
//! and others) and `@lezer/common`'s tree navigation (MIT, Copyright (C) 2018
//! Marijn Haverbeke and others); the licence text is in the crate's `NOTICE`.

mod block;
mod chars;
mod html;
mod incremental;
mod inline;
mod tables;
mod tree;

pub use incremental::reparse;
pub use tables::NodeType;
pub use tree::{Node, Tree};

use crate::text::Text;

/// Parses `doc` from scratch.
pub fn parse(doc: &Text) -> Tree {
    let mut src = String::with_capacity(doc.len());
    for chunk in doc.chunks() {
        src.push_str(chunk);
    }
    parse_str(&src)
}

/// Parses `src`, which must have `\n` line breaks only (as a `Text`'s
/// contents do); `parse` is the public way in.
pub(crate) fn parse_str(src: &str) -> Tree {
    let root = block::BlockContext::new(src).parse();
    let map = chars::Utf16Map::new(src);
    Tree::from_element(&root, |byte| map.get(byte))
}

#[cfg(test)]
mod tests;
