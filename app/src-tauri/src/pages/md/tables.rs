use super::inline::inline_md;
use super::lists::inline_md_self;
use super::tree::{dropped, element_children, is_heading, tag, Ref};
use super::{is_tag, Ctx, ImageMap, NBSP};

const CELL_BLOCKS: &[&str] = &["p", "div", "blockquote", "section", "article", "li"];

fn is_cell_block(name: &str) -> bool {
    CELL_BLOCKS.contains(&name) || is_heading(name)
}

/// A GFM cell must be a single line. Block children become segments joined by
/// " · "; lists collapse to "; "-joined items.
fn cell_md(cell: Ref<'_>, images: &ImageMap) -> String {
    let ctx = Ctx {
        images,
        in_cell: true,
    };
    let mut segs: Vec<String> = Vec::new();
    let mut cur = String::new();

    fn flush(cur: &mut String, segs: &mut Vec<String>) {
        let t = squeeze(cur);
        if !t.is_empty() {
            segs.push(t);
        }
        cur.clear();
    }

    for n in cell.children() {
        if let Some(t) = n.value().as_text() {
            cur.push_str(t);
            continue;
        }
        let Some(name) = tag(&n) else { continue };
        if dropped(&n, true) {
            continue;
        }
        match name {
            "br" => cur.push(' '),
            "ul" | "ol" => {
                let items: Vec<String> = n
                    .descendants()
                    .filter(|d| is_tag(d, "li"))
                    .map(|li| squeeze(&inline_md(li, &ctx)))
                    .filter(|s| !s.is_empty())
                    .collect();
                cur.push_str(&items.join("; "));
            }
            _ if is_cell_block(name) => {
                flush(&mut cur, &mut segs);
                let inner = squeeze(&inline_md(n, &ctx));
                if !inner.is_empty() {
                    segs.push(inner);
                }
            }
            _ => cur.push_str(&inline_md_self(n, &ctx)),
        }
    }
    flush(&mut cur, &mut segs);

    segs.join(" · ")
        .replace(NBSP, " ")
        .replace('|', "\\|")
        .trim()
        .to_string()
}

fn squeeze(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn table_md(node: Ref<'_>, images: &ImageMap) -> String {
    let rows: Vec<Ref<'_>> = node.descendants().filter(|d| is_tag(d, "tr")).collect();
    let Some(first) = rows.first() else {
        return String::new();
    };

    let head: Vec<String> = element_children(*first)
        .map(|c| cell_md(c, images))
        .collect();
    let cols = head.len();
    if cols == 0 {
        return String::new();
    }
    let pad = |mut v: Vec<String>| {
        v.resize(cols, String::new());
        v
    };

    let mut out = format!("| {} |\n", pad(head.clone()).join(" | "));
    out.push_str(&format!("| {} |\n", vec!["---"; cols].join(" | ")));
    for r in &rows[1..] {
        let cells: Vec<String> = element_children(*r).map(|c| cell_md(c, images)).collect();
        out.push_str(&format!("| {} |\n", pad(cells).join(" | ")));
    }
    out
}
