//! Content transactions, ported from `app/src/lib/canvas/collaboration.ts`:
//! `{edits:[{id,before,after,afterId?}], order?, background?, paper?}`. Preconditions
//! compare only the touched objects, so independent edits from the same old
//! revision merge; an edit already in place is skipped, and a failure applies
//! nothing. An addition's `afterId` places it above that element (`null`: at
//! the bottom); without one, or when that element is gone, it goes on top.
//! `order`, `background` and `paper` count as absent when JavaScript would
//! treat them as falsy, which the shared fixtures pin down. A `paper` edit
//! (`{before, after}`, whole papers) conflicts under the background's code:
//! the page setup is the canvas's background layer.

use super::error::{AuthorityError, Code};
use super::model;
use serde_json::{json, Map, Value};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

const MAX_EDITS: usize = 10_000;

/// Value equality as `sameValue` defines it: numbers by value (`1` == `1.0`),
/// object keys in any order, arrays in order.
pub fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| same_value(a, b))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(key, a)| y.get(key).is_some_and(|b| same_value(a, b)))
        }
        _ => a == b,
    }
}

/// JavaScript truthiness of a JSON value.
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|n| n != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// An `{id, before, after, afterId?}` edit; `json!` would re-serialize
/// embedded images.
fn edit_value(id: &str, before: &Value, after: &Value, after_id: Option<&Value>) -> Value {
    let mut edit = Map::new();
    edit.insert("id".into(), Value::String(id.into()));
    edit.insert("before".into(), before.clone());
    edit.insert("after".into(), after.clone());
    if let Some(after_id) = after_id {
        edit.insert("afterId".into(), after_id.clone());
    }
    Value::Object(edit)
}

fn invalid(message: &str) -> AuthorityError {
    AuthorityError::invalid(message)
}

/// `validateTransaction`: shape, unique edit IDs, valid elements, a layer
/// order that is a permutation of its own IDs, string backgrounds.
pub fn validate(value: &Value) -> Result<(), AuthorityError> {
    let Some(tx) = value.as_object() else {
        return Err(invalid("Invalid canvas transaction."));
    };
    let edits = tx.get("edits").and_then(Value::as_array);
    if tx
        .keys()
        .any(|k| !matches!(k.as_str(), "edits" | "order" | "background" | "paper"))
        || edits.is_none_or(|e| e.len() > MAX_EDITS)
    {
        return Err(invalid("Invalid canvas transaction fields."));
    }
    let mut ids = HashSet::new();
    for edit in edits.into_iter().flatten() {
        let id = edit
            .as_object()
            .and_then(|e| e.get("id"))
            .and_then(Value::as_str);
        let Some(id) = id.filter(|id| !id.is_empty() && ids.insert(*id)) else {
            return Err(invalid("Invalid element edit."));
        };
        let (before, after) = (edit.get("before"), edit.get("after"));
        if before.is_some_and(Value::is_null) && after.is_some_and(Value::is_null) {
            return Err(invalid("Invalid element edit."));
        }
        let placement = edit.get("afterId").map(|anchor| match anchor {
            Value::Null => true,
            Value::String(anchor) => !anchor.is_empty() && anchor != id,
            _ => false,
        });
        if placement.is_some_and(|ok| !ok || !before.is_some_and(Value::is_null)) {
            return Err(invalid("Invalid element placement."));
        }
        for element in [before, after] {
            if element.is_some_and(Value::is_null) {
                continue;
            }
            if element.and_then(|e| e.get("id")).and_then(Value::as_str) != Some(id) {
                return Err(invalid("Element edit IDs do not match."));
            }
            // The reference wraps the element in an otherwise valid document.
            model::validate_element(element.unwrap_or(&Value::Null), 0, &mut HashSet::new())
                .map_err(AuthorityError::invalid)?;
        }
    }
    if truthy(tx.get("order")) {
        let order = tx.get("order").and_then(Value::as_object);
        let side = |key| order.and_then(|o| o.get(key)).and_then(Value::as_array);
        let ok = match (side("before"), side("after")) {
            (Some(before), Some(after)) if before.len() == after.len() => {
                let before: Option<Vec<&str>> = before
                    .iter()
                    .map(|id| id.as_str().filter(|id| !id.is_empty()))
                    .collect();
                before.is_some_and(|before| {
                    let set: HashSet<&str> = before.iter().copied().collect();
                    let mut seen = HashSet::new();
                    set.len() == before.len()
                        && after.iter().all(|id| {
                            id.as_str()
                                .is_some_and(|id| set.contains(id) && seen.insert(id))
                        })
                })
            }
            _ => false,
        };
        if !ok {
            return Err(invalid("Invalid layer order."));
        }
    }
    if truthy(tx.get("background")) {
        let background = tx.get("background");
        let side = |key| {
            background
                .and_then(|b| b.get(key))
                .is_some_and(Value::is_string)
        };
        if !side("before") || !side("after") {
            return Err(invalid("Invalid background edit."));
        }
    }
    if truthy(tx.get("paper")) {
        let paper = tx.get("paper").and_then(Value::as_object);
        let side = |key| {
            paper
                .and_then(|p| p.get(key))
                .is_some_and(|p| model::validate_paper(p).is_ok())
        };
        if !side("before") || !side("after") {
            return Err(invalid("Invalid paper edit."));
        }
    }
    Ok(())
}

