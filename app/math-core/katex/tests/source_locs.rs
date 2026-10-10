//! Node locations with `Settings::source_map` on: every node's range lies in
//! the formula's own input, holds its children's ranges, and every node an
//! edit can land on has one. With the setting off the tree is the same apart
//! from locations, and the locations are the ones display has always had.
#![allow(clippy::non_ascii_literal, clippy::unwrap_used, clippy::panic)]

use std::sync::{Arc, OnceLock};

use katex::{
    KatexContext, Settings, parse,
    parser::parse_node::{AnyParseNode, NodeType, ParseNodeOrdGroup},
    types::{ErrorLocationProvider as _, Mode, SourceLocation},
};

fn ctx() -> &'static KatexContext {
    static CTX: OnceLock<KatexContext> = OnceLock::new();
    CTX.get_or_init(KatexContext::default)
}

fn settings(source_map: bool) -> Settings {
    Settings::builder()
        .display_mode(true)
        .source_map(source_map)
        .build()
}

fn parse_with(expr: &str, source_map: bool) -> Vec<AnyParseNode> {
    parse(ctx(), expr, &settings(source_map))
        .unwrap_or_else(|err| panic!("{expr:?} failed to parse: {err}"))
}

fn mapped(expr: &str) -> Vec<AnyParseNode> {
    parse_with(expr, true)
}

fn text(loc: &SourceLocation) -> &str {
    &loc.input[loc.start..loc.end]
}

fn span(node: &AnyParseNode) -> &str {
    text(node.loc().unwrap_or_else(|| panic!("no loc on {node:?}")))
}

/// Why a node may lack a location: it was made up by the parser, not
/// written, and nothing can be clicked or typed into it.
const fn synthetic(node: &AnyParseNode, parent: Option<&AnyParseNode>) -> Option<&'static str> {
    match (node, parent) {
        // `aligned` puts an empty `{}` at the start of every second cell so a
        // leading relation is binary (amsmath's \start@aligned).
        (AnyParseNode::OrdGroup(group), Some(AnyParseNode::OrdGroup(_)))
            if group.body.is_empty() =>
        {
            Some("aligned's binary-spacing {}")
        }
        _ => None,
    }
}

/// Walks the tree checking every location: in `input`, on char
/// boundaries, inside the parent's, and present unless synthetic.
fn check_tree(expr: &str, nodes: &[AnyParseNode]) {
    let mut first_input: Option<Arc<str>> = None;
    let mut stack: Vec<(&AnyParseNode, Option<&AnyParseNode>)> =
        nodes.iter().map(|node| (node, None)).collect();
    while let Some((node, parent)) = stack.pop() {
        match node.loc() {
            None => {
                assert!(
                    synthetic(node, parent).is_some(),
                    "{expr:?}: {:?} has no loc (parent {:?})",
                    NodeType::from(node),
                    parent.map(NodeType::from),
                );
            }
            Some(loc) => {
                assert_eq!(
                    &*loc.input, expr,
                    "{expr:?}: loc in another input on {node:?}"
                );
                match &first_input {
                    Some(input) => assert!(
                        Arc::ptr_eq(input, &loc.input),
                        "{expr:?}: locs point into different copies of the input"
                    ),
                    None => first_input = Some(Arc::clone(&loc.input)),
                }
                assert!(
                    loc.start <= loc.end && loc.end <= expr.len(),
                    "{expr:?}: {loc:?}"
                );
                assert!(
                    expr.is_char_boundary(loc.start) && expr.is_char_boundary(loc.end),
                    "{expr:?}: {loc:?} splits a character"
                );
                if let Some(outer) = parent.and_then(|parent| parent.loc()) {
                    assert!(
                        outer.start <= loc.start && loc.end <= outer.end,
                        "{expr:?}: {:?} at {:?} lies outside its parent {:?} at {:?}",
                        NodeType::from(node),
                        text(loc),
                        parent.map(NodeType::from),
                        text(outer),
                    );
                }
            }
        }
        stack.extend(node.children().into_iter().map(|child| (child, Some(node))));
    }
}

fn strip_locs(node: &mut AnyParseNode) {
    *node.loc_mut() = None;
    if let AnyParseNode::LeftRight(lr) = node {
        lr.body_loc = None;
    }
    for child in node.children_mut() {
        strip_locs(child);
    }
}

