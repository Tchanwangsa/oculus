//! HTML → Markdown for Canvas bodies.
//!
//! A direct port of the DOM converter that used to run inside the worker
//! WebView, minus the DOM mutation: instead of stripping cruft nodes up front
//! and re-reading the tree, the walk simply refuses to descend into them.
//! Same output, no mutable tree needed.

use std::collections::HashMap;

use ego_tree::NodeRef;
use scraper::node::Node;
use scraper::Html;

const NBSP: char = '\u{a0}';

/// Never rendered, in any position.
const SKIP_TAGS: &[&str] = &["script", "style", "noscript", "svg", "path"];

/// Rewrites for `<img src>` — original src → local relative path, filled in by
/// the image downloader before conversion.
pub type ImageMap = HashMap<String, String>;

/// Convert a Canvas HTML body to Markdown. `images` may be empty.
pub fn to_markdown(html: &str, images: &ImageMap) -> String {
    let doc = Html::parse_document(html);
    let body = doc
        .tree
        .root()
        .descendants()
        .find(|n| is_tag(n, "body"))
        .unwrap_or_else(|| doc.tree.root());

    let ctx = Ctx { images, in_cell: false };
    let out = block_md(body, &ctx);
    collapse_blank_lines(&out).trim().to_string()
}

/// Every `<img>` in a body, as (data-api-endpoint or /api/v1/files/{data-id},
/// src). The endpoint is what the file API is asked about; the src is the key
/// the rewrite map is looked up by during conversion.
pub fn image_refs(html: &str) -> Vec<(String, String)> {
    let doc = Html::parse_document(html);
    doc.tree
        .root()
        .descendants()
        .filter(|n| is_tag(n, "img"))
        .filter_map(|n| {
            let el = n.value().as_element()?;
            let endpoint = match el.attr("data-api-endpoint") {
                Some(e) => e.to_string(),
                // The <img> carries the file id even when the endpoint
                // attribute is absent.
                None => format!("/api/v1/files/{}", el.attr("data-id")?),
            };
            Some((endpoint, el.attr("src").unwrap_or_default().to_string()))
        })
        .collect()
}

/// Canvas page slugs and course file ids linked from a body. Pages link to
/// pages that no module lists, so one level of this is what keeps content from
/// being silently missed.
pub fn canvas_links(html: &str, course_id: i64) -> (Vec<String>, Vec<String>) {
    let doc = Html::parse_document(html);
    let prefix = format!("/courses/{course_id}/");
    let (mut pages, mut files) = (Vec::new(), Vec::new());

    for n in doc.tree.root().descendants() {
        if !is_tag(&n, "a") {
            continue;
        }
        let Some(href) = n.value().as_element().and_then(|e| e.attr("href")) else { continue };

        if let Some(slug) = after_marker(href, "/pages/") {
            let slug = slug.split(['?', '#', '/']).next().unwrap_or("");
            if !slug.is_empty() && href.contains("/courses/") && !pages.iter().any(|p| p == slug) {
                pages.push(slug.to_string());
            }
        }
        if href.contains(&prefix) {
            if let Some(rest) = after_marker(href, "/files/") {
                let id: String = rest.chars().take_while(char::is_ascii_digit).collect();
                if !id.is_empty() && !files.iter().any(|f| f == &id) {
                    files.push(id);
                }
            }
        }
    }
    (pages, files)
}

fn after_marker<'a>(s: &'a str, marker: &str) -> Option<&'a str> {
    s.find(marker).map(|i| &s[i + marker.len()..])
}

// ── Tree helpers ─────────────────────────────────────────────────────────────

type Ref<'a> = NodeRef<'a, Node>;

struct Ctx<'a> {
    images: &'a ImageMap,
    /// Inside a table cell: images and block structure are dropped, because a
    /// GFM cell must stay on one line.
    in_cell: bool,
}

fn is_tag(n: &Ref<'_>, name: &str) -> bool {
    n.value().as_element().is_some_and(|e| e.name() == name)
}

fn tag<'a>(n: &Ref<'a>) -> Option<&'a str> {
    n.value().as_element().map(|e| e.name())
}

fn attr<'a>(n: &Ref<'a>, name: &str) -> Option<&'a str> {
    n.value().as_element().and_then(|e| e.attr(name))
}

fn is_heading(name: &str) -> bool {
    matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
}

