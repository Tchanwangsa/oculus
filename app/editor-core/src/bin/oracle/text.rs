//! `text` and `text_edits`: the rope's queries and edits. Answers mirror what
//! `oracle/text.ts` computes with `@codemirror/state`'s `Text`. `text_chunks`
//! reports where the rope's leaves split, so the driver can probe there.

use std::collections::HashMap;

use oculus_editor_core::text::{Line, Text};
use serde::Deserialize;
use serde_json::{Value, json};

/// Line texts longer than this many units answer `{len, fnv1a}` instead.
const LINE_TEXT_LIMIT: usize = 2000;

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Queries {
    /// `line(n)` for each.
    lines: Vec<usize>,
    /// `lineAt(pos)` for each.
    positions: Vec<usize>,
    /// `sliceString(from, to)` for each.
    ranges: Vec<(usize, usize)>,
}

#[derive(Deserialize)]
pub struct Probe {
    /// Raw source; `Text::of` does the line-break normalising under test.
    doc: String,
    #[serde(flatten)]
    queries: Queries,
    /// Whole-document `iter()` and `iter(-1)`.
    #[serde(default)]
    iter: bool,
    /// `iterRange(from, to)`, either order.
    #[serde(default)]
    iter_ranges: Vec<(usize, usize)>,
    /// `iterLines(from, to)`.
    #[serde(default)]
    iter_lines: Vec<(usize, usize)>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Edit {
    Replace {
        from: usize,
        to: usize,
        insert: String,
    },
    Append {
        insert: String,
    },
    Slice {
        from: usize,
        to: usize,
    },
}

#[derive(Deserialize)]
pub struct Step {
    #[serde(flatten)]
    edit: Edit,
    #[serde(flatten)]
    queries: Queries,
}

#[derive(Deserialize)]
pub struct Edits {
    doc: String,
    steps: Vec<Step>,
    /// Answer the final document's full text.
    #[serde(default)]
    final_text: bool,
    /// Answer whether the final document equals `Text::of(eq_other)`.
    #[serde(default)]
    eq_other: Option<String>,
}

#[derive(Deserialize)]
pub struct ChunksRequest {
    doc: String,
}

/// FNV-1a (32-bit) over UTF-16 units; `oracle/text.ts` hashes the same way.
pub fn fnv1a(text: &str) -> u32 {
    text.encode_utf16().fold(0x811c_9dc5, |h, unit| {
        (h ^ u32::from(unit)).wrapping_mul(0x0100_0193)
    })
}

/// A line as `[number, from, to, text]`. Long lines' hashes are memoised by
/// line number in `hashes` (one per document version), since many probes can
/// land on the same 100 KB line.
fn line_json(line: Line, hashes: &mut HashMap<usize, u32>) -> Value {
    let text = if line.len() <= LINE_TEXT_LIMIT {
        json!(line.text)
    } else {
        let hash = *hashes
            .entry(line.number)
            .or_insert_with(|| fnv1a(&line.text));
        json!({ "len": line.len(), "fnv1a": hash })
    };
    json!([line.number, line.from, line.to, text])
}

fn queries(doc: &Text, q: &Queries) -> Result<Value, String> {
    let mut hashes = HashMap::new();
    let lines = q
        .lines
        .iter()
        .map(|&n| {
            doc.line(n)
                .map(|line| line_json(line, &mut hashes))
                .ok_or(format!("no line {n}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let line_at = q
        .positions
        .iter()
        .map(|&pos| doc.line_at(pos).map(|line| line_json(line, &mut hashes)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let slice = q
        .ranges
        .iter()
        .map(|&(from, to)| doc.slice_string(from, to))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(json!({
        "length": doc.len(),
        "lines": doc.lines(),
        "line": lines,
        "line_at": line_at,
        "slice": slice,
    }))
}

pub fn probe(p: Probe) -> Result<Value, String> {
    let doc = Text::of(&p.doc);
    let mut answer = queries(&doc, &p.queries)?;
    if p.iter {
        let forward: String = doc.iter().collect();
        let mut back: Vec<&str> = doc.iter_rev().collect();
        back.reverse();
        answer["iter"] = json!([forward, back.concat()]);
    }
    let mut ranges = Vec::new();
    for &(from, to) in &p.iter_ranges {
        let mut runs: Vec<&str> = doc
            .iter_range(from, to)
            .map_err(|e| e.to_string())?
            .collect();
        if from > to {
            runs.reverse();
        }
        ranges.push(runs.concat());
    }
    answer["iter_range"] = json!(ranges);
    let mut lines = Vec::new();
    for &(from, to) in &p.iter_lines {
        let iter = doc
            .iter_lines(from, to)
            .ok_or(format!("bad iter_lines {from}..{to}"))?;
        lines.push(iter.collect::<Vec<_>>());
    }
    answer["iter_lines"] = json!(lines);
    Ok(answer)
}

pub fn edits(e: Edits) -> Result<Value, String> {
    let mut doc = Text::of(&e.doc);
    let mut steps = Vec::with_capacity(e.steps.len());
    for step in &e.steps {
        doc = match &step.edit {
            Edit::Replace { from, to, insert } => doc.replace(*from, *to, &Text::of(insert)),
            Edit::Append { insert } => Ok(doc.append(&Text::of(insert))),
            Edit::Slice { from, to } => doc.slice(*from, *to),
        }
        .map_err(|e| e.to_string())?;
        steps.push(queries(&doc, &step.queries)?);
    }
    let text = doc.to_string();
    // Equal content in a freshly built (differently shaped) tree must compare equal.
    let eq_fresh = doc == Text::of(&text);
    let mut answer = json!({ "steps": steps, "eq_fresh": eq_fresh });
    if e.final_text {
        answer["text"] = json!(text);
    }
    if let Some(other) = &e.eq_other {
        answer["eq_other"] = json!(doc == Text::of(other));
    }
    Ok(answer)
}

pub fn chunks(request: ChunksRequest) -> Value {
    let doc = Text::of(&request.doc);
    let mut pos = 0;
    let mut cuts: Vec<usize> = doc
        .chunks()
        .map(|chunk| {
            pos += chunk.encode_utf16().count();
            pos
        })
        .collect();
    cuts.pop();
    json!({ "boundaries": cuts })
}
