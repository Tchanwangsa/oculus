//! Source mapping for the app's edit field (`Settings::source_map`).
//!
//! `build_html::build_group` gives each node's element `data-s`/`data-e`:
//! the node's source range as **UTF-16 code-unit offsets** into the formula
//! (what the DOM and CodeMirror count in; the markup is the only place the
//! offsets leave the crate, so they are converted once, here). Glyphs from
//! different nodes are never merged (`build_common::push_combine_chars`), and
//! an empty group draws a placeholder glyph so the caret has a box to sit in.
//! With the setting off, `Options::source_map` is `None` and none of this
//! runs.

use alloc::sync::Arc;

use crate::KatexContext;
use crate::build_common::make_symbol;
use crate::dom_tree::{HtmlDomNode, SymbolNode};
use crate::namespace::AttrMap;
use crate::options::Options;
use crate::parser::parse_node::AnyParseNode;
use crate::types::{ClassList, ErrorLocationProvider, Mode, ParseError, SourceLocation};

/// The placeholder's class. KaTeX's own inner classes are prefixed
/// `katex-`; ours are `oc-`.
pub const PLACEHOLDER_CLASS: &str = "oc-placeholder";

/// The placeholder glyph: AMS `\square`, which has real font metrics, so
/// fraction and script layouts build around it as around any symbol.
const PLACEHOLDER_GLYPH: &str = "\u{25a1}";

/// A source range in UTF-16 code units, `start..end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceRange {
    /// First code unit.
    pub start: usize,
    /// One past the last code unit.
    pub end: usize,
}

/// The formula's own input, against which node locations are mapped.
#[derive(Debug, PartialEq, Eq)]
pub struct SourceInput {
    input: Arc<str>,
    ascii: bool,
}

impl SourceInput {
    /// The input of the parsed `tree`: the parser's own `Arc` when a
    /// top-level node carries it (so most checks are a pointer compare),
    /// else a copy of `expression`.
    #[must_use]
    pub fn find(tree: &[AnyParseNode], expression: &str) -> Self {
        let parsed = tree
            .iter()
            .flat_map(|node| match node {
                AnyParseNode::Tag(tag) => tag.body.iter().collect(),
                other => alloc::vec![other],
            })
            .filter_map(ErrorLocationProvider::loc)
            .find(|loc| &*loc.input == expression)
            .map(|loc| Arc::clone(&loc.input));
        Self::new(parsed.unwrap_or_else(|| Arc::from(expression)))
    }

    /// Maps locations into `input`.
    #[must_use]
    pub fn new(input: Arc<str>) -> Self {
        let ascii = input.is_ascii();
        Self { input, ascii }
    }

    /// Whether `input` is this formula's, not some other text a location can
    /// point into (a macro body the parser did not retarget).
    fn owns(&self, input: &Arc<str>) -> bool {
        Arc::ptr_eq(&self.input, input)
            || (self.input.len() == input.len() && *self.input == **input)
    }

    /// A byte offset as UTF-16 code units; `None` off a char boundary or past
    /// the end.
    fn units(&self, byte: usize) -> Option<usize> {
        if self.ascii {
            (byte <= self.input.len()).then_some(byte)
        } else {
            self.input.get(..byte).map(|s| s.encode_utf16().count())
        }
    }

    /// `loc` in UTF-16 code units, or `None` when it points elsewhere or is
    /// malformed.
    #[must_use]
    pub fn range(&self, loc: &SourceLocation) -> Option<SourceRange> {
        if !self.owns(&loc.input) || loc.start > loc.end {
            return None;
        }
        Some(SourceRange {
            start: self.units(loc.start)?,
            end: self.units(loc.end)?,
        })
    }

    /// Where the caret sits in an empty slot `loc` covers: just inside its
    /// braces or brackets when it has them (`{}`, `\text{}`'s argument,
    /// `\sqrt[]`'s index), else its start. A zero-width range.
    #[must_use]
    pub fn slot(&self, loc: &SourceLocation) -> Option<SourceRange> {
        self.range(loc)?;
        let text = self.input.get(loc.start..loc.end)?;
        let open = match text.chars().next_back() {
            Some('}') => text.rfind('{'),
            Some(']') => text.rfind('['),
            _ => None,
        };
        let at = open.map_or(loc.start, |open| loc.start + open + 1);
        let at = self.units(at)?;
        Some(SourceRange { start: at, end: at })
    }
}

