//! Every text node goes through here. The renderer parses raw HTML in a page
//! (`rehype-raw`), so prose like "your <head> section" must reach it as
//! `&lt;head>`, never as a tag. Code spans and fences stay literal: they take
//! `text_content` inside backticks, which markdown never parses as HTML.

/// `&` and `<` as entities, so the text renders as itself.
fn push_html(c: char, out: &mut String) -> bool {
    match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        _ => return false,
    }
    true
}

/// Text placed where markdown is not parsed further (a block-level text node,
/// a list item's or a cell's loose text, a `<summary>`).
pub(super) fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if !push_html(c, &mut out) {
            out.push(c);
        }
    }
    out
}

/// Inline text: `escape_html` plus the markdown punctuation that would
/// otherwise start emphasis, code or a link.
pub(super) fn escape_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if push_html(c, &mut out) {
            continue;
        }
        if matches!(c, '*' | '_' | '`' | '[' | ']' | '\\') {
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
