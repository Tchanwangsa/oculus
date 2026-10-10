//! CodeMirror's JSON forms, read into the core's types and written back:
//! `ChangeSet.toJSON`, `ChangeDesc.toJSON`, `EditorSelection.toJSON` and
//! `historyField.toJSON`. Written JSON keeps CodeMirror's key order, so it
//! equals `JSON.stringify` of CodeMirror's own as a string.
//!
//! A selection range may also carry `goalColumn`, `bidiLevel` and `assoc`,
//! which `toJSON` drops: the history compares goal columns when it records
//! selections, so a mirror fed CodeMirror's live selections needs them. And
//! its `from` and `to`, which mapping can leave as `from > to`; given them,
//! the range is taken as CodeMirror holds it.

use oculus_editor_core::text::{
    ChangeDesc, ChangeSet, History, HistoryConfig, HistoryEvent, Isolate, Part, Selection,
    SelectionRange,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RangeJson {
    anchor: usize,
    head: usize,
    #[serde(default, skip_serializing)]
    goal_column: Option<f64>,
    #[serde(default, skip_serializing)]
    bidi_level: Option<u8>,
    #[serde(default, skip_serializing)]
    assoc: i8,
    #[serde(default, skip_serializing)]
    from: Option<usize>,
    #[serde(default, skip_serializing)]
    to: Option<usize>,
}

#[derive(Deserialize, Serialize)]
struct SelectionJson {
    ranges: Vec<RangeJson>,
    main: usize,
}

/// `HistEvent.toJSON`; an absent field is left out, as `JSON.stringify`
/// leaves out CodeMirror's `undefined`s.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct EventJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    changes: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mapped: Option<Vec<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start_selection: Option<SelectionJson>,
    selections_after: Vec<SelectionJson>,
}

/// `historyField.toJSON`, read with two optional extras it leaves out: a live
/// `HistoryState`'s `prevTime` and `prevUserEvent`, so that a mirror seeded
/// mid-session groups the next edit as CodeMirror will. Never written.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryJson {
    done: Vec<EventJson>,
    undone: Vec<EventJson>,
    #[serde(default, skip_serializing)]
    prev_time: Option<f64>,
    #[serde(default, skip_serializing)]
    prev_user_event: Option<String>,
}

fn parse<'a, T: Deserialize<'a>>(json: &'a str, what: &str) -> Result<T, String> {
    serde_json::from_str(json).map_err(|e| format!("bad {what} JSON: {e}"))
}