const FORMULAS: &[&str] = &[
    "ab",
    r"\frac{a}{b}",
    r"\frac ab",
    r"a\over b",
    r"{\over b}",
    r"{a\over}",
    r"a\above 2pt b",
    r"{n\choose k}",
    "x^2_i",
    "^2",
    "x'",
    "x''^2",
    "x²",
    "x₁₂",
    r"\sqrt[3]{x}",
    r"\sqrt x",
    r"\left(\frac{a}{b}\right)",
    r"\left(a\middle|b\right)",
    r"\dots",
    r"a\iff b",
    r"\neq",
    r"\alpha x",
    r"\def\x{ab}\x",
    r"\def\x#1{#1+#1}\x{y}",
    r"\newcommand{\br}[1]{\langle #1|}\br{x}",
    r"\bra{x}",
    r"\text{สวัสดี x}",
    r"\text{a $x$ b}",
    r"\begin{pmatrix}a&b\\c&\end{pmatrix}",
    r"\begin{pmatrix}&\\&\end{pmatrix}",
    r"\begin{cases}a&b\\c&d\end{cases}",
    r"\begin{aligned}a&=b\\&=c\end{aligned}",
    r"\begin{array}{cc}a&b\end{array}",
    r"\color{red}{x}",
    r"\color{red} x y",
    r"\textcolor{red}{x}",
    r"\mathbf{ab}",
    r"\bf ab",
    r"\displaystyle x",
    r"\large x",
    r"\operatorname{sin}x",
    r"\overset{a}{b}",
    r"\verb|x|",
    r"\tag{1} x",
    "{}",
    r"\frac{}{}",
    "x^{}",
    r"\hat{x}",
    "é",
    r"\sum_{i=1}^n i",
    r"\mathrm{d}x",
    r"\char`a",
    r"\kern1em x",
    r"\rule{1em}{2em}",
    r"\hbox{x}",
    r"\boxed{x}",
    r"\xrightarrow[b]{a}",
    r"\overbrace{x}^{y}",
    r"\mathchoice{a}{b}{c}{d}",
    r"\phantom{x}",
    r"\not=",
    r"\big(",
    r"\cancel{x}",
    r"a\\b",
    r"\begin{CD}A @>f>> B\\@VVV @AAA\\C @= D\end{CD}",
    r"\begin{CD}A @<a<< B @>>b> C\\@| @AcAA @VVdV\\D @= E @>>> F\end{CD}",
    r"\begin{align}a&=b\tag{1}\\c&=d\end{align}",
    r"\begin{gather}a\\b\end{gather}",
    r"\begin{smallmatrix}a&b\end{smallmatrix}",
    r"\begin{array}{|c|}\hline a\\\hline\end{array}",
    r"\begin{matrix}a\\[1em]b\end{matrix}",
    r"\begin{matrix}\end{matrix}",
    r"\begin{rcases}a\end{rcases}",
    r"\begin{gathered}a\end{gathered}",
    r"\begin{bmatrix*}[r]a\end{bmatrix*}",
    r"\begin{equation}a\end{equation}",
    r"\left.\right.",
    r"\left( \right)",
    r"\sqrt[]{}",
    "a_{}^{}",
    "x^{y^{z}}",
    r"\overline{}",
    r"\text{--- ''}",
    r"\textbf{a}",
    r"\boldsymbol{x}",
    r"\stackrel{a}{=}",
    r"\underbrace{x}_{y}",
    r"\xleftarrow{}",
    r"\pmb{x}",
    r"\raisebox{1em}{x}",
    r"\rlap{x}",
    r"\smash{x}",
    r"\vcenter{x}",
    r"\vphantom{x}",
    r"\mathop{x}",
    r"\binom{a}{b}",
    r"\genfrac(]{0pt}{2}{a}{b}",
    r"\cfrac{a}{b}",
    r"\operatorname*{lim}_x",
    r"\lim\limits_{x}",
    r"\Set{x|y}",
    r"\braket{a|b}",
    r"\colorbox{red}{x}",
    r"\fcolorbox{red}{blue}{x}",
    r"\kern-1em x",
    r"\hspace{1em}x",
    r"\mathring{a}",
    r"\ddots\vdots\cdots\dotsb",
    r"\TextOrMath{a}{b}",
    r"\href{http://a}{x}",
    r"\url{http://a}",
    r"\includegraphics{a.png}",
    r"\htmlClass{a}{x}",
    r"\phase{x}",
    r"\angl{n}",
    r"\sqrt{\smash[b]{y}}",
    r"\begingroup a\endgroup",
    r"\def\x#1.{#1}\x a.",
    r"\let\y=a\y",
    r"\gdef\z{q}\z",
    r"\mathchoice{a}{b}{c}{d}",
];

