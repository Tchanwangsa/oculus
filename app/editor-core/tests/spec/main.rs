//! The CommonMark 0.31.2 examples and the GFM table, strikethrough and
//! task-list examples, rendered from the parse tree to HTML and compared with
//! the spec's HTML. Lezer is the target, not the spec: every example that
//! differs is listed in `DIVERGENCES.md` with the reason, and this test fails
//! when the set of differing examples changes either way.
//!
//! The renderer is the reference renderer's logic over Lezer's tree: block
//! nesting and inline nodes come from the tree; what the tree leaves to a
//! renderer (link definitions, tight lists, entity and escape decoding, URL
//! encoding, code-span and line-break whitespace) is done here.

mod json;
mod render;
mod text;

use std::collections::{BTreeSet, HashMap};

use json::{Json, read_json};
use render::render;

/// HTML compared leniently: whitespace between lines collapsed (but not
/// inside `<pre>`), self-closing slashes, `<input>` attribute order and an
/// empty `<tbody>` ignored, table alignment as `align` or `style`.
fn normalize_html(html: &str) -> String {
    let mut s = html
        .replace("/>", ">")
        .replace(" >", ">")
        .replace("<tbody></tbody>", "");
    for (from, to) in [
        ("style=\"text-align: center\"", "align=\"center\""),
        ("style=\"text-align: left\"", "align=\"left\""),
        ("style=\"text-align: right\"", "align=\"right\""),
    ] {
        s = s.replace(from, to);
    }
    // Sort the attributes of <input> (task lists).
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(i) = rest.find("<input") {
        out.push_str(&rest[..i]);
        let end = rest[i..].find('>').map_or(rest.len(), |e| i + e + 1);
        let tag = &rest[i..end];
        let mut attrs: Vec<&str> = tag
            .trim_start_matches("<input")
            .trim_end_matches('>')
            .split_whitespace()
            .collect();
        attrs.sort_unstable();
        out.push_str(&format!("<input {}> ", attrs.join(" ")));
        rest = rest[end..].trim_start();
    }
    out.push_str(rest);
    // Collapse whitespace between lines, except inside <pre>, where it is
    // content.
    let mut normal = String::new();
    let mut rest = out.as_str();
    loop {
        let (outside, pre) = match rest.find("<pre") {
            Some(i) => {
                let end = rest[i..].find("</pre>").map_or(rest.len(), |e| i + e + 6);
                (&rest[..i], Some(&rest[i..end]))
            }
            None => (rest, None),
        };
        let collapsed: Vec<&str> = outside
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        normal.push_str(&collapsed.join("\n"));
        let Some(pre) = pre else { break };
        normal.push('\n');
        normal.push_str(pre);
        normal.push('\n');
        rest = &rest[outside.len() + pre.len()..];
    }
    normal.trim().to_string()
}

/// Example ids listed in `DIVERGENCES.md`: lines `- <id>:`.
fn recorded_divergences() -> BTreeSet<String> {
    let text = std::fs::read_to_string(format!("{}/DIVERGENCES.md", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_default();
    text.lines()
        .filter_map(|l| l.strip_prefix("- "))
        .filter_map(|l| {
            l.split_once(':')
                .map(|(id, _)| id.trim().trim_matches('`').to_string())
        })
        .collect()
}

fn entity_table() -> HashMap<String, String> {
    match read_json("entities.json") {
        Json::Obj(fields) => fields
            .into_iter()
            .map(|(k, v)| (k, v.str().to_string()))
            .collect(),
        _ => panic!("entities.json"),
    }
}

#[test]
fn spec_examples_match_or_are_recorded() {
    let entities = entity_table();
    let mut differing = BTreeSet::new();
    let mut report = String::new();
    let mut total = 0;
    let Json::Arr(examples) = read_json("commonmark-spec.json") else {
        panic!()
    };
    for ex in &examples {
        let Json::Num(number) = ex.get("example") else {
            panic!()
        };
        let (md, html) = (ex.get("markdown").str(), ex.get("html").str());
        total += 1;
        let got = render(md, &entities);
        if normalize_html(&got) != normalize_html(html) {
            let id = format!("{number}");
            report.push_str(&format!(
                "{id} [{}]\n  md:   {md:?}\n  want: {html:?}\n  got:  {got:?}\n",
                ex.get("section").str()
            ));
            differing.insert(id);
        }
    }
    let Json::Arr(gfm) = read_json("gfm-examples.json") else {
        panic!()
    };
    for ex in &gfm {
        let (name, md, html) = (
            ex.get("name").str(),
            ex.get("markdown").str(),
            ex.get("html").str(),
        );
        total += 1;
        let got = render(md, &entities);
        if normalize_html(&got) != normalize_html(html) {
            report.push_str(&format!(
                "{name}\n  md:   {md:?}\n  want: {html:?}\n  got:  {got:?}\n"
            ));
            differing.insert(name.to_string());
        }
    }
    if std::env::var_os("SPEC_REPORT").is_some() {
        std::fs::write(
            format!("{}/target/spec-report.txt", env!("CARGO_MANIFEST_DIR")),
            &report,
        )
        .unwrap();
    }
    let recorded = recorded_divergences();
    let new: Vec<_> = differing.difference(&recorded).collect();
    let fixed: Vec<_> = recorded.difference(&differing).collect();
    assert!(
        new.is_empty() && fixed.is_empty(),
        "{} of {total} examples differ. Not in DIVERGENCES.md: {new:?}. Listed but now matching: {fixed:?}. \
         Run with SPEC_REPORT=1 for target/spec-report.txt.",
        differing.len()
    );
}
