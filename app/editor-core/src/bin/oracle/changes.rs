//! `changes`: one document and three change sets on it, answered with every
//! `ChangeSet`/`ChangeDesc` operation and selection normalising and mapping,
//! and change sets and descs read back from CodeMirror's `toJSON` forms.
//! `oracle/changes.ts` computes the same with `@codemirror/state`. The JSON
//! helpers here are shared with `history`.

use oculus_editor_core::text::{
    ChangeDesc, ChangeError, ChangeSet, ChangeSpec, MapMode, Part, RangeChange, Selection,
    SelectionRange, State, Text, Touch,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// `ChangeSet.fromJSON`: a number keeps that many units, `[len, ...lines]`
/// replaces `len` units with the lines (none for a deletion).
pub fn set_from_json(json: &Value) -> Result<ChangeSet, String> {
    let bad = || format!("not a ChangeSet JSON: {json}");
    let parts = json
        .as_array()
        .ok_or_else(bad)?
        .iter()
        .map(|part| {
            if let Some(len) = part.as_u64() {
                return Ok(Part::Keep(len as usize));
            }
            let items = part.as_array().ok_or_else(bad)?;
            let len = items.first().and_then(Value::as_u64).ok_or_else(bad)?;
            let lines = items[1..]
                .iter()
                .map(|l| l.as_str().map(str::to_owned).ok_or_else(bad))
                .collect::<Result<_, _>>()?;
            Ok(Part::Replace(len as usize, lines))
        })
        .collect::<Result<Vec<_>, String>>()?;
    ChangeSet::from_parts(&parts).map_err(|e| e.to_string())
}

/// `ChangeDesc.fromJSON`: flat `len, ins` pairs.
pub fn desc_from_json(json: &[isize]) -> Result<ChangeDesc, String> {
    if json.len() % 2 == 1 || json.iter().step_by(2).any(|&len| len < 0) {
        return Err(format!("not a ChangeDesc JSON: {json:?}"));
    }
    let sections: Vec<(usize, isize)> = json.chunks(2).map(|p| (p[0] as usize, p[1])).collect();
    ChangeDesc::from_sections(&sections).map_err(|e| e.to_string())
}

/// A change spec: `[from, to, insert]`, or `{"set": [...]}` for a nested
/// change set over the same document.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Spec {
    Replace(usize, usize, String),
    Set { set: Vec<Spec> },
}

