use super::escape::{escape_html, escape_md};
use super::inline::{emphasis, inline_md};
use super::tree::{attr, dropped, tag, text_content, Ref};
use super::{is_tag, Ctx};

pub(super) fn list_md(node: Ref<'_>, depth: usize, ordered: bool, ctx: &Ctx) -> String {
    let mut out = String::new();
    let mut i = 1;
    for li in node.children().filter(|c| is_tag(c, "li")) {
        let marker = if ordered {
            let m = format!("{i}.");
            i += 1;
            m
        } else {
            "-".to_string()
        };
        let indent = "  ".repeat(depth);

        let mut inline_parts = String::new();
        let mut nested = String::new();
        for c in li.children() {
            match tag(&c) {
                Some(t @ ("ul" | "ol")) => nested.push_str(&list_md(c, depth + 1, t == "ol", ctx)),
                Some(_) => inline_parts.push_str(&inline_md_self(c, ctx)),
                None => {
                    if let Some(t) = c.value().as_text() {
                        inline_parts.push_str(&escape_html(t));
                    }
                }
            }
        }
        // Continuation lines (a `<br>`, an embed's block) stay inside the item.
        let hang = format!("\n{indent}{}", " ".repeat(marker.len() + 1));
        let body = inline_parts.trim().replace('\n', &hang);
        out.push_str(&format!("{indent}{marker} {body}\n"));
        out.push_str(&nested);
    }
    out
}

/// `inline_md` renders a node's children; this renders the node itself.
pub(super) fn inline_md_self(n: Ref<'_>, ctx: &Ctx) -> String {
    if dropped(&n, ctx.in_cell) {
        return String::new();
    }
    match tag(&n) {
        Some("a") => format!(
            "[{}]({})",
            inline_md(n, ctx),
            attr(&n, "href").unwrap_or_default()
        ),
        Some("strong" | "b") => emphasis(n, ctx, "**"),
        Some("em" | "i") => emphasis(n, ctx, "*"),
        Some("sup") => escape_md(&text_content(n)),
        Some("code") => format!("`{}`", text_content(n)),
        Some("br") => "  \n".to_string(),
        Some("iframe") => super::embeds::iframe_md(n, ctx).unwrap_or_default(),
        Some("img") => {
            let src = attr(&n, "src").unwrap_or_default();
            let src = ctx.images.get(src).map(String::as_str).unwrap_or(src);
            format!("![{}]({src})", attr(&n, "alt").unwrap_or_default())
        }
        _ => inline_md(n, ctx),
    }
}
