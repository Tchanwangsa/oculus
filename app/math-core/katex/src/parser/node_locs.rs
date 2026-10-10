//! Node locations for source mapping (`Settings::source_map`).
//!
//! With the setting on, every node's `loc` is a range of the formula's own
//! input that holds the locations of all its children, so the edit field can
//! map a rendered element back to the source it came from. The parser builds
//! most of those ranges where it makes the node; [`AnyParseNode::cover_children`]
//! then widens each node over its children, for nodes whose handler parsed a
//! body after its arguments (`\color{red} x`, `\bf ab`, `\displaystyle`).

use super::parse_node::{AnyParseNode, ParseNodeArrayTag, ParseNodeOp};
use crate::types::{ErrorLocationProvider as _, SourceLocation};

impl AnyParseNode {
    /// The node's source location, for writing.
    pub const fn loc_mut(&mut self) -> &mut Option<SourceLocation> {
        match self {
            Self::Array(node) => &mut node.loc,
            Self::OrdGroup(node) => &mut node.loc,
            Self::SupSub(node) => &mut node.loc,
            Self::Genfrac(node) => &mut node.loc,
            Self::LeftRight(node) => &mut node.loc,
            Self::LeftRightRight(node) => &mut node.loc,
            Self::Sqrt(node) => &mut node.loc,
            Self::Atom(node) => &mut node.loc,
            Self::MathOrd(node) => &mut node.loc,
            Self::Op(ParseNodeOp::Symbol { loc, .. } | ParseNodeOp::Body { loc, .. }) => loc,
            Self::Spacing(node) => &mut node.loc,
            Self::Text(node) => &mut node.loc,
            Self::Styling(node) => &mut node.loc,
            Self::Font(node) => &mut node.loc,
            Self::Color(node) => &mut node.loc,
            Self::Accent(node) => &mut node.loc,
            Self::Overline(node) => &mut node.loc,
            Self::Underline(node) => &mut node.loc,
            Self::Phantom(node) => &mut node.loc,
            Self::Hphantom(node) => &mut node.loc,
            Self::Vphantom(node) => &mut node.loc,
            Self::Rule(node) => &mut node.loc,
            Self::CdLabel(node) => &mut node.loc,
            Self::CdLabelParent(node) => &mut node.loc,
            Self::ColorToken(node) => &mut node.loc,
            Self::Raw(node) => &mut node.loc,
            Self::Size(node) => &mut node.loc,
            Self::Tag(node) => &mut node.loc,
            Self::Url(node) => &mut node.loc,
            Self::Verb(node) => &mut node.loc,
            Self::TextOrd(node) => &mut node.loc,
            Self::AccentToken(node) => &mut node.loc,
            Self::OpToken(node) => &mut node.loc,
            Self::AccentUnder(node) => &mut node.loc,
            Self::Cr(node) => &mut node.loc,
            Self::Delimsizing(node) => &mut node.loc,
            Self::Enclose(node) => &mut node.loc,
            Self::Environment(node) => &mut node.loc,
            Self::Hbox(node) => &mut node.loc,
            Self::HorizBrace(node) => &mut node.loc,
            Self::Href(node) => &mut node.loc,
            Self::Html(node) => &mut node.loc,
            Self::HtmlMathMl(node) => &mut node.loc,
            Self::Includegraphics(node) => &mut node.loc,
            Self::Infix(node) => &mut node.loc,
            Self::Internal(node) => &mut node.loc,
            Self::Kern(node) => &mut node.loc,
            Self::Lap(node) => &mut node.loc,
            Self::MathChoice(node) => &mut node.loc,
            Self::Middle(node) => &mut node.loc,
            Self::Mclass(node) => &mut node.loc,
            Self::OperatorName(node) => &mut node.loc,
            Self::Pmb(node) => &mut node.loc,
            Self::Raisebox(node) => &mut node.loc,
            Self::Sizing(node) => &mut node.loc,
            Self::Smash(node) => &mut node.loc,
            Self::Vcenter(node) => &mut node.loc,
            Self::XArrow(node) => &mut node.loc,
        }
    }

