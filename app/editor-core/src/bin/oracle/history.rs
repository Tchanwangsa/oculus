//! `history`: a state with an undo history driven through a sequence of
//! transactions and undo/redo commands, answered after every step.
//! `oracle/history.ts` runs the same steps on an `EditorState` with
//! `history()`.

use oculus_editor_core::text::{
    History, HistoryEvent, Isolate, Popped, Selection, State, Text, Transaction,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::changes::{
    SelectionSpec, Spec, change_set, desc_json, selection, selection_json, set_json,
};
use crate::text::fnv1a;

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum IsolateSpec {
    Before,
    After,
    Full,
}

impl From<IsolateSpec> for Isolate {
    fn from(i: IsolateSpec) -> Self {
        match i {
            IsolateSpec::Before => Isolate::Before,
            IsolateSpec::After => Isolate::After,
            IsolateSpec::Full => Isolate::Full,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Action {
    Tr {
        changes: Vec<Spec>,
        #[serde(default)]
        selection: Option<SelectionSpec>,
        #[serde(default)]
        user_event: Option<String>,
        #[serde(default = "yes")]
        add_to_history: bool,
        #[serde(default)]
        isolate: Option<IsolateSpec>,
    },
    /// `state.replaceSelection(text)` with optional annotations.
    ReplaceSelection {
        text: String,
        #[serde(default)]
        user_event: Option<String>,
        #[serde(default)]
        isolate: Option<IsolateSpec>,
    },
    Undo,
    Redo,
    UndoSelection,
    RedoSelection,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
pub struct Step {
    #[serde(flatten)]
    action: Action,
    time: i64,
    /// Answer the whole document and history, not their digests.
    #[serde(default)]
    full: bool,
}

#[derive(Deserialize)]
pub struct Request {
    doc: String,
    selection: SelectionSpec,
    steps: Vec<Step>,
}

fn event_json(e: &HistoryEvent) -> Value {
    let mut out = Map::new();
    if let Some(changes) = &e.changes {
        out.insert("changes".into(), set_json(changes));
    }
    if let Some(mapped) = &e.mapped {
        out.insert("mapped".into(), desc_json(mapped));
    }
    if let Some(start) = &e.start_selection {
        out.insert("start".into(), selection_json(start));
    }
    out.insert(
        "after".into(),
        e.selections_after.iter().map(selection_json).collect(),
    );
    Value::Object(out)
}

fn history_json(h: &History) -> Value {
    let mut out = json!({
        "done": h.done().map(event_json).collect::<Vec<_>>(),
        "undone": h.undone().map(event_json).collect::<Vec<_>>(),
        "prev_time": h.prev_time(),
    });
    if let Some(event) = h.prev_user_event() {
        out["prev_user_event"] = json!(event);
    }
    out
}

fn transaction(
    mut tr: Transaction,
    user_event: Option<String>,
    isolate: Option<IsolateSpec>,
) -> Transaction {
    tr.set_user_event(user_event.as_deref());
    tr.isolate = isolate.map(Isolate::from);
    tr
}

/// `fnv1a` of a canonical form of `v`: sorted keys,
/// length-prefixed strings, numbers as JavaScript prints them.
/// `oracle/history.ts` digests its history the same way.
fn digest(v: &Value) -> u32 {
    fn walk(v: &Value, out: &mut String) {
        match v {
            Value::Null => out.push('n'),
            Value::Bool(b) => out.push(if *b { 't' } else { 'f' }),
            Value::Number(n) => match n.as_f64() {
                Some(f) if n.is_f64() => out.push_str(&format!("{f}")),
                _ => out.push_str(&n.to_string()),
            },
            Value::String(s) => {
                out.push_str(&format!("s{}:", s.encode_utf16().count()));
                out.push_str(s);
            }
            Value::Array(items) => {
                out.push('[');
                for item in items {
                    walk(item, out);
                    out.push(',');
                }
                out.push(']');
            }
            Value::Object(map) => {
                out.push('{');
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                for k in keys {
                    out.push_str(k);
                    out.push(':');
                    walk(&map[k], out);
                    out.push(',');
                }
                out.push('}');
            }
        }
    }
    let mut out = String::new();
    walk(v, &mut out);
    fnv1a(&out)
}

type Next = Option<(Transaction, Option<History>)>;

fn popped(p: Popped) -> Result<Next, String> {
    Ok(p.map_err(|e| e.to_string())?.map(|(t, h)| (t, Some(h))))
}

pub fn answer(r: Request) -> Result<Value, String> {
    let doc = Text::of(&r.doc);
    let sel: Selection = selection(&r.selection)?;
    let mut state = State::with_selection(doc, sel).map_err(|e| e.to_string())?;
    let mut history = History::default();
    let mut answers = Vec::with_capacity(r.steps.len());
    for step in r.steps {
        let time = step.time;
        let mut ran = true;
        let popped = match step.action {
            Action::Tr {
                changes,
                selection: sel,
                user_event,
                add_to_history,
                isolate,
            } => {
                let changes = change_set(&changes, state.doc.len())?;
                let mut tr = transaction(Transaction::new(changes, time), user_event, isolate);
                tr.add_to_history = add_to_history;
                if let Some(sel) = sel {
                    tr.selection = Some(selection(&sel)?);
                }
                Some((tr, None))
            }
            Action::ReplaceSelection {
                text,
                user_event,
                isolate,
            } => {
                let (changes, sel) = state.replace_selection(&text).map_err(|e| e.to_string())?;
                let tr = transaction(
                    Transaction::new(changes, time).with_selection(sel),
                    user_event,
                    isolate,
                );
                Some((tr, None))
            }
            Action::Undo => popped(history.undo(&state, time))?,
            Action::Redo => popped(history.redo(&state, time))?,
            Action::UndoSelection => popped(history.undo_selection(&state, time))?,
            Action::RedoSelection => popped(history.redo_selection(&state, time))?,
        };
        match popped {
            Some((tr, next)) => {
                history = match next {
                    Some(next) => next,
                    None => history.update(&state, &tr).map_err(|e| e.to_string())?,
                };
                state = state.apply(&tr).map_err(|e| e.to_string())?;
            }
            None => ran = false,
        }
        let mut a = json!({
            "ran": ran,
            "sel": selection_json(&state.selection),
            "undo_depth": history.undo_depth(),
            "redo_depth": history.redo_depth(),
            "prev_time": history.prev_time(),
        });
        let hist = history_json(&history);
        if step.full {
            a["doc"] = json!(state.doc.to_string());
            a["hist"] = hist;
        } else {
            a["doc_fnv"] = json!(fnv1a(&state.doc.to_string()));
            a["hist_digest"] = json!(digest(&hist));
        }
        answers.push(a);
    }
    Ok(json!({ "steps": answers }))
}
