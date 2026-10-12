use super::embeds::{details_md, iframe_md};
use super::escape::escape_loose;
use super::inline::{inline_md, inline_md_self};
use super::lists::list_md;
use super::maths::{display_math, sole_equation, text_md};
use super::tables::table_md;
use super::tree::{dropped, is_heading, tag, Ref};
use super::widgets::widget_md;
use super::Ctx;

pub(super) fn block_md(node: Ref<'_>, ctx: &Ctx) -> String {
    block_md_where(node, ctx, |_| true)
}

/// `block_md` over the children `keep` accepts.
pub(super) fn block_md_where(node: Ref<'_>, ctx: &Ctx, keep: impl Fn(&Ref<'_>) -> bool) -> String {
    let mut out = String::new();
    for n in node.children().filter(|c| keep(c)) {
        if let Some(t) = n.value().as_text() {
            let t = t.trim();
            if !t.is_empty() {
                out.push_str(&text_md(t, ctx, escape_loose));
                out.push_str("\n\n");
            }
            continue;
        }
        let Some(name) = tag(&n) else { continue };
        if dropped(&n, ctx) {
            continue;
        }
        if let Some(md) = widget_md(n, ctx) {
            out.push_str(&md);
            continue;
        }
        if matches!(name, "p" | "div") {
            if let Some(latex) = sole_equation(n) {
                out.push_str(&display_math(&latex));
                out.push_str("\n\n");
                continue;
            }
        }

        if is_heading(name) {
            let level = name[1..].parse::<usize>().unwrap_or(1);
            out.push_str(&format!(
                "{} {}\n\n",
                "#".repeat(level),
                inline_md(n, ctx).trim()
            ));
            continue;
        }
        match name {
            "p" => {
                let c = inline_md(n, ctx);
                let c = c.trim();
                if !c.is_empty() {
                    out.push_str(c);
                    out.push_str("\n\n");
                }
            }
            "ul" | "ol" => {
                out.push_str(&list_md(n, 0, name == "ol", ctx));
                out.push('\n');
            }
            "pre" => out.push_str(&fence(&code_text(n))),
            "blockquote" => out.push_str(&quote(&block_md(n, ctx))),
            "hr" => out.push_str("---\n\n"),
            "table" => {
                out.push_str(&table_md(n, ctx));
                out.push('\n');
            }
            "div" | "section" | "article" => out.push_str(&block_md(n, ctx)),
            "details" => out.push_str(&details_md(n, ctx)),
            "iframe" => {
                if let Some(md) = iframe_md(n, ctx) {
                    out.push_str(md.trim_matches('\n'));
                    out.push_str("\n\n");
                }
            }
            _ => {
                let c = inline_md_self(n, ctx);
                let c = c.trim();
                if !c.is_empty() {
                    out.push_str(c);
                    out.push_str("\n\n");
                }
            }
        }
    }
    out
}

/// Markdown as a blockquote; a blank line inside stays in the quote.
pub(super) fn quote(md: &str) -> String {
    let mut out = String::new();
    for line in md.trim().lines() {
        if line.trim().is_empty() {
            out.push_str(">\n");
        } else {
            out.push_str("> ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push('\n');
    out
}

/// Code as written: text verbatim, a `<br>` or the end of a block element as
/// a newline.
pub(super) fn code_text(n: Ref<'_>) -> String {
    fn walk(n: Ref<'_>, out: &mut String) {
        for c in n.children() {
            if let Some(t) = c.value().as_text() {
                out.push_str(t);
                continue;
            }
            match tag(&c) {
                Some("br") => out.push('\n'),
                Some("div" | "p" | "li") => {
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                    walk(c, out);
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                Some(_) => walk(c, out),
                None => {}
            }
        }
    }
    let mut out = String::new();
    walk(n, &mut out);
    out
}

pub(super) fn fence(code: &str) -> String {
    format!("```\n{}\n```\n\n", code.trim_end_matches('\n'))
}

pub fn collapse_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut newlines = 0;
    for c in s.chars() {
        if c == '\n' {
            newlines += 1;
            if newlines > 2 {
                continue;
            }
        } else {
            newlines = 0;
        }
        out.push(c);
    }
    out
}