/// The document after `transaction`, and the part of it that changed
/// something (skipped edits, an order already in place and an unchanged
/// background are left out). Callers pass a valid document.
pub fn apply(document: &Value, transaction: &Value) -> Result<(Value, Value), AuthorityError> {
    validate(transaction)?;
    let edits: &[Value] = transaction["edits"].as_array().map_or(&[], Vec::as_slice);
    let order = Some(&transaction["order"]).filter(|o| truthy(Some(o)));
    let background = Some(&transaction["background"]).filter(|b| truthy(Some(b)));
    let paper = Some(&transaction["paper"]).filter(|p| truthy(Some(p)));
    let mut effective = Map::new();
    if edits.is_empty() && order.is_none() && background.is_none() && paper.is_none() {
        effective.insert("edits".into(), Value::Array(Vec::new()));
        return Ok((document.clone(), Value::Object(effective)));
    }

    // IDs in document order plus the current element for each; untouched
    // elements stay borrowed until the result is assembled.
    let mut order_ids: Vec<&str> = Vec::new();
    let mut elements: HashMap<&str, Cow<Value>> = HashMap::new();
    for element in document["elements"].as_array().into_iter().flatten() {
        let id = element["id"].as_str().unwrap_or_default();
        order_ids.push(id);
        elements.insert(id, Cow::Borrowed(element));
    }
    let mut applied = Vec::new();
    for edit in edits {
        let id = edit["id"].as_str().unwrap_or_default();
        let current = elements.get(id).map_or(&Value::Null, |v| v.as_ref());
        if same_value(current, &edit["after"]) {
            continue;
        }
        if !same_value(current, &edit["before"]) {
            return Err(AuthorityError::new(
                Code::ConflictShape,
                "A collaborator changed the same shape. Your draft is kept; download it or use the shared canvas.",
            ));
        }
        if edit["after"].is_null() {
            elements.remove(id);
            order_ids.retain(|kept| *kept != id);
        } else {
            if !elements.contains_key(id) {
                let at = match edit.get("afterId") {
                    Some(Value::Null) => 0,
                    Some(Value::String(anchor)) if elements.contains_key(anchor.as_str()) => {
                        order_ids
                            .iter()
                            .position(|kept| kept == anchor)
                            .map_or(order_ids.len(), |i| i + 1)
                    }
                    _ => order_ids.len(),
                };
                order_ids.insert(at, id);
            }
            elements.insert(id, Cow::Owned(edit["after"].clone()));
        }
        applied.push(edit_value(
            id,
            &edit["before"],
            &edit["after"],
            edit.get("afterId"),
        ));
    }
    effective.insert("edits".into(), Value::Array(applied));

    let mut ordered: Vec<&str> = order_ids;
    if let Some(order) = order {
        let ids = |key: &str| -> Vec<&str> {
            order[key]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect()
        };
        let (before, after) = (ids("before"), ids("after"));
        let affected: HashSet<&str> = before.iter().copied().collect();
        let current: Vec<&str> = ordered
            .iter()
            .copied()
            .filter(|id| affected.contains(id))
            .collect();
        let wanted: Vec<&str> = after
            .iter()
            .copied()
            .filter(|id| elements.contains_key(id))
            .collect();
        if current != wanted {
            let based: Vec<&str> = before
                .iter()
                .copied()
                .filter(|id| elements.contains_key(id))
                .collect();
            if current != based {
                return Err(AuthorityError::new(
                    Code::ConflictOrder,
                    "A collaborator changed the same layer order. Your draft is kept.",
                ));
            }
            let mut next = wanted.into_iter();
            ordered = ordered
                .iter()
                .map(|&id| {
                    if affected.contains(id) {
                        next.next().unwrap_or(id)
                    } else {
                        id
                    }
                })
                .collect();
            effective.insert("order".into(), order.clone());
        }
    }

    let mut result: Map<String, Value> = document
        .as_object()
        .into_iter()
        .flatten()
        .map(|(key, value)| {
            let value = if key == "elements" {
                Value::Null
            } else {
                value.clone()
            };
            (key.clone(), value)
        })
        .collect();
    if let Some(background) = background {
        let current = document["background"].as_str().unwrap_or_default();
        let (before, after) = (&background["before"], &background["after"]);
        if before.as_str() != Some(current) && after.as_str() != Some(current) {
            return Err(AuthorityError::new(
                Code::ConflictBackground,
                "A collaborator changed the background. Your draft is kept.",
            ));
        }
        if after.as_str() != Some(current) {
            effective.insert("background".into(), background.clone());
        }
        result.insert("background".into(), after.clone());
    }
    if let Some(paper) = paper {
        let current = &document["paper"];
        let (before, after) = (&paper["before"], &paper["after"]);
        if !same_value(current, before) && !same_value(current, after) {
            return Err(AuthorityError::new(
                Code::ConflictBackground,
                "A collaborator changed the pages. Your draft is kept.",
            ));
        }
        if !same_value(current, after) {
            effective.insert("paper".into(), paper.clone());
        }
        result.insert("paper".into(), after.clone());
    }
    let elements: Vec<Value> = ordered
        .into_iter()
        .filter_map(|id| elements.remove(id))
        .map(Cow::into_owned)
        .collect();
    result.insert("elements".into(), Value::Array(elements));
    let result = Value::Object(result);
    model::validate(&result).map_err(AuthorityError::invalid)?;
    Ok((result, Value::Object(effective)))
}