    /// The node's direct children, not always in source order: a SupSub
    /// gives base, superscript, subscript; an array its cells row by row,
    /// then its row tags; a `\tag` its body, then its tag.
    #[must_use]
    pub fn children(&self) -> Vec<&Self> {
        let mut out = Vec::new();
        match self {
            Self::Array(node) => {
                out.extend(node.body.iter().flatten());
                for tag in node.tags.iter().flatten() {
                    if let ParseNodeArrayTag::Nodes(nodes) = tag {
                        out.extend(nodes);
                    }
                }
            }
            Self::OrdGroup(node) => out.extend(&node.body),
            Self::SupSub(node) => {
                out.extend(node.base.as_deref());
                out.extend(node.sup.as_deref());
                out.extend(node.sub.as_deref());
            }
            Self::Genfrac(node) => out.extend([&*node.numer, &*node.denom]),
            Self::LeftRight(node) => out.extend(&node.body),
            Self::Sqrt(node) => {
                out.extend(node.index.as_ref());
                out.push(&node.body);
            }
            Self::Op(ParseNodeOp::Body { body, .. }) => out.extend(body),
            Self::Text(node) => out.extend(&node.body),
            Self::Styling(node) => out.extend(&node.body),
            Self::Font(node) => out.push(&node.body),
            Self::Color(node) => out.extend(&node.body),
            Self::Accent(node) => out.push(&node.base),
            Self::Overline(node) => out.push(&node.body),
            Self::Underline(node) => out.push(&node.body),
            Self::Phantom(node) => out.extend(&node.body),
            Self::Hphantom(node) => out.push(&node.body),
            Self::Vphantom(node) => out.push(&node.body),
            Self::CdLabel(node) => out.push(&node.label),
            Self::CdLabelParent(node) => out.push(&node.fragment),
            Self::Tag(node) => out.extend(node.body.iter().chain(&node.tag)),
            Self::AccentUnder(node) => out.push(&node.base),
            Self::Enclose(node) => out.push(&node.body),
            Self::Environment(node) => out.push(&node.name_group),
            Self::Hbox(node) => out.extend(&node.body),
            Self::HorizBrace(node) => out.push(&node.base),
            Self::Href(node) => out.extend(&node.body),
            Self::Html(node) => out.extend(&node.body),
            Self::HtmlMathMl(node) => out.extend(node.html.iter().chain(&node.mathml)),
            Self::Lap(node) => out.push(&node.body),
            Self::MathChoice(node) => out.extend(
                node.display
                    .iter()
                    .chain(&node.text)
                    .chain(&node.script)
                    .chain(&node.scriptscript),
            ),
            Self::Mclass(node) => out.extend(&node.body),
            Self::OperatorName(node) => out.extend(&node.body),
            Self::Pmb(node) => out.extend(&node.body),
            Self::Raisebox(node) => out.push(&node.body),
            Self::Sizing(node) => out.extend(&node.body),
            Self::Smash(node) => out.push(&node.body),
            Self::Vcenter(node) => out.push(&node.body),
            Self::XArrow(node) => {
                out.extend(node.below.as_deref());
                out.extend(node.body.as_deref());
            }
            Self::LeftRightRight(_)
            | Self::Atom(_)
            | Self::MathOrd(_)
            | Self::Op(ParseNodeOp::Symbol { .. })
            | Self::Spacing(_)
            | Self::Rule(_)
            | Self::ColorToken(_)
            | Self::Raw(_)
            | Self::Size(_)
            | Self::Url(_)
            | Self::Verb(_)
            | Self::TextOrd(_)
            | Self::AccentToken(_)
            | Self::OpToken(_)
            | Self::Cr(_)
            | Self::Delimsizing(_)
            | Self::Includegraphics(_)
            | Self::Infix(_)
            | Self::Internal(_)
            | Self::Kern(_)
            | Self::Middle(_) => {}
        }
        out
    }

