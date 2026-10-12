//! Every text node goes through here. The renderer parses raw HTML in a page
//! (`rehype-raw`), so prose like "your <head> section" must reach it as
//! `&lt;head>`, never as a tag. Code spans and fences stay literal: they take
//! `text_content` inside backticks, which markdown never parses as HTML.
//!
//! Every `$` in markdown text is escaped, so remark-math never pairs currency
//! ("$10 … $15") as a formula; maths reaches the page only from `maths.rs`.

/// `&` and `<` as entities, so the text renders as itself.
fn push_html(c: char, out: &mut String) -> bool {
    match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        _ => return false,
    }
    true
}

/// Text inside an HTML block, which markdown does not parse (a `<summary>`).
pub(super) fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if !push_html(c, &mut out) {
            out.push(c);
        }
    }
    out
}

/// Loose text: a block-level text node, a list item's or a cell's own text.
pub(super) fn escape_loose(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if push_html(c, &mut out) {
            continue;
        }
        if c == '$' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Inline text: `escape_loose` plus the markdown punctuation that would
/// otherwise start emphasis, code or a link.
pub(super) fn escape_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if push_html(c, &mut out) {
            continue;
        }
        if matches!(c, '*' | '_' | '`' | '[' | ']' | '\\' | '$') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A double-quoted attribute value.
pub(super) fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;")
}
