use super::embeds::{details_md, iframe_md};
use super::escape::escape_md;
use super::links::normalize_href;
use super::maths::{equation_latex, inline_math, is_equation_image, text_md};
use super::tree::{attr, dropped, tag, text_content, Ref};
use super::widgets::toggler_md;
use super::Ctx;

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

fn emphasis(n: Ref<'_>, ctx: &Ctx, marks: &str) -> String {
    let inner = inline_md(n, ctx).trim().to_string();
    if inner.is_empty() {
        String::new()
    } else if is_marker_only(&inner) {
        inner
    } else {
        format!("{marks}{inner}{marks}")
    }
}

/// A node's children as inline markdown.
pub(super) fn inline_md(node: Ref<'_>, ctx: &Ctx) -> String {
    let mut out = String::new();
    for n in node.children() {
        match n.value().as_text() {
            Some(t) => out.push_str(&text_md(t, ctx, escape_md)),
            None => out.push_str(&inline_md_self(n, ctx)),
        }
    }
    out
}

/// One element as inline markdown. Blocks met here (an embed, a `<details>`)
/// put blank lines either side, splitting the paragraph they sit in.
pub(super) fn inline_md_self(n: Ref<'_>, ctx: &Ctx) -> String {
    let Some(name) = tag(&n) else {
        return String::new();
    };
    if dropped(&n, ctx) {
        return String::new();
    }
    if let Some(md) = toggler_md(n, ctx) {
        return md;
    }
    match name {
        "a" => link_md(n, ctx),
        "strong" | "b" => emphasis(n, ctx, "**"),
        "em" | "i" => emphasis(n, ctx, "*"),
        // Footnote markers: keep the character, drop the superscript.
        "sup" => escape_md(&text_content(n)),
        "code" => format!("`{}`", text_content(n)),
        "br" => "  \n".to_string(),
        "iframe" => iframe_md(n, ctx).unwrap_or_default(),
        "details" if !ctx.in_cell => format!("\n\n{}", details_md(n, ctx)),
        "img" => img_md(n, ctx),
        _ => inline_md(n, ctx),
    }
}

fn link_md(n: Ref<'_>, ctx: &Ctx) -> String {
    let text = inline_md(n, ctx);
    let href = attr(&n, "href").and_then(|h| normalize_href(h, attr(&n, "data-api-endpoint")));
    match href {
        Some(href) => format!("[{text}]({href})"),
        None => text,
    }
}

fn img_md(n: Ref<'_>, ctx: &Ctx) -> String {
    if n.value().as_element().is_some_and(is_equation_image) {
        return equation_latex(&n)
            .map(|l| inline_math(&l))
            .unwrap_or_default();
    }
    let src = attr(&n, "src").unwrap_or_default();
    let src = ctx.walk.images.get(src).map(String::as_str).unwrap_or(src);
    format!("![{}]({src})", attr(&n, "alt").unwrap_or_default())
}
