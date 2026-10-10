//! Which slots each kind of atom has. `None` means the atom's source does
//! not hold its arguments where the node says (a macro's output posing as
//! a structure): it is then one atom with no stops inside.

use core::ops::Range;

use katex::parser::parse_node::{AnyParseNode, ParseNodeArray, ParseNodeGenfrac, ParseNodeOp};

use super::{SlotSpec, is_text, source::Source, within};
use crate::slot::{Bounds, SlotKind};

/// The slots of `node`, an atom covering `range`, in any order.
#[expect(
    clippy::match_same_arms,
    reason = "one arm per variant, each with its reason"
)]
pub(super) fn slots<'n>(
    src: Source,
    node: &'n AnyParseNode,
    range: Range<usize>,
) -> Option<Vec<SlotSpec<'n>>> {
    use AnyParseNode as N;
    match node {
        // A `CD` diagram's cells are its arrows' syntax as much as its
        // content, and an edit in one reshapes the rest: edited as TeX.
        N::Array(_) if src.text(range.clone()).starts_with(r"\begin{CD}") => None,
        N::Array(array) => cells(src, array, range),
        N::OrdGroup(group) => {
            let interior = if src.wrapped(&range, '{', '}') {
                range.start + 1..range.end - 1
            } else if src.command_at(range.start) == Some(r"\begingroup") {
                let end = range.start + src.text(range.clone()).rfind(r"\endgroup")?;
                src.token_end(range.start)..end
            } else {
                return None;
            };
            let elements: Vec<&AnyParseNode> = group.body.iter().collect();
            Some(vec![spec(
                SlotKind::Group,
                Bounds::Delimited,
                interior,
                Some(node),
                elements,
            )])
        }
        // Flattened into the run it sits in (base, then one atom per
        // script); never an atom itself.
        N::SupSub(_) => None,
        N::Genfrac(frac) => fraction(src, frac, range),
        N::LeftRight(left_right) => {
            if src.command_at(range.start) != Some(r"\left") {
                return command_argument(src, range, &node.children(), SlotKind::Body)
                    .map(|spec| vec![spec]);
            }
            let start = src.token_end(src.token_end(range.start));
            let end = range.start + src.text(range).rfind(r"\right")?;
            let elements: Vec<&AnyParseNode> = left_right.body.iter().collect();
            (start <= end).then(|| {
                vec![spec(
                    SlotKind::LeftRight,
                    Bounds::Delimited,
                    start..end,
                    Some(node),
                    elements,
                )]
            })
        }
        // `\right`'s delimiter while its `\left` is parsed; never in a
        // finished tree.
        N::LeftRightRight(_) => Some(Vec::new()),
        N::Sqrt(sqrt) => {
            let mut out = vec![argument(
                src,
                range.clone(),
                &sqrt.body,
                SlotKind::Radicand,
            )?];
            if let Some(index) = &sqrt.index {
                out.push(argument(src, range, index, SlotKind::Index)?);
            }
            Some(out)
        }
        // One symbol each: the caret goes before or after, never inside.
        N::Atom(_) | N::MathOrd(_) | N::TextOrd(_) | N::Spacing(_) => Some(Vec::new()),
        N::Op(ParseNodeOp::Symbol { .. }) => Some(Vec::new()),
        N::Op(ParseNodeOp::Body { body, .. }) => body_argument(src, range, body, SlotKind::Body),
        N::Text(text) => {
            let mut specs = body_argument(src, range, &text.body, SlotKind::Text)?;
            for spec in &mut specs {
                spec.text = true;
            }
            Some(specs)
        }
        N::Styling(styling) => {
            // `$…$` or `\(…\)` in text; switches and wrappers were
            // flattened before they could become atoms.
            let written = src.text(range.clone());
            let open = if written.starts_with('$') {
                1
            } else if written.starts_with(r"\(") {
                2
            } else {
                return None;
            };
            let interior = range.start + open..range.end.checked_sub(open)?;
            let elements: Vec<&AnyParseNode> = styling.body.iter().collect();
            let mut spec = spec(SlotKind::Math, Bounds::Delimited, interior, None, elements);
            spec.text = false;
            Some(vec![spec])
        }
        N::Font(font) => Some(vec![argument(src, range, &font.body, SlotKind::Body)?]),
        N::Color(color) => {
            // `\color` is a switch (flattened); `\textcolor` and colour
            // macros (`\blue{x}`) take an argument.
            body_argument(src, range, &color.body, SlotKind::Body)
        }
        N::Accent(accent) => Some(vec![argument(src, range, &accent.base, SlotKind::Body)?]),
        N::AccentUnder(accent) => Some(vec![argument(src, range, &accent.base, SlotKind::Body)?]),
        N::HorizBrace(brace) => Some(vec![argument(src, range, &brace.base, SlotKind::Body)?]),
        N::Overline(line) => Some(vec![argument(src, range, &line.body, SlotKind::Body)?]),
        N::Underline(line) => Some(vec![argument(src, range, &line.body, SlotKind::Body)?]),
        N::Phantom(phantom) => body_argument(src, range, &phantom.body, SlotKind::Body),
        N::Hphantom(phantom) => Some(vec![argument(src, range, &phantom.body, SlotKind::Body)?]),
        N::Vphantom(phantom) => Some(vec![argument(src, range, &phantom.body, SlotKind::Body)?]),
        N::Enclose(enclose) => Some(vec![argument(src, range, &enclose.body, SlotKind::Body)?]),
        N::Lap(lap) => Some(vec![argument(src, range, &lap.body, SlotKind::Body)?]),
        N::Raisebox(raisebox) => Some(vec![argument(src, range, &raisebox.body, SlotKind::Body)?]),
        N::Smash(smash) => Some(vec![argument(src, range, &smash.body, SlotKind::Body)?]),
        N::Vcenter(vcenter) => Some(vec![argument(src, range, &vcenter.body, SlotKind::Body)?]),
        N::Hbox(hbox) => body_argument(src, range, &hbox.body, SlotKind::Text),
        N::Href(href) => body_argument(src, range, &href.body, SlotKind::Body),
        N::Html(html) => body_argument(src, range, &html.body, SlotKind::Body),
        N::Pmb(pmb) => body_argument(src, range, &pmb.body, SlotKind::Body),
        N::OperatorName(name) => body_argument(src, range, &name.body, SlotKind::Body),
        N::Mclass(mclass) => match src.command_at(range.start) {
            Some(r"\overset" | r"\stackrel") => {
                group_arguments(src, node, range, &[SlotKind::Above, SlotKind::Body])
            }
            Some(r"\underset") => {
                group_arguments(src, node, range, &[SlotKind::Below, SlotKind::Body])
            }
            _ => body_argument(src, range, &mclass.body, SlotKind::Body),
        },
        N::MathChoice(_) => group_arguments(src, node, range, &[SlotKind::Body; 4]),
        N::XArrow(arrow) => {
            // A CD arrow (`@>f>>`) has labels too, but no braces to edit
            // them in: it is edited as TeX.
            if !src
                .command_at(range.start)
                .is_some_and(|word| word.starts_with(r"\x"))
            {
                return None;
            }
            let mut out = Vec::new();
            if let Some(body) = &arrow.body {
                out.push(argument(src, range.clone(), body, SlotKind::Above)?);
            }
            if let Some(below) = &arrow.below {
                out.push(argument(src, range, below, SlotKind::Below)?);
            }
            Some(out)
        }
        // A CD arrow's pieces: edited as TeX, like the arrow.
        N::CdLabel(_) | N::CdLabelParent(_) => None,
        // Flattened at the top level (its body joins the row, its label is
        // an atom of its own); never an atom itself.
        N::Tag(_) => None,
        // A row break: the top level splits its rows at it; elsewhere a
        // single atom.
        N::Cr(_) => Some(Vec::new()),
        // Arguments that are not maths: dimensions, colour names, URLs,
        // file names, verbatim text, delimiters, raw strings.
        N::Rule(_)
        | N::Kern(_)
        | N::Size(_)
        | N::ColorToken(_)
        | N::Raw(_)
        | N::Url(_)
        | N::Verb(_)
        | N::Includegraphics(_)
        | N::Delimsizing(_)
        | N::Middle(_) => Some(Vec::new()),
        // Single symbols an accent or operator is made from.
        N::AccentToken(_) | N::OpToken(_) => Some(Vec::new()),
        // Macro output in two renderings (`\not`, `\neq`): one atom.
        N::HtmlMathMl(_) => None,
        // Parser intermediates; one left in a tree (`\over^]`) is source
        // that does not render.
        N::Infix(_) | N::Internal(_) | N::Environment(_) => None,
        // A switch (`\large`), flattened into its run; an atom only when
        // a macro's output, and then one with no stops inside.
        N::Sizing(_) => None,
    }
}