#[test]
fn every_node_nests_in_its_parent_and_the_input() {
    for expr in FORMULAS {
        check_tree(expr, &mapped(expr));
    }
}

#[test]
fn mapping_changes_only_locations() {
    for expr in FORMULAS {
        let mut on = mapped(expr);
        let mut off = parse_with(expr, false);
        on.iter_mut().for_each(strip_locs);
        off.iter_mut().for_each(strip_locs);
        assert_eq!(on, off, "{expr:?}: the tree changed with source mapping");
    }
}

fn only(expr: &str) -> AnyParseNode {
    let mut nodes = mapped(expr);
    assert_eq!(nodes.len(), 1, "{expr:?}: {nodes:?}");
    nodes.remove(0)
}

fn leaves(node: &AnyParseNode) -> Vec<&AnyParseNode> {
    let children = node.children();
    if children.is_empty() {
        return vec![node];
    }
    children.into_iter().flat_map(leaves).collect()
}

#[test]
fn functions_cover_their_invocation() {
    for expr in [
        r"\frac{a}{b}",
        r"\frac ab",
        r"\sqrt[3]{x}",
        r"\left(\frac{a}{b}\right)",
        r"\left(a\middle|b\right)",
        r"\color{red}{x}",
        r"\color{red} x y",
        r"\textcolor{red}{x}",
        r"\mathbf{ab}",
        r"\bf ab",
        r"\operatorname{sin}",
        r"\overset{a}{b}",
        r"\verb|x|",
        r"\text{a $x$ b}",
        r"\begin{pmatrix}a&b\\c&\end{pmatrix}",
        r"\begin{cases}a&b\\c&d\end{cases}",
        r"\begin{aligned}a&=b\\&=c\end{aligned}",
        r"\hat{x}",
        r"\xrightarrow[b]{a}",
        "x^2_i",
        "^2",
        "x''^2",
        "{}",
    ] {
        assert_eq!(span(&only(expr)), expr, "{expr:?}");
    }

    // `\dfrac` wraps its Genfrac in a Styling; both span the invocation.
    let AnyParseNode::Styling(styling) = only(r"\dfrac{a}{b}") else {
        panic!()
    };
    assert_eq!(span(&styling.body[0]), r"\dfrac{a}{b}");

    // A matrix's delimiters wrap its array; both span the environment.
    let AnyParseNode::LeftRight(matrix) = only(r"\begin{pmatrix}a\end{pmatrix}") else {
        panic!()
    };
    assert_eq!(span(&matrix.body[0]), r"\begin{pmatrix}a\end{pmatrix}");

    let AnyParseNode::Sqrt(sqrt) = only(r"\sqrt[3]{x}") else {
        panic!()
    };
    assert_eq!(span(sqrt.index.as_ref().unwrap()), "[3]");
    assert_eq!(span(&sqrt.body), "{x}");

    let AnyParseNode::Genfrac(frac) = only(r"\frac ab") else {
        panic!()
    };
    assert_eq!((span(&frac.numer), span(&frac.denom)), ("a", "b"));

    let AnyParseNode::LeftRight(lr) = only(r"\left(a\middle|b\right)") else {
        panic!()
    };
    assert_eq!(span(&lr.body[1]), r"\middle|");
    // Its body runs from `\left`'s delimiter to `\right`.
    assert_eq!(lr.body_loc.as_ref().map(text), Some(r"a\middle|b"));
    for (expr, body) in [(r"\left(\right)", ""), (r"\left\langle  x \right.", "x ")] {
        let AnyParseNode::LeftRight(lr) = only(expr) else {
            panic!()
        };
        assert_eq!(lr.body_loc.as_ref().map(text), Some(body), "{expr:?}");
    }

    let tag = only(r"\tag{1} x");
    assert_eq!(span(&tag), r"\tag{1} x");
    let AnyParseNode::Tag(tag) = tag else {
        panic!()
    };
    assert_eq!(span(&tag.body[0]), "x");
    let tag_leaves: Vec<&str> = tag.tag.iter().flat_map(leaves).map(span).collect();
    assert!(tag_leaves.contains(&"1"), "{tag_leaves:?}");
}