pub fn change_set(specs: &[Spec], len: usize) -> Result<ChangeSet, String> {
    let specs = specs
        .iter()
        .map(|spec| {
            Ok(match spec {
                Spec::Replace(from, to, insert) => ChangeSpec::replace(*from, *to, insert),
                Spec::Set { set } => ChangeSpec::Set(change_set(set, len)?),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    ChangeSet::of(&specs, len).map_err(|e| e.to_string())
}

/// A range as `[anchor, head, goal_column, bidi_level, assoc]`, built with
/// `EditorSelection.range`.
#[derive(Deserialize)]
pub struct RangeSpec(usize, usize, Option<f64>, Option<u8>, i8);

#[derive(Deserialize)]
pub struct SelectionSpec {
    ranges: Vec<RangeSpec>,
    main: usize,
}

fn range(r: &RangeSpec) -> SelectionRange {
    SelectionRange::range(r.0, r.1, r.2, r.3, r.4)
}

pub fn selection(spec: &SelectionSpec) -> Result<Selection, String> {
    let ranges = spec.ranges.iter().map(range).collect();
    Selection::create(ranges, spec.main).map_err(|e| e.to_string())
}

/// `ChangeSet.toJSON`.
pub fn set_json(set: &ChangeSet) -> Value {
    Value::Array(
        set.parts()
            .into_iter()
            .map(|part| match part {
                Part::Keep(len) => json!(len),
                Part::Replace(len, lines) => {
                    let mut out = vec![json!(len)];
                    out.extend(lines.into_iter().map(Value::String));
                    Value::Array(out)
                }
            })
            .collect(),
    )
}

/// `ChangeDesc.toJSON`: flat `len, ins` pairs.
pub fn desc_json(desc: &ChangeDesc) -> Value {
    Value::Array(
        desc.sections()
            .flat_map(|(len, ins)| [json!(len), json!(ins)])
            .collect(),
    )
}

/// Ranges as `[anchor, head, assoc, goal_column, bidi_level]`, plus the main
/// index.
pub fn selection_json(sel: &Selection) -> Value {
    let ranges: Vec<Value> = sel.ranges().iter().map(range_json).collect();
    json!({ "ranges": ranges, "main": sel.main_index() })
}

fn range_json(r: &SelectionRange) -> Value {
    json!([
        r.anchor(),
        r.head(),
        r.assoc(),
        r.goal_column(),
        r.bidi_level()
    ])
}

/// Selection methods to run on one of `selections`.
#[derive(Deserialize)]
pub struct SelectionOps {
    /// `main.extend(from, to, assoc)`.
    extend: (usize, usize, i8),
    /// `addRange(range, main)`.
    add: (RangeSpec, bool),
    /// `replaceRange(range, which)`.
    replace: (RangeSpec, usize),
}

/// `changeByRange` with one of the recipes `oracle/changes.ts` also runs:
/// delete the range or the code point before a cursor, wrap the range in
/// `**`, or insert at the head.
fn by_range(doc: &Text, sel: &Selection, recipe: &str) -> Result<(ChangeSet, Selection), String> {
    let state = State::with_selection(doc.clone(), sel.clone()).map_err(|e| e.to_string())?;
    let result = state.change_by_range(|r| match recipe {
        "delete_back" => {
            let (from, to) = if r.is_empty() {
                let head = r.head();
                let before = if head >= 2
                    && doc
                        .slice_string(head - 2, head)
                        .is_ok_and(|s| s.chars().count() == 1)
                {
                    2
                } else {
                    head.min(1)
                };
                (head - before, head)
            } else {
                (r.from(), r.to())
            };
            RangeChange {
                changes: vec![ChangeSpec::delete(from, to)],
                range: SelectionRange::cursor(from, 0, None, None),
            }
        }
        "wrap" => RangeChange {
            changes: vec![
                ChangeSpec::insert(r.from(), "**"),
                ChangeSpec::insert(r.to(), "**"),
            ],
            range: SelectionRange::new(r.anchor() + 2, r.head() + 2),
        },
        _ => RangeChange {
            changes: vec![ChangeSpec::insert(r.head(), "ไ😀")],
            range: SelectionRange::cursor(r.head() + 3, 0, None, None),
        },
    });
    result.map_err(|e| e.to_string())
}

const RECIPES: [&str; 3] = ["delete_back", "wrap", "insert_head"];

#[derive(Deserialize)]
pub struct Request {
    doc: String,
    /// On `doc`.
    a: Vec<Spec>,
    /// On `a` applied to `doc`.
    b: Vec<Spec>,
    /// On `doc`.
    c: Vec<Spec>,
    /// Old positions to map through `a`.
    positions: Vec<usize>,
    /// Ranges for `a.touchesRange`.
    ranges: Vec<(usize, usize)>,
    /// Selections on `doc`, normalised and mapped through `a`.
    selections: Vec<SelectionSpec>,
    /// One per selection.
    selection_ops: Vec<SelectionOps>,
    /// CodeMirror's `toJSON` of change sets, read back with `fromJSON`, each
    /// with the document it applies to.
    #[serde(default)]
    json_sets: Vec<(Value, String)>,
    /// CodeMirror's `toJSON` of change descs, read back with `fromJSON`.
    #[serde(default)]
    json_descs: Vec<Vec<isize>>,
    /// Raw JSON sets, composed and mapped.
    #[serde(default)]
    raw_sets: Option<RawSets>,
}

/// `a` and `c` on one document, `b` on `a`'s result, with sections that
/// may not be merged as CodeMirror merges its own.
#[derive(Deserialize)]
pub struct RawSets {
    a: Value,
    b: Value,
    c: Value,
}

/// The `try_` operations on raw sets, `"throw"` where they refuse.
fn raw_ops(r: &RawSets) -> Result<Value, String> {
    let (a, b, c) = (
        set_from_json(&r.a)?,
        set_from_json(&r.b)?,
        set_from_json(&r.c)?,
    );
    let or_throw = |v: Result<Value, ChangeError>| v.unwrap_or_else(|_| json!("throw"));
    Ok(json!([
        or_throw(a.try_compose(&b).map(|s| set_json(&s))),
        or_throw(a.try_map(&c, false).map(|s| set_json(&s))),
        or_throw(a.try_map(&c, true).map(|s| set_json(&s))),
        or_throw(a.desc().try_compose_desc(b.desc()).map(|d| desc_json(&d))),
        or_throw(a.desc().try_map_desc(c.desc(), true).map(|d| desc_json(&d))),
    ]))
}

const MODES: [MapMode; 4] = [
    MapMode::Simple,
    MapMode::TrackDel,
    MapMode::TrackBefore,
    MapMode::TrackAfter,
];

fn changes_json(set: &ChangeSet, individual: bool) -> Value {
    Value::Array(
        set.iter_changes(individual)
            .map(|c| json!([c.from_a, c.to_a, c.from_b, c.to_b, c.inserted.to_string()]))
            .collect(),
    )
}

/// A set read back from JSON: its own JSON, lengths and desc, `doc` with
/// it applied, and its inverse.
fn json_set(json: &Value, doc: &str) -> Result<Value, String> {
    let set = set_from_json(json)?;
    let doc = Text::of(doc);
    let applied = set.apply(&doc).map_err(|e| e.to_string())?;
    let inverted = set.invert(&doc).map_err(|e| e.to_string())?;
    Ok(json!([
        set_json(&set),
        set.length(),
        set.new_length(),
        desc_json(set.desc()),
        applied.to_string(),
        set_json(&inverted),
    ]))
}

/// A desc read back from JSON: its own JSON, lengths, and every position
/// of a short one mapped with both assocs.
fn json_desc(json: &[isize]) -> Result<Value, String> {
    let desc = desc_from_json(json)?;
    let mapped: Vec<Value> = if desc.length() <= 60 {
        (0..=desc.length())
            .map(|pos| json!([desc.map_pos(pos, -1), desc.map_pos(pos, 1)]))
            .collect()
    } else {
        Vec::new()
    };
    Ok(json!([
        desc_json(&desc),
        desc.length(),
        desc.new_length(),
        mapped
    ]))
}

pub fn answer(r: Request) -> Result<Value, String> {
    let doc = Text::of(&r.doc);
    let a = change_set(&r.a, doc.len())?;
    let applied = a.apply(&doc).map_err(|e| e.to_string())?;
    let b = change_set(&r.b, applied.len())?;
    let c = change_set(&r.c, doc.len())?;
    let inverted = a.invert(&doc).map_err(|e| e.to_string())?;
    let composed = a.compose(&b);
    let map_pos: Vec<Value> = r
        .positions
        .iter()
        .map(|&pos| {
            let mut out = Vec::new();
            for assoc in [-1, 1] {
                for mode in MODES {
                    out.push(json!(a.map_pos_mode(pos, assoc, mode)));
                }
            }
            Value::Array(out)
        })
        .collect();
    let touches: Vec<Value> = r
        .ranges
        .iter()
        .map(|&(from, to)| match a.touches_range(from, to) {
            Touch::No => json!(false),
            Touch::Yes => json!(true),
            Touch::Cover => json!("cover"),
        })
        .collect();
    let selections = r
        .selections
        .iter()
        .zip(&r.selection_ops)
        .map(|(spec, ops)| {
            let sel = selection(spec)?;
            let by_range = RECIPES
                .iter()
                .map(|recipe| {
                    let (changes, after) = by_range(&doc, &sel, recipe)?;
                    Ok(json!([set_json(&changes), selection_json(&after)]))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let (from, to, assoc) = ops.extend;
            Ok(json!([
                selection_json(&sel),
                selection_json(&sel.map(&a, -1)),
                selection_json(&sel.map(&a, 1)),
                range_json(&sel.main().extend(from, to, assoc)),
                selection_json(&sel.as_single()),
                selection_json(&sel.add_range(range(&ops.add.0), ops.add.1)),
                selection_json(&sel.replace_range(range(&ops.replace.0), ops.replace.1)),
                by_range,
            ]))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(json!({
        "a": set_json(&a),
        "b": set_json(&b),
        "c": set_json(&c),
        "empty": a.is_empty(),
        "length": a.length(),
        "new_length": a.new_length(),
        "applied": applied.to_string(),
        "inverted": set_json(&inverted),
        "inverted_applied": inverted.apply(&applied).map_err(|e| e.to_string())?.to_string(),
        "inverted_desc": desc_json(&a.inverted_desc()),
        "composed": set_json(&composed),
        "composed_applied": composed.apply(&doc).map_err(|e| e.to_string())?.to_string(),
        "compose_desc": desc_json(&a.desc().compose_desc(b.desc())),
        "map": set_json(&a.map(&c, false)),
        "map_before": set_json(&a.map(&c, true)),
        "c_map_a": set_json(&c.map(&a, false)),
        "set_map_desc": set_json(&a.map_desc(&c, true)),
        "map_desc": desc_json(&a.desc().map_desc(c.desc(), false)),
        "map_desc_before": desc_json(&a.desc().map_desc(c.desc(), true)),
        "map_pos": map_pos,
        "changes": changes_json(&a, false),
        "changes_individual": changes_json(&a, true),
        "gaps": a.iter_gaps().into_iter().map(|(x, y, l)| json!([x, y, l])).collect::<Vec<_>>(),
        "changed_ranges": a
            .desc()
            .iter_changed_ranges(true)
            .map(|c| json!([c.from_a, c.to_a, c.from_b, c.to_b]))
            .collect::<Vec<_>>(),
        "touches": touches,
        "selections": selections,
        "json_sets": r.json_sets.iter().map(|(j, on)| json_set(j, on)).collect::<Result<Vec<_>, _>>()?,
        "json_descs": r.json_descs.iter().map(|j| json_desc(j)).collect::<Result<Vec<_>, _>>()?,
        "raw_ops": r.raw_sets.as_ref().map(raw_ops).transpose()?,
    }))
}