/// [`command_argument`] for a node's own body.
fn body_argument<'n>(
    src: Source,
    range: Range<usize>,
    body: &'n [AnyParseNode],
    kind: SlotKind,
) -> Option<Vec<SlotSpec<'n>>> {
    let children: Vec<&AnyParseNode> = body.iter().collect();
    command_argument(src, range, &children, kind).map(|spec| vec![spec])
}

fn spec<'n>(
    kind: SlotKind,
    bounds: Bounds,
    interior: Range<usize>,
    container: Option<&AnyParseNode>,
    elements: Vec<&'n AnyParseNode>,
) -> SlotSpec<'n> {
    let text = is_text(container, &elements);
    SlotSpec {
        kind,
        bounds,
        interior,
        text,
        elements,
    }
}

/// A one-node argument of the atom at `owner`: a `{…}` or `[…]` group's
/// content, or a bare token. A node with the owner's own range comes
/// from a macro's body; its argument is then the invocation's last group.
pub(super) fn argument<'n>(
    src: Source,
    owner: Range<usize>,
    node: &'n AnyParseNode,
    kind: SlotKind,
) -> Option<SlotSpec<'n>> {
    let node = unwrap(src, node);
    let range = src.range(node)?;
    if range == owner || !within(&range, &owner) {
        return command_argument(src, owner, &node.children(), kind);
    }
    // KaTeX unwraps a one-atom group argument (`\hat{x}`'s base is the
    // `x`, `\hat{{}}`'s the inner `{}`): its braces are still in the
    // source, around the node, which is then an atom of the argument.
    let braced = src.0[..range.start].ends_with('{')
        && src.0[range.end..].starts_with('}')
        && within(&(range.start - 1..range.end + 1), &owner);
    if braced {
        return Some(spec(kind, Bounds::Delimited, range, Some(node), vec![node]));
    }
    if let AnyParseNode::OrdGroup(group) = node
        && (src.wrapped(&range, '{', '}') || src.wrapped(&range, '[', ']'))
    {
        let elements: Vec<&AnyParseNode> = group.body.iter().collect();
        return Some(spec(
            kind,
            Bounds::Delimited,
            range.start + 1..range.end - 1,
            Some(node),
            elements,
        ));
    }
    Some(spec(kind, Bounds::Bare, range, Some(node), vec![node]))
}

