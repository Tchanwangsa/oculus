use super::tree::{attr, dropped, tag, text_content, Ref};
use super::Ctx;

pub(super) fn escape_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '*' | '_' | '`' | '[' | ']' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Emphasis wrapping content that is only footnote markers or digits produces
/// ambiguous Markdown (`**\*†**`), so that content is emitted bare.
fn is_marker_only(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| {
            matches!(c, '\\' | '*' | '†' | '‡' | '§' | '¶')
                || c.is_ascii_digit()
                || c.is_whitespace()
        })
}

pub(super) fn emphasis(n: Ref<'_>, ctx: &Ctx, marks: &str) -> String {
    let inner = inline_md(n, ctx).trim().to_string();
    if inner.is_empty() {
        String::new()
    } else if is_marker_only(&inner) {
        inner
    } else {
        format!("{marks}{inner}{marks}")
    }
}

pub(super) fn inline_md(node: Ref<'_>, ctx: &Ctx) -> String {
    let mut out = String::new();
    for n in node.children() {
        if let Some(t) = n.value().as_text() {
            out.push_str(&escape_md(t));
            continue;
        }
        let Some(name) = tag(&n) else { continue };
        if dropped(&n, ctx.in_cell) {
            continue;
        }
        match name {
            "a" => out.push_str(&format!(
                "[{}]({})",
                inline_md(n, ctx),
                attr(&n, "href").unwrap_or_default()
            )),
            "strong" | "b" => out.push_str(&emphasis(n, ctx, "**")),
            "em" | "i" => out.push_str(&emphasis(n, ctx, "*")),
            // Footnote markers: keep the character, drop the superscript.
            "sup" => out.push_str(&escape_md(&text_content(n))),
            "code" => out.push_str(&format!("`{}`", text_content(n))),
            "br" => out.push_str("  \n"),
            "img" => {
                let src = attr(&n, "src").unwrap_or_default();
                let src = ctx.images.get(src).map(String::as_str).unwrap_or(src);
                out.push_str(&format!(
                    "![{}]({src})",
                    attr(&n, "alt").unwrap_or_default()
                ));
            }
            _ => out.push_str(&inline_md(n, ctx)),
        }
    }
    out
}
