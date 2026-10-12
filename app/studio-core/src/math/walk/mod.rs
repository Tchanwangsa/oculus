//! KaTeX's HTML tree -> pieces placed in em. The tree is what katex.css
//! lays out in a browser; this is that layout for the subset KaTeX emits:
//! inline spans side by side on one baseline (margins, padding, explicit
//! widths, relative shifts, `vertical-align`), with vlists (`vlist`),
//! rules (`rule`), SVG (`svg`), laps (`lap`) and glyphs (`symbol`) as their
//! own cases, and `classes` holding what each class does. Lengths are in
//! em of the formula's own size; y grows down from the baseline.
//!
//! Pieces come out in reading order: tree order, except that a vlist's
//! rows, which KaTeX lists bottom to top, are read top to bottom (a
//! numerator before its denominator, a root sign before its radicand).

mod classes;
mod css;
mod lap;
mod rule;
mod svg;
mod symbol;
mod vlist;

use super::fonts::{Family, Font};
use super::{Formula, Shape};
use katex::dom_tree::{DomSpan, HtmlDomNode};
use katex::types::CssProperty;
use vello_cpu::kurbo::{Rect, Vec2};

/// One drawn thing before it has a key.
pub struct Piece<'a> {
    /// What keys it outside a `\k` group: its character, `frac` for a
    /// fraction bar, `sqrt` for a root sign, and so on.
    pub label: String,
    /// The innermost `\k` group it is in.
    pub group: Option<&'a str>,
    pub rect: Rect,
    pub shape: Shape,
    /// The CSS colour KaTeX gave it (`\color`), as written.
    pub colour: Option<&'a str>,
}

/// What an element inherits, and what its parent gives it.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    /// The element's font size, in em of the formula's.
    pub size: f64,
    pub font: Font,
    pub colour: Option<&'a str>,
    /// `text-align`: 0 left, 0.5 centre, 1 right.
    pub align: f64,
    pub group: Option<&'a str>,
    /// The containing block's width, which `width: 100%` fills; `None`
    /// while a vlist is still measuring its rows.
    pub avail: Option<f64>,
    /// The family `.delim-sizeN > span` gives this element.
    pub family: Option<Family>,
}

/// An element laid out: its advance and what it draws, relative to its
/// left edge and baseline.
pub struct Laid<'a> {
    pub width: f64,
    pub pieces: Vec<Piece<'a>>,
    /// It holds something `width: 100%` that `avail` sized (or would have).
    pub fills: bool,
}

impl<'a> Laid<'a> {
    fn empty(width: f64) -> Laid<'a> {
        Laid {
            width,
            pieces: Vec::new(),
            fills: false,
        }
    }

    fn add(&mut self, other: Laid<'a>, dx: f64, dy: f64) {
        self.fills |= other.fills;
        let d = Vec2::new(dx, dy);
        self.pieces.extend(other.pieces.into_iter().map(|mut p| {
            p.rect = p.rect + d;
            p
        }));
    }
}

/// The whole tree: its pieces in reading order and its width, height and
/// depth (KaTeX's own, from the root).
pub fn walk(root: &DomSpan) -> Result<(Formula, Vec<Piece<'_>>), String> {
    let cx = Ctx {
        size: 1.0,
        font: Font::MAIN,
        colour: None,
        align: 0.0,
        group: None,
        avail: None,
        family: None,
    };
    let laid = span(root, cx)?;
    let formula = Formula {
        width: laid.width,
        height: root.height,
        depth: root.depth,
        items: Vec::new(),
    };
    Ok((formula, laid.pieces))
}

fn node<'a>(n: &'a HtmlDomNode, cx: Ctx<'a>) -> Result<Laid<'a>, String> {
    match n {
        HtmlDomNode::DomSpan(s) => span(s, cx),
        HtmlDomNode::Anchor(a) => list(&a.children, cx),
        HtmlDomNode::Fragment(f) => list(&f.children, cx),
        HtmlDomNode::Symbol(s) => symbol::layout(s, cx),
        HtmlDomNode::SvgNode(s) => svg::bare(s, cx),
        HtmlDomNode::Img(_) => Err("\\includegraphics is not supported in Math".to_owned()),
        HtmlDomNode::MathML(_) => Ok(Laid::empty(0.0)),
    }
}