/// Single-child wrappers that share their child's range: `\colorbox`'s
/// Styling over its group, `\frac ab`'s group over its bare token.
fn unwrap<'n>(src: Source, mut node: &'n AnyParseNode) -> &'n AnyParseNode {
    loop {
        let (AnyParseNode::Styling(_) | AnyParseNode::Mclass(_) | AnyParseNode::OrdGroup(_)) = node
        else {
            return node;
        };
        let range = src.range(node);
        if matches!(node, AnyParseNode::OrdGroup(_))
            && range
                .as_ref()
                .is_some_and(|range| src.wrapped(range, '{', '}'))
        {
            return node;
        }
        match node.children().as_slice() {
            [child] if src.range(child) == range => node = child,
            _ => return node,
        }
    }
}

/// The argument of a command whose node holds its content directly
/// (`\text`, `\operatorname`, `\textcolor`): the invocation's last `{…}`
/// group, holding the topmost nodes inside it, or a bare token. Nodes
/// that map to the whole invocation are a macro's own output and are
/// left out; text in the group that no node covers means the content is
/// not what the nodes say (`\href` without trust draws its source), and
/// the command has no slot.
fn command_argument<'n>(
    src: Source,
    owner: Range<usize>,
    children: &[&'n AnyParseNode],
    kind: SlotKind,
) -> Option<SlotSpec<'n>> {
    if let Some(interior) = src.last_group(owner.clone()) {
        return inside(src, interior, children, kind);
    }
    match children {
        [child] => {
            let range = src.range(child)?;
            (range != owner && within(&range, &owner))
                .then(|| spec(kind, Bounds::Bare, range, None, vec![child]))
        }
        _ => None,
    }
}