#[test]
fn infix_operators_cover_both_sides() {
    let AnyParseNode::Genfrac(frac) = only(r"a\over b") else {
        panic!()
    };
    assert_eq!(frac.loc.as_ref().map(text), Some(r"a\over b"));
    assert_eq!((span(&frac.numer), span(&frac.denom)), ("a", "b"));

    // An empty side is the empty range against the operator.
    let AnyParseNode::OrdGroup(group) = only(r"{\over b}") else {
        panic!()
    };
    let AnyParseNode::Genfrac(frac) = &group.body[0] else {
        panic!()
    };
    let numer = frac.numer.loc().unwrap();
    assert_eq!((numer.start, numer.end), (1, 1));
    assert_eq!(span(&group.body[0]), r"\over b");

    let AnyParseNode::OrdGroup(group) = only(r"{a\over}") else {
        panic!()
    };
    let AnyParseNode::Genfrac(frac) = &group.body[0] else {
        panic!()
    };
    let denom = frac.denom.loc().unwrap();
    assert_eq!((denom.start, denom.end), (7, 7));
}

#[test]
fn scripts() {
    let AnyParseNode::SupSub(supsub) = only("x'") else {
        panic!()
    };
    assert_eq!(span(supsub.sup.as_ref().unwrap()), "'");

    let AnyParseNode::SupSub(supsub) = only("x''^2") else {
        panic!()
    };
    let primes = supsub.sup.as_ref().unwrap();
    assert_eq!(span(primes), "''^2");
    let spans: Vec<&str> = primes.children().into_iter().map(span).collect();
    assert_eq!(spans, ["'", "'", "2"]);

    let node = only("x²");
    assert_eq!(span(&node), "x²");
    let AnyParseNode::SupSub(supsub) = node else {
        panic!()
    };
    let sup = supsub.sup.as_ref().unwrap();
    assert_eq!(span(sup), "²");
    assert_eq!(span(sup.children()[0]), "²");

    let AnyParseNode::SupSub(supsub) = only("x₁₂") else {
        panic!()
    };
    assert_eq!(span(supsub.sub.as_ref().unwrap()), "₁₂");

    let AnyParseNode::SupSub(supsub) = only("x^{}") else {
        panic!()
    };
    assert_eq!(span(supsub.sup.as_ref().unwrap()), "{}");
}

#[test]
fn macro_bodies_map_to_their_invocation() {
    // Every glyph \dots draws comes from its body, so maps to `\dots`.
    for expr in [r"\dots", r"\neq", r"\iff"] {
        for leaf in leaves(&AnyParseNode::OrdGroup(ParseNodeOrdGroup {
            mode: Mode::Math,
            loc: None,
            body: mapped(expr),
            semisimple: None,
        })) {
            assert_eq!(leaf.loc().map(text), Some(expr), "{expr:?}: {leaf:?}");
        }
    }

    let nodes = mapped(r"a\iff b");
    let spans: Vec<&str> = nodes.iter().map(span).collect();
    assert_eq!(spans.first(), Some(&"a"));
    assert_eq!(spans.last(), Some(&"b"));
    assert!(
        spans[1..spans.len() - 1].iter().all(|s| *s == r"\iff "),
        "{spans:?}"
    );

    let nodes = mapped(r"\def\x{ab}\x");
    let locs: Vec<(usize, usize)> = nodes
        .iter()
        .map(|node| node.loc().map(|loc| (loc.start, loc.end)).unwrap())
        .collect();
    assert_eq!(locs, [(10, 12), (10, 12)]);

    // A pasted argument keeps its own location, inside the invocation's.
    let expr = r"\newcommand{\br}[1]{\langle #1|}\br{x}";
    let invocation = &expr[expr.rfind(r"\br").unwrap()..];
    let nodes = mapped(expr);
    let spans: Vec<&str> = nodes.iter().flat_map(leaves).map(span).collect();
    assert_eq!(spans, [invocation, "x", invocation]);

    let expr = r"\def\x#1{#1+#1}\x{y}";
    let nodes = mapped(expr);
    let spans: Vec<&str> = nodes.iter().map(span).collect();
    assert_eq!(spans, ["y", r"\x{y}", "y"]);

    // The built-in \bra's body maps to the invocation; its argument to itself.
    let nodes = mapped(r"\bra{x}");
    let spans: Vec<&str> = nodes.iter().flat_map(leaves).map(span).collect();
    assert!(
        spans.contains(&"x") && spans.contains(&r"\bra{x}"),
        "{spans:?}"
    );
}