/// Element children only — the equivalent of `.children`, not `.childNodes`.
fn element_children<'a>(n: Ref<'a>) -> impl Iterator<Item = Ref<'a>> {
    n.children().filter(|c| c.value().is_element())
}

fn text_content(n: Ref<'_>) -> String {
    n.descendants()
        .filter_map(|d| d.value().as_text().map(|t| t.to_string()))
        .collect()
}

/// Decorative and accessibility cruft that must not reach the output. In a
/// table cell the net is wider: an image or an icon there breaks the row.
fn dropped(n: &Ref<'_>, in_cell: bool) -> bool {
    let Some(el) = n.value().as_element() else { return false };
    let name = el.name();

    if SKIP_TAGS.contains(&name) {
        return true;
    }
    let classes: Vec<&str> = el.classes().collect();
    if classes.iter().any(|c| {
        *c == "screenreader-only" || *c == "external_link_icon" || c.contains("screenReaderContent")
    }) {
        return true;
    }
    if name == "button" && classes.contains(&"ally-accessible-versions") {
        return true;
    }
    if name == "img" && (el.attr("role") == Some("presentation") || in_cell) {
        return true;
    }
    false
}

// ── Inline ───────────────────────────────────────────────────────────────────

fn escape_md(s: &str) -> String {
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
            matches!(c, '\\' | '*' | '†' | '‡' | '§' | '¶') || c.is_ascii_digit() || c.is_whitespace()
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

fn inline_md(node: Ref<'_>, ctx: &Ctx) -> String {
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
                out.push_str(&format!("![{}]({src})", attr(&n, "alt").unwrap_or_default()));
            }
            _ => out.push_str(&inline_md(n, ctx)),
        }
    }
    out
}

// ── Lists ────────────────────────────────────────────────────────────────────

fn list_md(node: Ref<'_>, depth: usize, ordered: bool, ctx: &Ctx) -> String {
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
                        inline_parts.push_str(t);
                    }
                }
            }
        }
        out.push_str(&format!("{indent}{marker} {}\n", inline_parts.trim()));
        out.push_str(&nested);
    }
    out
}

/// `inline_md` renders a node's *children*; a child element reached directly
/// needs itself rendered, which is what this wrapper does.
fn inline_md_self(n: Ref<'_>, ctx: &Ctx) -> String {
    if dropped(&n, ctx.in_cell) {
        return String::new();
    }
    match tag(&n) {
        Some("a") => format!("[{}]({})", inline_md(n, ctx), attr(&n, "href").unwrap_or_default()),
        Some("strong" | "b") => emphasis(n, ctx, "**"),
        Some("em" | "i") => emphasis(n, ctx, "*"),
        Some("sup") => escape_md(&text_content(n)),
        Some("code") => format!("`{}`", text_content(n)),
        Some("br") => "  \n".to_string(),
        Some("img") => {
            let src = attr(&n, "src").unwrap_or_default();
            let src = ctx.images.get(src).map(String::as_str).unwrap_or(src);
            format!("![{}]({src})", attr(&n, "alt").unwrap_or_default())
        }
        _ => inline_md(n, ctx),
    }
}

// ── Tables ───────────────────────────────────────────────────────────────────

const CELL_BLOCKS: &[&str] = &["p", "div", "blockquote", "section", "article", "li"];

fn is_cell_block(name: &str) -> bool {
    CELL_BLOCKS.contains(&name) || is_heading(name)
}

/// A GFM cell must be a single line. Block children become segments joined by
/// " · "; lists collapse to "; "-joined items.
fn cell_md(cell: Ref<'_>, images: &ImageMap) -> String {
    let ctx = Ctx { images, in_cell: true };
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

    segs.join(" · ").replace(NBSP, " ").replace('|', "\\|").trim().to_string()
}