/// Whether a transaction (as `apply` returns it) changes anything.
pub fn has_changes(transaction: &Value) -> bool {
    transaction["edits"]
        .as_array()
        .is_some_and(|e| !e.is_empty())
        || truthy(transaction.get("order"))
        || truthy(transaction.get("background"))
        || truthy(transaction.get("paper"))
}

/// `diffCanvasContent`: the transaction from `before` to `after`, viewport excluded.
pub fn diff(before: &Value, after: &Value) -> Value {
    let index = |doc: &Value| -> Map<String, Value> {
        doc["elements"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|el| {
                (
                    el["id"].as_str().unwrap_or_default().to_string(),
                    el.clone(),
                )
            })
            .collect()
    };
    let (old, next) = (index(before), index(after));
    let mut edits = Vec::new();
    for (id, element) in &old {
        let now = next.get(id).unwrap_or(&Value::Null);
        if !same_value(element, now) {
            edits.push(edit_value(id, element, now, None));
        }
    }
    // Mid-stack additions name the element below them; top additions append.
    let ids: Vec<&String> = next.keys().collect();
    for (index, (id, element)) in next.iter().enumerate() {
        if old.contains_key(id) {
            continue;
        }
        let mid_stack = ids[index + 1..]
            .iter()
            .any(|later| old.contains_key(*later));
        let below = mid_stack.then(|| index.checked_sub(1).map_or(Value::Null, |i| json!(ids[i])));
        edits.push(edit_value(id, &Value::Null, element, below.as_ref()));
    }
    let old_order: Vec<&String> = old.keys().filter(|id| next.contains_key(*id)).collect();
    let new_order: Vec<&String> = next.keys().filter(|id| old.contains_key(*id)).collect();
    let mut transaction = Map::new();
    transaction.insert("edits".into(), Value::Array(edits));
    if old_order != new_order {
        transaction.insert(
            "order".into(),
            json!({"before": old_order, "after": new_order}),
        );
    }
    if before["background"] != after["background"] {
        transaction.insert(
            "background".into(),
            json!({"before": before["background"], "after": after["background"]}),
        );
    }
    if let (Some(old), Some(new)) = (
        before.get("paper").filter(|p| p.is_object()),
        after.get("paper").filter(|p| p.is_object()),
    ) {
        if !same_value(old, new) {
            transaction.insert("paper".into(), json!({"before": old, "after": new}));
        }
    }
    Value::Object(transaction)
}