/// `ChangeSet.fromJSON`: a number keeps that many units, `[len, ...lines]`
/// replaces `len` units with the lines joined by `\n` (none: a deletion).
fn set_from_value(json: &Value) -> Result<ChangeSet, String> {
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

/// `ChangeDesc.fromJSON`: flat `len, ins` pairs, `ins` -1 for kept.
fn desc_from_flat(json: &[i64]) -> Result<ChangeDesc, String> {
    if json.len() % 2 == 1 || json.iter().step_by(2).any(|&len| len < 0) {
        return Err(format!("not a ChangeDesc JSON: {json:?}"));
    }
    let sections: Vec<(usize, isize)> = json
        .chunks(2)
        .map(|p| (p[0] as usize, p[1] as isize))
        .collect();
    ChangeDesc::from_sections(&sections).map_err(|e| e.to_string())
}

fn selection_from(s: &SelectionJson) -> Result<Selection, String> {
    let ranges = s
        .ranges
        .iter()
        .map(|r| match (r.from, r.to) {
            (Some(from), Some(to)) => SelectionRange::raw(
                (r.anchor, r.head),
                (from, to),
                r.goal_column,
                r.bidi_level,
                r.assoc,
            )
            .ok_or_else(|| {
                format!(
                    "range from {from} to {to} has neither end at anchor {} and head {}",
                    r.anchor, r.head
                )
            }),
            _ => Ok(SelectionRange::range(
                r.anchor,
                r.head,
                r.goal_column,
                r.bidi_level,
                r.assoc,
            )),
        })
        .collect::<Result<_, _>>()?;
    // Not merged, as `EditorSelection.fromJSON`: CodeMirror's own selections
    // are already normalised, and a restored history's may not be.
    Selection::verbatim(ranges, s.main).map_err(|e| e.to_string())
}

fn selection_to(sel: &Selection) -> SelectionJson {
    SelectionJson {
        ranges: sel
            .ranges()
            .iter()
            .map(|r| RangeJson {
                anchor: r.anchor(),
                head: r.head(),
                goal_column: None,
                bidi_level: None,
                assoc: 0,
                from: None,
                to: None,
            })
            .collect(),
        main: sel.main_index(),
    }
}

fn event_from(e: &EventJson) -> Result<HistoryEvent, String> {
    Ok(HistoryEvent {
        changes: e.changes.as_ref().map(set_from_value).transpose()?,
        mapped: e.mapped.as_deref().map(desc_from_flat).transpose()?,
        start_selection: e.start_selection.as_ref().map(selection_from).transpose()?,
        selections_after: e
            .selections_after
            .iter()
            .map(selection_from)
            .collect::<Result<_, _>>()?,
    })
}

fn event_to(e: &HistoryEvent) -> EventJson {
    EventJson {
        changes: e.changes.as_ref().map(set_to_value),
        mapped: e.mapped.as_ref().map(|m| {
            m.sections()
                .flat_map(|(len, ins)| [len as i64, ins as i64])
                .collect()
        }),
        start_selection: e.start_selection.as_ref().map(selection_to),
        selections_after: e.selections_after.iter().map(selection_to).collect(),
    }
}

fn set_to_value(set: &ChangeSet) -> Value {
    Value::Array(
        set.parts()
            .into_iter()
            .map(|part| match part {
                Part::Keep(len) => Value::from(len),
                Part::Replace(len, lines) => {
                    let mut out = vec![Value::from(len)];
                    out.extend(lines.into_iter().map(Value::String));
                    Value::Array(out)
                }
            })
            .collect(),
    )
}

pub fn change_set(json: &str) -> Result<ChangeSet, String> {
    set_from_value(&parse::<Value>(json, "ChangeSet")?)
}

pub fn change_set_json(set: &ChangeSet) -> String {
    set_to_value(set).to_string()
}

pub fn selection(json: &str) -> Result<Selection, String> {
    selection_from(&parse(json, "EditorSelection")?)
}

pub fn selection_json(sel: &Selection) -> String {
    serde_json::to_string(&selection_to(sel)).expect("plain data serialises")
}

/// `historyField.fromJSON` with the default `history()` config, resumed at
/// `prevTime`/`prevUserEvent` when given. Not checked against a document;
/// see `History::check`.
pub fn history(json: &str) -> Result<History, String> {
    let h: HistoryJson = parse(json, "history")?;
    let branch =
        |events: &[EventJson]| events.iter().map(event_from).collect::<Result<Vec<_>, _>>();
    let restored = History::from_events(
        HistoryConfig::default(),
        branch(&h.done)?,
        branch(&h.undone)?,
    );
    if h.prev_time.is_none() && h.prev_user_event.is_none() {
        return Ok(restored);
    }
    let time = millis(h.prev_time.unwrap_or(0.0))?;
    Ok(restored.with_previous(time, h.prev_user_event.as_deref()))
}

/// A JavaScript time in milliseconds, which must be whole (as `Date.now()`)
/// so that the history groups edits exactly as CodeMirror does.
pub fn millis(time: f64) -> Result<i64, String> {
    if time.is_finite() && time.fract() == 0.0 {
        Ok(time as i64)
    } else {
        Err(format!("time must be whole milliseconds, not {time}"))
    }
}

pub fn history_json(h: &History) -> String {
    let out = HistoryJson {
        done: h.done().map(event_to).collect(),
        undone: h.undone().map(event_to).collect(),
        prev_time: None,
        prev_user_event: None,
    };
    serde_json::to_string(&out).expect("plain data serialises")
}

/// `isolateHistory`'s values.
pub fn isolate(name: &str) -> Result<Isolate, String> {
    match name {
        "before" => Ok(Isolate::Before),
        "after" => Ok(Isolate::After),
        "full" => Ok(Isolate::Full),
        _ => Err(format!(
            "isolate must be before, after or full, not {name:?}"
        )),
    }
}
