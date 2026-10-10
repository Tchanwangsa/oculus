//! A field's slots, for JS: `FieldSlot[]`'s JSON, by slot id.

use oculus_math_edit::{Bounds, Field, SlotKind, utf16::Units};
use serde::Serialize;

#[derive(Serialize)]
struct Json {
    kind: &'static str,
    /// A row's number, or a cell's row.
    #[serde(skip_serializing_if = "Option::is_none")]
    row: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    col: Option<usize>,
    bounds: &'static str,
    text: bool,
    from: u32,
    to: u32,
    parent: Option<usize>,
}

const fn kind(kind: SlotKind) -> (&'static str, Option<usize>, Option<usize>) {
    match kind {
        SlotKind::Row(row) => ("row", Some(row), None),
        SlotKind::Cell { row, col } => ("cell", Some(row), Some(col)),
        SlotKind::Group => ("group", None, None),
        SlotKind::Sup => ("sup", None, None),
        SlotKind::Sub => ("sub", None, None),
        SlotKind::Numer => ("numer", None, None),
        SlotKind::Denom => ("denom", None, None),
        SlotKind::Radicand => ("radicand", None, None),
        SlotKind::Index => ("index", None, None),
        SlotKind::Body => ("body", None, None),
        SlotKind::Above => ("above", None, None),
        SlotKind::Below => ("below", None, None),
        SlotKind::LeftRight => ("leftRight", None, None),
        SlotKind::Text => ("text", None, None),
        SlotKind::Math => ("math", None, None),
        SlotKind::Tag => ("tag", None, None),
    }
}

const fn bounds(bounds: Bounds) -> &'static str {
    match bounds {
        Bounds::Delimited => "delimited",
        Bounds::Bare => "bare",
        Bounds::Open => "open",
    }
}

/// Every slot of `field`, by slot id, as JSON.
#[must_use]
pub fn slots(field: &Field) -> String {
    let units = Units::new(field.source());
    let json: Vec<Json> = field
        .stops()
        .slots()
        .iter()
        .map(|slot| {
            let (kind, row, col) = kind(slot.kind);
            Json {
                kind,
                row,
                col,
                bounds: bounds(slot.bounds),
                text: slot.text,
                from: units.of(slot.interior.start),
                to: units.of(slot.interior.end),
                parent: slot.parent.map(|id| id.0),
            }
        })
        .collect();
    serde_json::to_string(&json).unwrap_or_default()
}