/// Whether a glyph `next` that directly follows `prev` in the source is a
/// combining mark on it.
///
/// Such a pair stays one glyph with the joint range: a caret never sits
/// inside a cluster, and a mark in an element of its own may not shape onto
/// its base. The marks are those of the general combining blocks and of Thai
/// and Lao (the lexer already keeps U+0300–036F with their base).
#[must_use]
pub fn continues_cluster(
    prev: Option<SourceRange>,
    next: Option<SourceRange>,
    next_text: &str,
) -> bool {
    let (Some(prev), Some(next)) = (prev, next) else {
        return false;
    };
    prev.end == next.start
        && next_text.chars().next().is_some_and(|c| {
            matches!(
                c as u32,
                0x0300..=0x036F
                    | 0x0E31
                    | 0x0E34..=0x0E3A
                    | 0x0E47..=0x0E4E
                    | 0x0EB1
                    | 0x0EB4..=0x0EBC
                    | 0x0EC8..=0x0ECE
                    | 0x1AB0..=0x1AFF
                    | 0x1DC0..=0x1DFF
                    | 0x200D
                    | 0x20D0..=0x20FF
                    | 0xFE00..=0xFE0F
                    | 0xFE20..=0xFE2F
            )
        })
}

/// `node`'s source range, when mapping is on and its location is in the
/// formula.
#[must_use]
pub fn node_range(options: &Options, node: &AnyParseNode) -> Option<SourceRange> {
    let source = options.source_map.as_ref()?;
    source.range(ErrorLocationProvider::loc(node)?)
}

fn set_attributes(attributes: &mut AttrMap, range: SourceRange) {
    attributes.insert("data-s".to_owned(), range.start.to_string());
    attributes.insert("data-e".to_owned(), range.end.to_string());
}

/// Whether `node` already carries a range.
fn has_range(node: &HtmlDomNode) -> bool {
    match node {
        HtmlDomNode::DomSpan(span) => span.attributes.contains_key("data-s"),
        HtmlDomNode::Anchor(anchor) => anchor.attributes.contains_key("data-s"),
        HtmlDomNode::SvgNode(svg) => svg.attributes.contains_key("data-s"),
        HtmlDomNode::Symbol(symbol) => symbol.source.is_some(),
        HtmlDomNode::Img(_) | HtmlDomNode::MathML(_) | HtmlDomNode::Fragment(_) => false,
    }
}

/// Gives `node` the range of the parse node it was built from.
///
/// An element that already has one (a builder returned a child's element as
/// its own) takes the outer node's. A fragment is no element: each direct
/// child without a range takes it, and no wrapper is added, since the
/// inter-atom spacing reads top-level classes and vlists are
/// position-sensitive.
pub fn tag(node: &mut HtmlDomNode, range: SourceRange) {
    match node {
        HtmlDomNode::DomSpan(span) => set_attributes(&mut span.attributes, range),
        HtmlDomNode::Anchor(anchor) => set_attributes(&mut anchor.attributes, range),
        HtmlDomNode::SvgNode(svg) => set_attributes(&mut svg.attributes, range),
        HtmlDomNode::Symbol(symbol) => symbol.source = Some(range),
        HtmlDomNode::Fragment(fragment) => {
            for child in &mut fragment.children {
                if !has_range(child) {
                    tag(child, range);
                }
            }
        }
        HtmlDomNode::Img(_) | HtmlDomNode::MathML(_) => {}
    }
}

/// The placeholder for an empty slot that `loc` covers, or `None` when
/// mapping is off or the slot has no location in the formula (a group the
/// parser made up, such as `aligned`'s spacing `{}`).
pub fn placeholder(
    ctx: &KatexContext,
    options: &Options,
    loc: Option<&SourceLocation>,
) -> Result<Option<SymbolNode>, ParseError> {
    let Some(slot) = options
        .source_map
        .as_ref()
        .zip(loc)
        .and_then(|(source, loc)| source.slot(loc))
    else {
        return Ok(None);
    };
    let mut symbol = make_symbol(
        ctx,
        PLACEHOLDER_GLYPH,
        "AMS-Regular",
        Mode::Math,
        Some(options),
        ClassList::Const(&["mord", "amsrm", PLACEHOLDER_CLASS]),
    )?;
    symbol.source = Some(slot);
    Ok(Some(symbol))
}