#[test]
fn control_words_keep_their_trailing_space() {
    // A control word's location runs through the spaces that end it, as in
    // KaTeX: inserting at a node's end then never extends the word
    // (`\alpha x` → `\alpha yx`, not `\alphay x`), deleting the node takes
    // its separator, and error messages read the same with mapping on.
    for source_map in [true, false] {
        let nodes = parse_with(r"\alpha x", source_map);
        assert_eq!(nodes[0].loc().map(text), Some(r"\alpha "));
    }
    let nodes = mapped(r"\frac\alpha  b");
    assert_eq!(span(&nodes[0]), r"\frac\alpha  b");
}

#[test]
fn text_offsets_land_on_char_boundaries() {
    let expr = r"\text{สวัสดี x}";
    let node = only(expr);
    assert_eq!(span(&node), expr);
    let spans: Vec<&str> = leaves(&node).into_iter().map(span).collect();
    assert_eq!(spans.concat(), "สวัสดี x");
}

fn cells(node: &AnyParseNode) -> Vec<Vec<&AnyParseNode>> {
    let array = match node {
        AnyParseNode::Array(array) => array,
        AnyParseNode::LeftRight(lr) => match &lr.body[0] {
            AnyParseNode::Array(array) => array,
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    array.body.iter().map(|row| row.iter().collect()).collect()
}

#[test]
fn array_cells() {
    let expr = r"\begin{pmatrix}a&b\\c&\end{pmatrix}";
    let node = only(expr);
    let rows = cells(&node);
    let spans: Vec<Vec<&str>> = rows
        .iter()
        .map(|row| row.iter().map(|c| span(c)).collect())
        .collect();
    assert_eq!(spans, [vec!["a", "b"], vec!["c", ""]]);
    // The empty cell sits just before `\end`.
    let empty = rows[1][1].loc().unwrap();
    assert_eq!(empty.start, expr.find(r"\end").unwrap());
    // Its inner group shares the range.
    assert_eq!(rows[1][1].children()[0].loc(), Some(empty));

    let expr = r"\begin{pmatrix}&\\&\end{pmatrix}";
    let node = only(expr);
    let starts: Vec<Vec<usize>> = cells(&node)
        .iter()
        .map(|row| row.iter().map(|c| c.loc().unwrap().start).collect())
        .collect();
    assert_eq!(starts, [vec![15, 16], vec![18, 19]]);

    let node = only(r"\begin{aligned}a&=b\\&=c\end{aligned}");
    let spans: Vec<Vec<&str>> = cells(&node)
        .iter()
        .map(|row| row.iter().map(|c| span(c)).collect())
        .collect();
    assert_eq!(spans, [vec!["a", "=b"], vec!["", "=c"]]);
}

#[test]
fn display_locations_are_unchanged() {
    // With mapping off, nodes the mapping fills keep no location, functions
    // keep their control word's, and macro bodies point into the body.
    let AnyParseNode::SupSub(supsub) = &parse_with("x^2", false)[0] else {
        panic!()
    };
    assert_eq!(supsub.loc, None);

    let frac = &parse_with(r"\frac{a}{b}", false)[0];
    assert_eq!(frac.loc().map(text), Some(r"\frac"));

    let dots = parse_with(r"\dots", false);
    assert!(
        dots.iter()
            .any(|node| node.loc().is_some_and(|loc| &*loc.input != r"\dots"))
    );

    let node = &parse_with(r"\begin{pmatrix}a\end{pmatrix}", false)[0];
    assert_eq!(node.loc(), None);
    for row in cells(node) {
        for cell in row {
            assert_eq!(cell.loc(), None);
        }
    }

    let AnyParseNode::Tag(tag) = &parse_with(r"\tag{1} x", false)[0] else {
        panic!()
    };
    assert_eq!(tag.loc, None);
}