/// Inline children side by side.
fn list<'a>(children: &'a [HtmlDomNode], cx: Ctx<'a>) -> Result<Laid<'a>, String> {
    let mut out = Laid::empty(0.0);
    for c in children {
        let laid = node(c, cx)?;
        let w = laid.width;
        out.add(laid, out.width, 0.0);
        out.width += w;
    }
    Ok(out)
}

/// The context inside `s`: its size, font, alignment, colour and group.
fn inside<'a>(s: &'a DomSpan, cx: Ctx<'a>) -> Ctx<'a> {
    let mut font = classes::font(cx.font, &s.classes);
    if let Some(family) = cx.family {
        font.family = family;
    }
    Ctx {
        size: cx.size * classes::size_factor(&s.classes),
        font,
        colour: s.style.get(CssProperty::Color).or(cx.colour),
        align: classes::align(&s.classes).unwrap_or(cx.align),
        group: s.attributes.get("data-k").map(String::as_str).or(cx.group),
        avail: cx.avail,
        family: classes::child_family(&s.classes),
    }
}

fn span<'a>(s: &'a DomSpan, cx: Ctx<'a>) -> Result<Laid<'a>, String> {
    let c = &s.classes;
    if c.contains("katex-newline") {
        return Err(
            "a line break (\\\\) outside an environment; use \\begin{aligned} … \\end{aligned}"
                .to_owned(),
        );
    }
    if c.contains("katex-strut") || c.contains("vlist-s") {
        return Ok(Laid::empty(0.0));
    }
    let cx = inside(s, cx);
    if c.contains("vlist-t") {
        return vlist::layout(s, cx);
    }
    if c.contains("llap") || c.contains("rlap") || c.contains("clap") {
        return lap::layout(s, cx);
    }
    if let Some(label) = rule::line_label(c) {
        return Ok(rule::line(s, cx, label));
    }
    if c.contains("katex-rule") {
        return Ok(rule::rule(s, cx));
    }
    if svg::is_holder(s) {
        return svg::holder(s, cx);
    }
    boxed(s, cx)
}

/// A plain span: its children inline inside its margins, padding and
/// width, moved by `top`/`left`/`bottom` and `vertical-align`.
fn boxed<'a>(s: &'a DomSpan, mut cx: Ctx<'a>) -> Result<Laid<'a>, String> {
    let st = &s.style;
    let k = classes::class_box(&s.classes);
    let (size, avail) = (cx.size, cx.avail);
    let em = |p| css::get(st, p, size, avail);
    let margin_left = em(CssProperty::MarginLeft).unwrap_or(k.margin.0 * size);
    let margin_right = em(CssProperty::MarginRight).unwrap_or(k.margin.1 * size);
    let pad_left = em(CssProperty::PaddingLeft).unwrap_or(k.padding.0 * size);
    let pad_right = k.padding.1 * size;
    let width = em(CssProperty::Width).or(k.width.map(|w| w * size));
    let min_width = em(CssProperty::MinWidth).unwrap_or(0.0);
    if let Some(w) = width {
        cx.avail = Some(w);
    }
    let inner = list(&s.children, cx)?;
    let content = width.unwrap_or(inner.width).max(min_width);
    let mut out = Laid::empty(margin_left + pad_left + content + pad_right + margin_right);
    // Percentages inside a box with a width of its own resolve against it.
    let fills = inner.fills && width.is_none();
    let dx = margin_left + pad_left + em(CssProperty::Left).unwrap_or(0.0);
    let mut dy = em(CssProperty::Top).unwrap_or(0.0) - em(CssProperty::Bottom).unwrap_or(0.0);
    dy -= em(CssProperty::VerticalAlign).unwrap_or(0.0);
    out.add(inner, dx, dy);
    out.fills = fills;
    Ok(out)
}
