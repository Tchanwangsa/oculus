use std::collections::{BTreeMap, VecDeque};

use serde_json::Value;

/// Agent signatures carry proposed fields, not checkbox state. A replacement
/// keeps the student's explicit ticks on rows it can identify, while every
/// proposed field still comes from the new card.
pub(super) fn inherit(kind: &str, previous_kind: &str, next: &mut Value, previous: &Value) {
    if family(kind) != family(previous_kind) || family(kind).is_none() {
        return;
    }
    let keys = ["items", "tasks", "events", "changes"];
    let old_rows = keys
        .iter()
        .find_map(|key| previous.get(key).and_then(Value::as_array));
    let Some(old_rows) = old_rows else { return };
    for key in keys {
        if let Some(rows) = next.get_mut(key).and_then(Value::as_array_mut) {
            inherit_rows(rows, old_rows);
        }
    }
}

fn family(kind: &str) -> Option<&'static str> {
    match kind {
        "NewTasks" | "NewProject" | "TaskChanges" | "DeleteTasks" => Some("task"),
        "NewEvents" | "EventChanges" | "DeleteEvents" => Some("event"),
        "TagChanges" => Some("tag"),
        _ => None,
    }
}

fn identity(row: &Value) -> Option<String> {
    if row.is_number() || row.is_string() {
        return Some(format!("id:{row}"));
    }
    for key in ["id", "task", "tag"] {
        if let Some(id) = row.get(key).filter(|id| id.is_number() || id.is_string()) {
            return Some(format!("id:{id}"));
        }
    }
    for key in ["title", "name"] {
        if let Some(value) = row.get(key).and_then(Value::as_str) {
            return Some(format!("{key}:{value}"));
        }
    }
    None
}

fn inherit_rows(next: &mut [Value], previous: &[Value]) {
    let mut candidates: BTreeMap<String, VecDeque<&Value>> = BTreeMap::new();
    for row in previous {
        if let Some(key) = identity(row) {
            candidates.entry(key).or_default().push_back(row);
        }
    }
    for row in next {
        let previous = identity(row)
            .and_then(|key| candidates.get_mut(&key))
            .and_then(VecDeque::pop_front);
        let Some(previous) = previous else { continue };
        if let Some(included) = previous.get("included").and_then(Value::as_bool) {
            if let Some(object) = row.as_object_mut() {
                object.insert("included".into(), Value::Bool(included));
            }
        }
        if let (Some(next), Some(old)) = (
            row.get_mut("subtasks").and_then(Value::as_array_mut),
            previous.get("subtasks").and_then(Value::as_array),
        ) {
            inherit_rows(next, old);
        }
    }
}