/// Collapse all whitespace runs to single spaces and trim.
fn squeeze(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn table_md(node: Ref<'_>, images: &ImageMap) -> String {
    let rows: Vec<Ref<'_>> = node.descendants().filter(|d| is_tag(d, "tr")).collect();
    let Some(first) = rows.first() else { return String::new() };

    let head: Vec<String> = element_children(*first).map(|c| cell_md(c, images)).collect();
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

// ── Blocks ───────────────────────────────────────────────────────────────────

fn block_md(node: Ref<'_>, ctx: &Ctx) -> String {
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
            out.push_str(&format!("{} {}\n\n", "#".repeat(level), inline_md(n, ctx).trim()));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn md(html: &str) -> String {
        to_markdown(html, &ImageMap::new())
    }

    #[test]
    fn headings_paragraphs_and_emphasis() {
        assert_eq!(
            md("<h2>Week 1</h2><p>Read <strong>chapter 3</strong> and <em>notes</em>.</p>"),
            "## Week 1\n\nRead **chapter 3** and *notes*."
        );
    }

    #[test]
    fn nested_lists_indent() {
        let out = md("<ul><li>a<ul><li>b</li></ul></li><li>c</li></ul>");
        assert_eq!(out, "- a\n  - b\n- c");
    }

    #[test]
    fn ordered_lists_number_from_one() {
        assert_eq!(md("<ol><li>first</li><li>second</li></ol>"), "1. first\n2. second");
    }

    #[test]
    fn tables_stay_single_line_per_row() {
        let out = md("<table><tr><th>Week</th><th>Topic</th></tr>\
                      <tr><td>1</td><td><p>Intro</p><p>Setup</p></td></tr></table>");
        assert_eq!(
            out,
            "| Week | Topic |\n| --- | --- |\n| 1 | Intro · Setup |"
        );
    }

    #[test]
    fn table_cells_escape_pipes_and_drop_images() {
        let out = md("<table><tr><td>a|b</td><td><img src='x.png'>text</td></tr></table>");
        assert!(out.contains(r"a\|b"), "{out}");
        assert!(!out.contains("x.png"), "{out}");
    }

    #[test]
    fn short_rows_are_padded_to_the_header_width() {
        let out = md("<table><tr><th>A</th><th>B</th></tr><tr><td>1</td></tr></table>");
        assert!(out.ends_with("| 1 |  |"), "{out}");
    }

    #[test]
    fn screenreader_and_script_content_is_dropped() {
        let out = md("<p>keep<span class='screenreader-only'>drop</span></p><script>bad()</script>");
        assert_eq!(out, "keep");
    }

    #[test]
    fn empty_emphasis_leaves_no_artifact() {
        // Canvas emits <strong></strong>; naive conversion yields "****".
        assert_eq!(md("<p>a<strong></strong>b</p>"), "ab");
    }

    #[test]
    fn marker_only_emphasis_is_not_wrapped() {
        assert_eq!(md("<p><strong>†</strong> note</p>"), "† note");
    }

    #[test]
    fn links_and_images_survive() {
        assert_eq!(
            md("<p><a href='/x'>go</a> <img src='p.png' alt='fig'></p>"),
            "[go](/x) ![fig](p.png)"
        );
    }

    #[test]
    fn image_src_is_rewritten_to_the_local_copy() {
        let mut images = ImageMap::new();
        images.insert("/courses/1/files/9/preview".into(), "images/9.png".into());
        let out = to_markdown("<p><img src='/courses/1/files/9/preview' alt='f'></p>", &images);
        assert_eq!(out, "![f](images/9.png)");
    }

    #[test]
    fn markdown_specials_in_text_are_escaped() {
        assert_eq!(md("<p>2*3 and _x_ [ok]</p>"), r"2\*3 and \_x\_ \[ok\]");
    }

    #[test]
    fn blockquotes_and_code_blocks() {
        assert_eq!(md("<blockquote><p>hi</p></blockquote>"), "> hi");
        assert_eq!(md("<pre>let x = 1;\n</pre>"), "```\nlet x = 1;\n```");
    }

    #[test]
    fn finds_image_endpoints_from_either_attribute() {
        let refs = image_refs(
            "<img data-api-endpoint='/api/v1/files/5' src='a.png'>\
             <img data-id='7' src='b.png'><img src='c.png'>",
        );
        assert_eq!(
            refs,
            vec![
                ("/api/v1/files/5".to_string(), "a.png".to_string()),
                ("/api/v1/files/7".to_string(), "b.png".to_string()),
            ]
        );
    }

    #[test]
    fn extracts_page_slugs_and_course_file_ids() {
        let (pages, files) = canvas_links(
            "<a href='/courses/42/pages/week-one?x=1'>p</a>\
             <a href='/courses/42/files/123/download'>f</a>\
             <a href='/courses/99/files/456'>other course</a>",
            42,
        );
        assert_eq!(pages, vec!["week-one"]);
        assert_eq!(files, vec!["123"]);
    }
}
