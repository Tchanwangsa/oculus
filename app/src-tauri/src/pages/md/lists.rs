use super::escape::escape_loose;
use super::inline::inline_md_self;
use super::maths::text_md;
use super::tree::{tag, Ref};
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
                        inline_parts.push_str(&text_md(t, ctx, escape_loose));
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
