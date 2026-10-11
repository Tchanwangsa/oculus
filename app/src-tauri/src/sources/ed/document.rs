//! Ed's `<document>` XML → Markdown.
//!
//! Parsed with the HTML parser, so void-style tags like <break/> swallow their
//! following siblings: every renderer emits its marker and still recurses.

use ego_tree::NodeRef;
use scraper::node::Node;
use scraper::Html;

type Ref<'a> = NodeRef<'a, Node>;

fn tag<'a>(n: &Ref<'a>) -> Option<&'a str> {
    n.value().as_element().map(|e| e.name())
}

fn attr<'a>(n: &Ref<'a>, name: &str) -> Option<&'a str> {
    n.value().as_element().and_then(|e| e.attr(name))
}

pub fn document_md(xml: &str) -> String {
    // `<link>` is void to an HTML parser and would strand its text; rename it.
    // (html5ever rewrites `<image>` to `<img>`, keeping `src`.)
    let xml = xml
        .replace("<link ", "<edlink ")
        .replace("</link>", "</edlink>");
    let doc = Html::parse_fragment(&xml);
    let mut out = String::new();
    render_nodes(doc.tree.root(), &mut out);
    crate::pages::md::collapse_blank_lines(&out)
        .trim()
        .to_string()
}

fn render_nodes(n: Ref<'_>, out: &mut String) {
    for c in n.children() {
        render_node(c, out);
    }
}

fn inner(n: Ref<'_>) -> String {
    let mut s = String::new();
    render_nodes(n, &mut s);
    s
}

fn render_node(n: Ref<'_>, out: &mut String) {
    if let Some(t) = n.value().as_text() {
        out.push_str(t);
        return;
    }
    let Some(name) = tag(&n) else {
        render_nodes(n, out);
        return;
    };
    match name {
        "paragraph" | "figure" => {
            render_nodes(n, out);
            out.push_str("\n\n");
        }
        "heading" => {
            let level = attr(&n, "level")
                .and_then(|l| l.parse().ok())
                .unwrap_or(2usize);
            out.push_str(&"#".repeat(level.clamp(1, 6)));
            out.push(' ');
            render_nodes(n, out);
            out.push_str("\n\n");
        }
        "bold" => wrap(n, "**", out),
        "italic" => wrap(n, "*", out),
        "underline" | "spoiler" => render_nodes(n, out),
        "code" => wrap(n, "`", out),
        "edlink" => {
            let href = attr(&n, "href").unwrap_or("");
            let text = inner(n);
            let text = text.trim();
            if text.is_empty() {
                out.push_str(href);
            } else {
                out.push_str(&format!("[{text}]({href})"));
            }
        }
        "image" | "img" => {
            out.push_str(&format!("![]({})\n\n", attr(&n, "src").unwrap_or("")));
            render_nodes(n, out);
        }
        "break" => {
            out.push_str("  \n");
            render_nodes(n, out);
        }
        "list" => {
            let ordered = attr(&n, "style") == Some("number");
            let mut i = 1;
            for c in n.children() {
                if tag(&c) == Some("list-item") {
                    let marker = if ordered {
                        let m = format!("{i}.");
                        i += 1;
                        m
                    } else {
                        "-".to_string()
                    };
                    let body = inner(c);
                    out.push_str(&format!("{marker} {}\n", squeeze_item(&body)));
                } else {
                    render_node(c, out);
                }
            }
            out.push('\n');
        }
        "callout" => {
            let body = inner(n);
            for line in body.trim().lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }
        // Raw LaTeX, emitted as $$ display math.
        "math" => {
            let latex: String = n
                .descendants()
                .filter_map(|d| d.value().as_text().map(|t| t.to_string()))
                .collect();
            let latex = latex.trim();
            if !latex.is_empty() {
                out.push_str(&format!("\n$$\n{latex}\n$$\n\n"));
            }
        }
        "pre" | "snippet" => {
            let body: String = n
                .descendants()
                .filter_map(|d| d.value().as_text().map(|t| t.to_string()))
                .collect();
            out.push_str(&format!("```\n{}\n```\n\n", body.trim_end_matches('\n')));
        }
        _ => render_nodes(n, out),
    }
}

fn wrap(n: Ref<'_>, marks: &str, out: &mut String) {
    let body = inner(n);
    let trimmed = body.trim();
    if trimmed.is_empty() {
        out.push_str(&body);
    } else {
        out.push_str(&format!("{marks}{trimmed}{marks}"));
    }
}

/// A list item must stay on one line or the list breaks apart.
fn squeeze_item(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
