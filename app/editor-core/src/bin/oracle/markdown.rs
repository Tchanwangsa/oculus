//! `markdown`: the parse tree of one document, and `resolve_inner` probes on
//! it. Answers mirror what `oracle/markdown.ts` computes with the app's
//! `@lezer/markdown` parser (nested code languages pruned).

use oculus_editor_core::markdown::{self, Node, NodeType, Tree};
use oculus_editor_core::text::{ChangeSet, ChangeSpec, Text};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
pub struct Query {
    /// Raw source; `Text::of` normalises its line breaks first.
    text: String,
    /// `resolveInner(pos, side)` for each; answered with the node and its
    /// ancestors.
    #[serde(default)]
    probes: Vec<(usize, i32)>,
    /// Also answer the node type names, by id.
    #[serde(default)]
    node_types: bool,
    /// Also answer each node's parent, first and last child, and next and
    /// previous sibling.
    #[serde(default)]
    nav: bool,
    /// Also answer `resolveInner` at every position 0..=len+1, sides -1, 0, 1.
    #[serde(default)]
    all_positions: bool,
    /// Edit steps, each a list of `[from, to, insert]` in the document
    /// before it; the tree after each, reparsed incrementally, is answered.
    #[serde(default)]
    edits: Vec<Vec<(usize, usize, String)>>,
    /// Answer each edited tree's navigation too, and `resolveInner` at
    /// every position when it is at most this long.
    #[serde(default)]
    edit_nav_limit: Option<usize>,
}

fn nav(tree: &Tree) -> Vec<String> {
    tree.iter()
        .map(|n| {
            let links = [
                n.parent(),
                n.first_child(),
                n.last_child(),
                n.next_sibling(),
                n.prev_sibling(),
            ];
            links.map(key).join("|")
        })
        .collect()
}

fn all_positions(tree: &Tree) -> Vec<String> {
    let len = tree.root().to();
    (0..=len + 1)
        .flat_map(|pos| [-1, 0, 1].map(|side| chain(tree, pos, side).join("<")))
        .collect()
}

fn dump(tree: &Tree) -> Vec<String> {
    tree.iter().map(|n| key(Some(n))).collect()
}

fn key(node: Option<Node>) -> String {
    node.map_or("-".into(), |n| {
        format!("{} {} {}", n.name(), n.from(), n.to())
    })
}

fn chain(tree: &Tree, pos: usize, side: i32) -> Vec<String> {
    let mut out = Vec::new();
    let mut node = Some(tree.resolve_inner(pos, side));
    while let Some(n) = node {
        out.push(key(Some(n)));
        node = n.parent();
    }
    out
}

pub fn parse(query: Query) -> Result<Value, String> {
    let tree = markdown::parse(&Text::of(&query.text));
    let nodes: Vec<String> = tree
        .iter()
        .map(|n| format!("{} {} {}", n.name(), n.from(), n.to()))
        .collect();
    let resolved: Vec<Vec<String>> = query
        .probes
        .iter()
        .map(|&(pos, side)| chain(&tree, pos, side))
        .collect();
    let mut answer = json!({ "nodes": nodes, "resolved": resolved });
    if query.nav {
        answer["nav"] = json!(nav(&tree));
    }
    if !query.edits.is_empty() {
        let mut doc = Text::of(&query.text);
        let mut tree = tree.clone();
        let mut steps = Vec::new();
        for step in &query.edits {
            let specs: Vec<ChangeSpec> = step
                .iter()
                .map(|(from, to, insert)| ChangeSpec::replace(*from, *to, insert))
                .collect();
            let changes = ChangeSet::of(&specs, doc.len()).map_err(|e| e.to_string())?;
            doc = changes.apply(&doc).map_err(|e| e.to_string())?;
            tree = markdown::reparse(&tree, &doc, &changes);
            let mut step = json!({ "nodes": dump(&tree) });
            if let Some(limit) = query.edit_nav_limit {
                step["nav"] = json!(nav(&tree));
                if doc.len() <= limit {
                    step["all_positions"] = json!(all_positions(&tree));
                }
            }
            steps.push(step);
        }
        answer["edits"] = json!(steps);
    }
    if query.all_positions {
        answer["all_positions"] = json!(all_positions(&tree));
    }
    if query.node_types {
        let names: Vec<&str> = NodeType::ALL.iter().map(|t| t.name()).collect();
        answer["node_types"] = json!(names);
    }
    Ok(answer)
}
