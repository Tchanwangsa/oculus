//! Starting alignment in a plain display row, including an unwrapped row.

use super::{
    super::{Target, diff, finish, grid::read_array, splice::splice},
    merge,
};
use crate::{
    field::{Field, Outcome},
    slot::SlotKind,
};

pub fn insert(field: &Field) -> Option<Outcome> {
    if !field.display() || !field.selection().is_caret() {
        return None;
    }
    let stops = field.stops();
    let head = field.selection().head;
    let slot = stops.stop(head).slot;
    let s = stops.slot(slot);
    match s.kind {
        SlotKind::Row(_) => {}
        SlotKind::Cell { row, .. } => {
            let (parent, atom) = stops.owner(slot)?;
            let array = read_array(field, parent, atom)?;
            if array.name != "gathered" || array.cells.get(row)?.len() != 1 {
                return None;
            }
        }
        _ => return None,
    }
    let source = field.source();
    let offset = stops.offset(head);
    let before = &source[s.interior.start..offset];
    let after = &source[offset..s.interior.end];
    let open = r"\begin{aligned}";
    let text = format!("{open}{before}&{after}\\end{{aligned}}");
    let spliced = splice(field, slot, s.interior.clone(), &text);
    let mut outcome = finish(
        field,
        spliced.source,
        &Target::Hole(spliced.at + open.len() + before.len() + 1),
    );
    if outcome.changes.is_empty() {
        return None;
    }
    outcome.field = merge::adjacent(outcome.field);
    outcome.changes = vec![diff(field.source(), outcome.field.source())];
    outcome.isolate = true;
    Some(outcome)
}
