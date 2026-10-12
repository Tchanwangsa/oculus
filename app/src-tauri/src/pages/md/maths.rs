//! Maths becomes remark-math's `$…$` and `$$` blocks. Two sources only: Canvas
//! equation images (their LaTeX is in the tag; nothing is fetched) and MathJax
//! `\(…\)` / `\[…\]` in text. A `$` in text is never maths (`escape.rs`).

use scraper::node::Element;

use super::tree::{Ctx, Ref};

/// A Canvas equation-editor image (`img.equation_image`, src under
/// `/equation_images/`).
pub(super) fn is_equation_image(el: &Element) -> bool {
    el.name() == "img"
        && (el.classes().any(|c| c == "equation_image")
            || el
                .attr("src")
                .is_some_and(|s| s.contains("/equation_images/")))
}

/// An equation image's LaTeX: `data-equation-content`, else `title`, else the
/// src path (encoded twice), else `alt` minus `LaTeX: `.
pub(super) fn equation_latex(n: &Ref<'_>) -> Option<String> {
    let el = n.value().as_element()?;
    let nonblank = |s: &&str| !s.trim().is_empty();
    let latex = el
        .attr("data-equation-content")
        .filter(nonblank)
        .map(str::to_string)
        .or_else(|| el.attr("title").filter(nonblank).map(str::to_string))
        .or_else(|| {
            let src = el.attr("src")?;
            let enc = src.split_once("/equation_images/")?.1;
            let enc = enc.split(['?', '#']).next()?;
            Some(percent_decode(&percent_decode(enc))).filter(|s| nonblank(&s.as_str()))
        })
        .or_else(|| {
            let alt = el.attr("alt")?.trim();
            Some(alt.strip_prefix("LaTeX:").unwrap_or(alt).to_string())
        })?;
    let latex = latex.trim();
    (!latex.is_empty()).then(|| latex.to_string())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = b.get(i + 1..i + 3).and_then(|h| {
            std::str::from_utf8(h)
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        });
        match (b[i], hex) {
            (b'%', Some(v)) => {
                out.push(v);
                i += 3;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `$latex$` on one line, or nothing when the source is blank.
pub(super) fn inline_math(latex: &str) -> String {
    let latex = latex.replace(['\r', '\n'], " ");
    let latex = latex.trim();
    if latex.is_empty() {
        String::new()
    } else {
        format!("${latex}$")
    }
}

/// A `$$` block, without its surrounding blank lines; blank lines inside are
/// dropped so a list item's indent can't end it.
pub(super) fn display_math(latex: &str) -> String {
    let body: Vec<&str> = latex
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if body.is_empty() {
        String::new()
    } else {
        format!("$$\n{}\n$$", body.join("\n"))
    }
}

/// The LaTeX of a block whose only content is one equation image, which then
/// renders as display maths.
pub(super) fn sole_equation(n: Ref<'_>) -> Option<String> {
    let mut latex = None;
    for d in n.descendants().skip(1) {
        if let Some(t) = d.value().as_text() {
            if !t.trim().is_empty() {
                return None;
            }
            continue;
        }
        let Some(el) = d.value().as_element() else {
            continue;
        };
        if is_equation_image(el) {
            if latex.is_some() {
                return None;
            }
            latex = Some(equation_latex(&d)?);
        } else if matches!(
            el.name(),
            "img" | "iframe" | "video" | "audio" | "object" | "embed"
        ) {
            return None;
        }
    }
    latex
}

enum Seg<'s> {
    Prose(&'s str),
    Inline(&'s str),
    Display(&'s str),
}

/// Splits text at MathJax delimiters. An opener without its closer, or with
/// only whitespace before it, stays prose.
fn split_math(s: &str) -> Vec<Seg<'_>> {
    let mut out = Vec::new();
    let (mut start, mut i) = (0, 0);
    while let Some(off) = s[i..].find('\\') {
        let at = i + off;
        let (close, display) = match s.as_bytes().get(at + 1) {
            Some(b'(') => ("\\)", false),
            Some(b'[') => ("\\]", true),
            _ => {
                i = at + 1;
                continue;
            }
        };
        let body = at + 2;
        match s[body..].find(close) {
            Some(len) if !s[body..body + len].trim().is_empty() => {
                if start < at {
                    out.push(Seg::Prose(&s[start..at]));
                }
                let inner = &s[body..body + len];
                out.push(if display {
                    Seg::Display(inner)
                } else {
                    Seg::Inline(inner)
                });
                i = body + len + 2;
                start = i;
            }
            _ => i = body,
        }
    }
    if start < s.len() {
        out.push(Seg::Prose(&s[start..]));
    }
    out
}

/// A text node as markdown: maths delimiters become dollars and the prose
/// around them goes through `prose`. Display maths splits the paragraph it
/// sits in, except in a table cell, where it stays inline.
pub(super) fn text_md(s: &str, ctx: &Ctx, prose: fn(&str) -> String) -> String {
    let mut out = String::new();
    for seg in split_math(s) {
        match seg {
            Seg::Prose(p) => out.push_str(&prose(p)),
            Seg::Inline(m) => out.push_str(&inline_math(m)),
            Seg::Display(m) if ctx.in_cell => out.push_str(&inline_math(m)),
            Seg::Display(m) => {
                let d = display_math(m);
                if !d.is_empty() {
                    out.push_str(&format!("\n\n{d}\n\n"));
                }
            }
        }
    }
    out
}
