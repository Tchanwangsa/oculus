use super::inline::inline_md;
use super::lists::list_md;
use super::tables::table_md;
use super::tree::{dropped, is_heading, tag, text_content, Ref};
use super::Ctx;

pub(super) fn block_md(node: Ref<'_>, ctx: &Ctx) -> String {
    let mut out = String::new();
    for n in node.children() {
        if let Some(t) = n.value().as_text() {
            let t = t.trim();
            if !t.is_empty() {
                out.push_str(t);
                out.push_str("\n\n");
            }
            continue;
        }
        let Some(name) = tag(&n) else { continue };
        if dropped(&n, ctx.in_cell) {
            continue;
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
            "pre" => {
                let body = text_content(n);
                out.push_str(&format!("```\n{}\n```\n\n", body.trim_end_matches('\n')));
            }
            "blockquote" => {
                let inner = block_md(n, ctx);
                for line in inner.trim().lines() {
                    out.push_str("> ");
                    out.push_str(line);
                    out.push('\n');
                }
                out.push('\n');
            }
            "hr" => out.push_str("---\n\n"),
            "table" => {
                out.push_str(&table_md(n, ctx.images));
                out.push('\n');
            }
            "div" | "section" | "article" => out.push_str(&block_md(n, ctx)),
            _ => {
                let c = inline_md(n, ctx);
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