/// A slot for `interior`, holding the topmost nodes under `children`
/// that lie inside it.
fn inside<'n>(
    src: Source,
    interior: Range<usize>,
    children: &[&'n AnyParseNode],
    kind: SlotKind,
) -> Option<SlotSpec<'n>> {
    let mut elements = Vec::new();
    let mut stack: Vec<&AnyParseNode> = children.iter().rev().copied().collect();
    while let Some(node) = stack.pop() {
        let Some(range) = src.range(node) else {
            continue;
        };
        if within(&range, &interior) {
            elements.push(node);
        } else {
            stack.extend(node.children().into_iter().rev());
        }
    }
    let mut covered: Vec<Range<usize>> = elements
        .iter()
        .filter_map(|node| src.range(node))
        .map(|range| range.start..src.limits_end(range.end))
        .collect();
    covered.sort_by_key(|range| range.start);
    if src.uncovered(interior.clone(), &covered) {
        return None;
    }
    let container = children.first().copied();
    Some(spec(kind, Bounds::Delimited, interior, container, elements))
}

/// One slot per top-level `{…}` group of the invocation (`\overset{a}{b}`,
/// `\mathchoice`), each holding the nodes inside it.
fn group_arguments<'n>(
    src: Source,
    node: &'n AnyParseNode,
    range: Range<usize>,
    kinds: &[SlotKind],
) -> Option<Vec<SlotSpec<'n>>> {
    let groups = src.groups(range);
    if groups.len() != kinds.len() {
        return None;
    }
    let children = node.children();
    groups
        .into_iter()
        .zip(kinds)
        .map(|(interior, kind)| inside(src, interior, &children, *kind))
        .collect()
}

/// A fraction's numerator and denominator: arguments of `\frac`-like
/// commands, or the two open sides of an infix `\over`.
fn fraction<'n>(
    src: Source,
    frac: &'n ParseNodeGenfrac,
    range: Range<usize>,
) -> Option<Vec<SlotSpec<'n>>> {
    let numer = src.range(&frac.numer)?;
    let denom = src.range(&frac.denom)?;
    let in_order = numer.end <= denom.start;
    let infix = numer.start == range.start && numer != range && in_order;
    if infix {
        let side = |kind, interior: Range<usize>, node: &'n AnyParseNode| {
            spec(kind, Bounds::Open, interior, Some(node), vec![node])
        };
        return Some(vec![
            side(SlotKind::Numer, numer, &frac.numer),
            side(SlotKind::Denom, denom, &frac.denom),
        ]);
    }
    Some(vec![
        argument(src, range.clone(), &frac.numer, SlotKind::Numer)?,
        argument(src, range, &frac.denom, SlotKind::Denom)?,
    ])
}

/// An array's cells, row by row. An array with no cell at all
/// (`\begin{matrix}\end{matrix}`) gets one empty cell before its `\end`.
fn cells<'n>(
    src: Source,
    array: &'n ParseNodeArray,
    range: Range<usize>,
) -> Option<Vec<SlotSpec<'n>>> {
    let mut out = Vec::new();
    for (row, cells) in array.body.iter().enumerate() {
        for (col, cell) in cells.iter().enumerate() {
            let interior = src.range(cell)?;
            out.push(spec(
                SlotKind::Cell { row, col },
                Bounds::Open,
                interior,
                Some(cell),
                vec![cell],
            ));
        }
    }
    if out.is_empty() {
        let end = range.start + src.text(range).rfind(r"\end")?;
        out.push(spec(
            SlotKind::Cell { row: 0, col: 0 },
            Bounds::Open,
            end..end,
            None,
            Vec::new(),
        ));
    }
    Some(out)
}

/// The `\tag{…}` label: a text slot in the invocation's group, holding
/// the argument's nodes (the macro's own parentheses map to the whole
/// invocation and are left out).
pub(super) fn label<'n>(
    src: Source,
    range: Range<usize>,
    nodes: &[&'n AnyParseNode],
) -> Option<SlotSpec<'n>> {
    let interior = src.last_group(range)?;
    let mut spec = inside(src, interior, nodes, SlotKind::Tag)?;
    spec.text = true;
    Some(spec)
}
