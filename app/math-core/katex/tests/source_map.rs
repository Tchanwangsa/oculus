//! Source mapping (`Settings::source_map`) on synthetic formulas: the
//! properties `oracle/render.ts --source-map` checks over the whole corpus,
//! plus the exact ranges of a few structures.
#![allow(clippy::non_ascii_literal, clippy::unwrap_used, clippy::panic)]

use core::fmt::Write as _;

use katex::{
    KatexContext, render_to_string,
    types::{OutputFormat, Settings},
};

fn render(tex: &str, source_map: bool) -> String {
    let settings = Settings::builder()
        .output(OutputFormat::Html)
        .source_map(source_map)
        .build();
    render_to_string(&KatexContext::default(), tex, &settings)
        .unwrap_or_else(|e| panic!("{tex}: {e}"))
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    El(El),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
struct El {
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
}

impl El {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    fn range(&self) -> Option<(usize, usize)> {
        Some((
            self.attr("data-s")?.parse().unwrap(),
            self.attr("data-e")?.parse().unwrap(),
        ))
    }

    fn is_placeholder(&self) -> bool {
        self.attr("class")
            .is_some_and(|c| c.split(' ').any(|c| c == "oc-placeholder"))
    }

    fn text(&self) -> String {
        self.children
            .iter()
            .map(|c| match c {
                Node::Text(t) => t.clone(),
                Node::El(e) => e.text(),
            })
            .collect()
    }
}

/// KaTeX's markup: double-quoted attributes, `/>` for empty elements.
fn parse(html: &str) -> El {
    let mut stack = vec![El {
        attrs: vec![],
        children: vec![],
    }];
    let mut rest = html;
    while !rest.is_empty() {
        if let Some(tag) = rest.strip_prefix('<') {
            let end = tag.find('>').unwrap();
            let inner = &tag[..end];
            rest = &tag[end + 1..];
            if inner.starts_with('/') {
                let done = stack.pop().unwrap();
                stack.last_mut().unwrap().children.push(Node::El(done));
                continue;
            }
            let empty = inner.ends_with('/');
            let inner = inner.trim_end_matches('/');
            let mut attrs = vec![];
            let mut at = inner.find(' ').map_or("", |i| &inner[i..]);
            while let Some(eq) = at.find("=\"") {
                let name = at[..eq].trim().to_owned();
                let value_end = at[eq + 2..].find('"').unwrap();
                attrs.push((name, at[eq + 2..eq + 2 + value_end].to_owned()));
                at = &at[eq + 2 + value_end + 1..];
            }
            let el = El {
                attrs,
                children: vec![],
            };
            if empty {
                stack.last_mut().unwrap().children.push(Node::El(el));
            } else {
                stack.push(el);
            }
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            let text = rest[..end]
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&#x27;", "'")
                .replace("&amp;", "&");
            stack.last_mut().unwrap().children.push(Node::Text(text));
            rest = &rest[end..];
        }
    }
    assert_eq!(stack.len(), 1, "unbalanced markup");
    stack.pop().unwrap()
}

/// A mapped element: its range, its text, and whether it has mapped
/// descendants.
#[derive(Debug)]
struct Mapped {
    range: (usize, usize),
    text: String,
    leaf: bool,
    placeholder: bool,
}

/// Checks ranges (well-formed, nested in the nearest mapped ancestor) and
/// glyphs (each has a mapped ancestor-or-self); returns the mapped elements
/// in document order.
fn mapped(tex: &str, html: &str) -> Vec<Mapped> {
    fn walk(
        tex: &str,
        len: usize,
        n: &El,
        nearest: Option<(usize, usize)>,
        out: &mut Vec<Mapped>,
    ) -> bool {
        let range = n.range();
        let here = range.or(nearest);
        if let Some((s, e)) = range {
            assert!(s <= e && e <= len, "{tex}: range {s}..{e} out of 0..{len}");
            if let Some((ps, pe)) = nearest {
                assert!(
                    ps <= s && e <= pe,
                    "{tex}: {s}..{e} not inside its mapped ancestor {ps}..{pe}"
                );
            }
        }
        let index = out.len();
        if let Some(range) = range {
            out.push(Mapped {
                range,
                text: n.text(),
                leaf: true,
                placeholder: n.is_placeholder(),
            });
        }
        let mut has_mapped = false;
        for c in &n.children {
            match c {
                Node::Text(t) => {
                    if t.chars().any(|c| !c.is_whitespace() && c != '\u{200b}') {
                        assert!(here.is_some(), "{tex}: glyph {t:?} has no mapped ancestor");
                    }
                }
                Node::El(e) => has_mapped |= walk(tex, len, e, here, out),
            }
        }
        if range.is_some() {
            out[index].leaf = !has_mapped;
        }
        has_mapped || range.is_some()
    }
    let root = parse(html);
    let mut out = vec![];
    walk(tex, tex.encode_utf16().count(), &root, None, &mut out);
    out
}

/// Every letter and digit outside a command name and an environment's name
/// lies in a mapped leaf.
fn assert_covered(tex: &str, mapped: &[Mapped]) {
    let units: Vec<u16> = tex.encode_utf16().collect();
    let mut names = vec![false; units.len()];
    for open in ["\\begin{", "\\end{"] {
        for (at, _) in tex.match_indices(open) {
            let close = tex[at..].find('}').map_or(tex.len(), |c| at + c);
            let unit = |byte: usize| tex[..byte].encode_utf16().count();
            names[unit(at)..unit(close)].fill(true);
        }
    }
    let mut in_command = false;
    for (i, &u) in units.iter().enumerate() {
        if names[i] {
            continue;
        }
        let c = char::from_u32(u32::from(u)).unwrap_or('\u{fffd}');
        let previous = i.checked_sub(1).map(|p| units[p]);
        if previous == Some(u16::from(b'\\')) && !in_command {
            in_command = c.is_ascii_alphabetic();
            continue;
        }
        if in_command && c.is_ascii_alphabetic() {
            continue;
        }
        in_command = false;
        if !c.is_alphanumeric() {
            continue;
        }
        assert!(
            mapped
                .iter()
                .any(|m| m.leaf && m.range.0 <= i && i < m.range.1),
            "{tex}: {c:?} at {i} is in no mapped leaf: {mapped:#?}"
        );
    }
}

/// Glyph runs as one span per glyph, the run's italic correction on its last
/// glyph, so a merged run and its split glyphs compare equal when they lay
/// out the same.
fn canonical(n: &El, out: &mut String) {
    for c in &n.children {
        match c {
            Node::Text(t) => out.push_str(t),
            Node::El(e) if e.is_placeholder() => {}
            Node::El(e) => {
                let attrs: Vec<_> = e
                    .attrs
                    .iter()
                    .filter(|(k, _)| k != "data-s" && k != "data-e")
                    .collect();
                if let [Node::Text(t)] = e.children.as_slice()
                    && attrs.is_empty()
                {
                    // A bare glyph: display output writes the text alone.
                    out.push_str(t);
                } else if let [Node::Text(t)] = e.children.as_slice() {
                    let style = e.attr("style").unwrap_or("");
                    let bare: String = style
                        .split(';')
                        .filter(|d| !d.is_empty() && !d.starts_with("margin-right"))
                        .fold(String::new(), |mut bare, d| {
                            write!(bare, "{d};").unwrap();
                            bare
                        });
                    let count = t.chars().count();
                    for (k, ch) in t.chars().enumerate() {
                        out.push_str("<span");
                        for (name, value) in &attrs {
                            if name == "style" {
                                let s = if k + 1 == count { style } else { &bare };
                                if !s.is_empty() {
                                    write!(out, " style=\"{s}\"").unwrap();
                                }
                            } else {
                                write!(out, " {name}=\"{value}\"").unwrap();
                            }
                        }
                        write!(out, ">{ch}</span>").unwrap();
                    }
                } else {
                    out.push_str("<el");
                    for (name, value) in &attrs {
                        write!(out, " {name}=\"{value}\"").unwrap();
                    }
                    out.push('>');
                    canonical(e, out);
                    out.push_str("</el>");
                }
            }
        }
    }
}

/// Flag-off output equals flag-on output without its ranges, glyph runs
/// split aside.
fn assert_identity(tex: &str) {
    let off = render(tex, false);
    assert!(!off.contains("data-s") && !off.contains("oc-placeholder"));
    let (mut a, mut b) = (String::new(), String::new());
    canonical(&parse(&off), &mut a);
    canonical(&parse(&render(tex, true)), &mut b);
    assert_eq!(a, b, "{tex}: flag-on layout differs from flag-off");
}

/// All the properties; the mapped elements for further checks.
fn check(tex: &str) -> Vec<Mapped> {
    let html = render(tex, true);
    let mapped = mapped(tex, &html);
    assert_covered(tex, &mapped);
    if !mapped.iter().any(|m| m.placeholder) {
        assert_identity(tex);
    }
    mapped
}

/// The range of the leaf whose text is `text`.
fn leaf(mapped: &[Mapped], text: &str) -> (usize, usize) {
    mapped
        .iter()
        .find(|m| m.leaf && m.text == text)
        .unwrap_or_else(|| panic!("no leaf {text:?} in {mapped:#?}"))
        .range
}

fn has_range(mapped: &[Mapped], range: (usize, usize)) -> bool {
    mapped.iter().any(|m| m.range == range)
}

#[test]
fn utf16_offsets_in_thai_text() {
    let tex = "\\text{สวัสดี x}";
    let m = check(tex);
    // `x` is byte 25 but UTF-16 unit 13; each Thai mark stays on its base.
    assert_eq!(leaf(&m, "x"), (13, 14));
    assert_eq!(leaf(&m, "ส"), (6, 7));
    assert_eq!(leaf(&m, "วั"), (7, 9));
    assert_eq!(leaf(&m, "ดี"), (10, 12));
    assert!(has_range(&m, (0, 15)));
}

#[test]
fn macro_maps_to_its_invocation() {
    let m = check("\\dots");
    assert!(m.iter().all(|m| m.range == (0, 5)), "{m:#?}");
}

#[test]
fn fraction() {
    let m = check("\\frac{a}{b}");
    assert!(has_range(&m, (0, 11)));
    assert_eq!(leaf(&m, "a"), (6, 7));
    assert_eq!(leaf(&m, "b"), (9, 10));
}

#[test]
fn empty_slots_draw_placeholders() {
    for (tex, slots) in [
        ("\\frac{}{}", vec![6, 8]),
        ("{}", vec![1]),
        ("x^{}", vec![3]),
        ("\\sqrt{}", vec![6]),
        ("\\text{}", vec![6]),
        // Optional arguments: just inside the brackets.
        ("\\sqrt[]{}", vec![6, 8]),
        ("\\sqrt[ ]{x}", vec![6]),
        ("\\xrightarrow[]{}", vec![13, 15]),
        // `\left…\right`'s body: after `\left`'s delimiter.
        ("\\left(\\right)", vec![6]),
        ("\\left\\langle \\right.", vec![13]),
        ("\\left( \\right)", vec![6]),
    ] {
        let m = check(tex);
        let mut got: Vec<usize> = m
            .iter()
            .filter(|m| m.placeholder)
            .map(|m| {
                assert_eq!(
                    m.range.0, m.range.1,
                    "{tex}: a placeholder's range is empty"
                );
                m.range.0
            })
            .collect();
        got.sort_unstable();
        assert_eq!(got, slots, "{tex}");
    }
    // Display output keeps the empty group empty.
    assert!(render("\\frac{}{}", false).contains("<span class=\"mord mtight\"></span>"));
}

#[test]
fn scripts() {
    let m = check("x^2_i");
    assert!(has_range(&m, (0, 5)));
    assert_eq!(leaf(&m, "x"), (0, 1));
    assert_eq!(leaf(&m, "2"), (2, 3));
    assert_eq!(leaf(&m, "i"), (4, 5));
}

#[test]
fn left_right() {
    let m = check("\\left(a\\right)");
    assert!(has_range(&m, (0, 14)));
    assert_eq!(leaf(&m, "a"), (6, 7));
}

#[test]
fn matrix_with_an_empty_cell() {
    let tex = "\\begin{pmatrix}a&b\\\\c&\\end{pmatrix}";
    let m = check(tex);
    assert_eq!(leaf(&m, "a"), (15, 16));
    assert_eq!(leaf(&m, "b"), (17, 18));
    assert_eq!(leaf(&m, "c"), (20, 21));
    let slots: Vec<_> = m
        .iter()
        .filter(|m| m.placeholder)
        .map(|m| m.range)
        .collect();
    assert_eq!(slots, [(22, 22)]);
}

#[test]
fn glyphs_of_different_nodes_stay_apart() {
    let m = check("ab");
    assert_eq!((leaf(&m, "a"), leaf(&m, "b")), ((0, 1), (1, 2)));
    let m = check("\\text{ab}");
    assert!(has_range(&m, (0, 9)));
    assert_eq!((leaf(&m, "a"), leaf(&m, "b")), ((6, 7), (7, 8)));
    let m = check("\\mathbf{ab}");
    assert!(has_range(&m, (0, 11)));
    assert_eq!((leaf(&m, "a"), leaf(&m, "b")), ((8, 9), (9, 10)));
    // Display output still merges them.
    assert!(render("ab", false).contains(">ab</span>"));
}

#[test]
fn split_glyphs_lay_out_as_the_merged_run() {
    // One italic correction (`f`'s is dropped), no line break or glue
    // between `:` and `=`.
    for tex in ["\\mathrm{fi}", "a:=b", "\\text{if}x"] {
        check(tex);
    }
}

#[test]
fn off_by_default() {
    let settings = Settings::default();
    let html = render_to_string(&KatexContext::default(), "\\frac{}{a}", &settings).unwrap();
    assert!(!html.contains("data-s") && !html.contains("oc-placeholder"));
}
