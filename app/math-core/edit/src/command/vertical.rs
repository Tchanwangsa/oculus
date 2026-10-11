//! ↑ and ↓ between stacked slots. The model does not measure: the view
//! passes each stop's rendered x, and the caret goes to the stop of the
//! slot above or below whose x is nearest its own.

use crate::{
    field::{Direction, Effect, Field, Outcome, Selection},
    slot::{SlotId, SlotKind, StopId},
    stops::Stops,
};

/// ↑ (`down` false) or ↓. From the caret's slot, the slot stacked above
/// or below it in the same structure (numerator and denominator, a
/// script atom's sup and sub, a root's index and radicand, `\overset`'s
/// parts, an array's rows in the same column, a block's rows); without
/// one, the structure's own slot is tried, and so on out to the row,
/// past which the caret leaves the field.
pub fn vertical(field: &Field, down: bool, xs: &[f64]) -> Outcome {
    let stops = field.stops();
    let head = field.selection().head;
    let x = xs.get(head.0).copied().filter(|x| x.is_finite());
    let mut slot = stops.stop(head).slot;
    loop {
        if let Some(target) = stacked(stops, slot, down) {
            let caret = nearest(stops, target, x, xs);
            return Outcome::moved(field.clone().select(Selection::caret(caret)));
        }
        let Some(parent) = stops.parent(slot) else {
            let direction = if down { Direction::Down } else { Direction::Up };
            return Outcome::effect(field, Effect::Leave(direction));
        };
        slot = parent;
    }
}

/// The stop of `slot` nearest `x`; its first when nothing is measured.
fn nearest(stops: &Stops, slot: SlotId, x: Option<f64>, xs: &[f64]) -> StopId {
    let ids = &stops.slot(slot).stops;
    let Some(x) = x else {
        return ids[0];
    };
    ids.iter()
        .copied()
        .filter_map(|id| {
            let at = xs.get(id.0).copied().filter(|at| at.is_finite())?;
            Some((id, (at - x).abs()))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(ids[0], |(id, _)| id)
}

/// The slot above (`down` false) or below `slot` in its structure.
fn stacked(stops: &Stops, slot: SlotId, down: bool) -> Option<SlotId> {
    let kind = stops.slot(slot).kind;
    if let SlotKind::Row(n) = kind {
        let m = if down { n + 1 } else { n.checked_sub(1)? };
        return stops
            .slots_with_ids()
            .find(|(_, s)| s.kind == SlotKind::Row(m))
            .map(|(id, _)| id);
    }
    let (parent, atom) = stops.owner(slot)?;
    let siblings = stops.atom_slots(parent, atom);
    if let SlotKind::Cell { row, col } = kind {
        let target = if down { row + 1 } else { row.checked_sub(1)? };
        // The same column, or the row's last cell when it is shorter.
        return siblings
            .into_iter()
            .filter(|&s| matches!(stops.slot(s).kind, SlotKind::Cell { row: r, col: c } if r == target && c <= col))
            .max_by_key(|&s| match stops.slot(s).kind {
                SlotKind::Cell { col: c, .. } => c,
                _ => 0,
            });
    }
    let level = level(kind)?;
    let candidates = siblings
        .into_iter()
        .filter_map(|s| Some((s, level_of(stops, s)?)));
    if down {
        candidates
            .filter(|&(_, l)| l > level)
            .min_by_key(|&(_, l)| l)
            .map(|(s, _)| s)
    } else {
        candidates
            .filter(|&(_, l)| l < level)
            .max_by_key(|&(_, l)| l)
            .map(|(s, _)| s)
    }
}

fn level_of(stops: &Stops, slot: SlotId) -> Option<u8> {
    level(stops.slot(slot).kind)
}

/// Where a slot sits in its structure, top to bottom.
const fn level(kind: SlotKind) -> Option<u8> {
    match kind {
        SlotKind::Above | SlotKind::Index | SlotKind::Sup | SlotKind::Numer => Some(0),
        SlotKind::Body | SlotKind::Radicand => Some(1),
        SlotKind::Denom | SlotKind::Sub | SlotKind::Below => Some(2),
        _ => None,
    }
}