    /// [`Self::children`], for writing.
    pub fn children_mut(&mut self) -> Vec<&mut Self> {
        let mut out = Vec::new();
        match self {
            Self::Array(node) => {
                out.extend(node.body.iter_mut().flatten());
                for tag in node.tags.iter_mut().flatten() {
                    if let ParseNodeArrayTag::Nodes(nodes) = tag {
                        out.extend(nodes);
                    }
                }
            }
            Self::OrdGroup(node) => out.extend(&mut node.body),
            Self::SupSub(node) => {
                out.extend(node.base.as_deref_mut());
                out.extend(node.sup.as_deref_mut());
                out.extend(node.sub.as_deref_mut());
            }
            Self::Genfrac(node) => {
                let node = &mut **node;
                out.extend([&mut *node.numer, &mut *node.denom]);
            }
            Self::LeftRight(node) => out.extend(&mut node.body),
            Self::Sqrt(node) => {
                let node = &mut **node;
                out.extend(node.index.as_mut());
                out.push(&mut node.body);
            }
            Self::Op(ParseNodeOp::Body { body, .. }) => out.extend(body),
            Self::Text(node) => out.extend(&mut node.body),
            Self::Styling(node) => out.extend(&mut node.body),
            Self::Font(node) => out.push(&mut node.body),
            Self::Color(node) => out.extend(&mut node.body),
            Self::Accent(node) => out.push(&mut node.base),
            Self::Overline(node) => out.push(&mut node.body),
            Self::Underline(node) => out.push(&mut node.body),
            Self::Phantom(node) => out.extend(&mut node.body),
            Self::Hphantom(node) => out.push(&mut node.body),
            Self::Vphantom(node) => out.push(&mut node.body),
            Self::CdLabel(node) => out.push(&mut node.label),
            Self::CdLabelParent(node) => out.push(&mut node.fragment),
            Self::Tag(node) => out.extend(node.body.iter_mut().chain(&mut node.tag)),
            Self::AccentUnder(node) => out.push(&mut node.base),
            Self::Enclose(node) => out.push(&mut node.body),
            Self::Environment(node) => out.push(&mut node.name_group),
            Self::Hbox(node) => out.extend(&mut node.body),
            Self::HorizBrace(node) => out.push(&mut node.base),
            Self::Href(node) => out.extend(&mut node.body),
            Self::Html(node) => out.extend(&mut node.body),
            Self::HtmlMathMl(node) => out.extend(node.html.iter_mut().chain(&mut node.mathml)),
            Self::Lap(node) => out.push(&mut node.body),
            Self::MathChoice(node) => out.extend(
                node.display
                    .iter_mut()
                    .chain(&mut node.text)
                    .chain(&mut node.script)
                    .chain(&mut node.scriptscript),
            ),
            Self::Mclass(node) => out.extend(&mut node.body),
            Self::OperatorName(node) => out.extend(&mut node.body),
            Self::Pmb(node) => out.extend(&mut node.body),
            Self::Raisebox(node) => out.push(&mut node.body),
            Self::Sizing(node) => out.extend(&mut node.body),
            Self::Smash(node) => out.push(&mut node.body),
            Self::Vcenter(node) => out.push(&mut node.body),
            Self::XArrow(node) => {
                out.extend(node.below.as_deref_mut());
                out.extend(node.body.as_deref_mut());
            }
            Self::LeftRightRight(_)
            | Self::Atom(_)
            | Self::MathOrd(_)
            | Self::Op(ParseNodeOp::Symbol { .. })
            | Self::Spacing(_)
            | Self::Rule(_)
            | Self::ColorToken(_)
            | Self::Raw(_)
            | Self::Size(_)
            | Self::Url(_)
            | Self::Verb(_)
            | Self::TextOrd(_)
            | Self::AccentToken(_)
            | Self::OpToken(_)
            | Self::Cr(_)
            | Self::Delimsizing(_)
            | Self::Includegraphics(_)
            | Self::Infix(_)
            | Self::Internal(_)
            | Self::Kern(_)
            | Self::Middle(_) => {}
        }
        out
    }

    /// Widens this node's location, and every descendant's, to hold its
    /// children's (bottom-up). Locations in another input are left out, and a
    /// node with no location takes its children's span.
    pub fn cover_children(&mut self) {
        let mut span = self.loc().cloned();
        for child in self.children_mut() {
            child.cover_children();
            span = SourceLocation::cover(span, child.loc());
        }
        if span.as_ref() != self.loc() {
            *self.loc_mut() = span;
        }
    }

    /// [`Self::cover_children`], then `empty` if the node still has no
    /// location: an empty group gets the empty range where its content
    /// would go.
    pub fn cover_children_or(&mut self, empty: Option<SourceLocation>) {
        self.cover_children();
        if self.loc().is_none() {
            *self.loc_mut() = empty;
        }
    }

    /// Gives `loc` to this node and every descendant that has none: pieces
    /// the parser made up for a construct written as one unit.
    pub fn fill_missing_locs(&mut self, loc: &SourceLocation) {
        if self.loc().is_none() {
            *self.loc_mut() = Some(loc.clone());
        }
        for child in self.children_mut() {
            child.fill_missing_locs(loc);
        }
    }

    /// Sets the location of this node, and of each single-child wrapper
    /// below it that shares its old location, to `loc`. Used where a
    /// construct's end is only known after its node is built (an
    /// environment's `\end{…}`).
    pub fn widen_wrappers(&mut self, loc: &SourceLocation) {
        let old = self.loc().cloned();
        *self.loc_mut() = Some(loc.clone());
        let mut node = self;
        loop {
            let mut children = node.children_mut();
            if children.len() != 1 {
                return;
            }
            let Some(child) = children.pop() else { return };
            if child.loc().cloned() != old {
                return;
            }
            *child.loc_mut() = Some(loc.clone());
            node = child;
        }
    }
}
