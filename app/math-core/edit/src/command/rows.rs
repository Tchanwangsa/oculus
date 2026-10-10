//! Enter: a new row in display maths, never a second empty one.

use super::{
    Target, finish,
    grid::{array_row, top_break},
    splice::splice,
};
use crate::{
    field::{Direction, Effect, Field, Outcome, Selection},
    slot::{SlotKind, SlotPath},
};

/// Enter (or Shift+Enter). Inline maths: leave the field forward.
/// Display: in an array cell (`aligned`, `cases`, a matrix), an empty
/// row after the caret's row, the caret in its first cell; elsewhere the
/// caret's top-level row splits at the caret (inside a structure, just
/// after it), as a line break would, the new row on a line of its own
/// (`grid::top_break`). On an empty row nothing happens,
/// and from a row's end with an empty row after it the caret just goes
/// there. A caret beside an array that is its row's only atom counts as
/// in its first or last cell, so the row joins the array.
pub fn enter(field: &Field) -> Outcome {
    if !field.display() {
        return Outcome::effect(field, Effect::Leave(Direction::Right));
    }
    let stops = field.stops();
    let head = field.selection().head;
    let chain = stops.ancestors(stops.stop(head).slot);
    let Some(n) = chain.iter().position(|&slot| {
        matches!(
            stops.slot(slot).kind,
            SlotKind::Row(_) | SlotKind::Cell { .. }
        )
    }) else {
        return Outcome::none(field);
    };
    let line = chain[n];
    let SlotKind::Row(row) = stops.slot(line).kind else {
        return array_row(field, line);
    };
    let s = stops.slot(line);
    if s.is_empty() {
        return Outcome::none(field);
    }
    let offset = if n == 0 {
        stops.offset(head)
    } else {
        stops.owner(chain[n - 1]).map_or_else(
            || stops.offset(head),
            |(slot, atom)| stops.slot(slot).atoms[atom].end,
        )
    };
    let at_start = offset <= s.interior.start;
    let at_end = offset >= s.interior.end;
    if n == 0 && s.atoms.len() == 1 && (at_start || at_end) {
        let cells = stops.atom_slots(line, 0);
        let cell = if at_start {
            cells.first()
        } else {
            cells.last()
        };
        if let Some(&cell) = cell
            && matches!(stops.slot(cell).kind, SlotKind::Cell { .. })
        {
            return array_row(field, cell);
        }
    }
    let neighbour = |m: usize| {
        stops
            .slots_with_ids()
            .find(|(_, slot)| slot.kind == SlotKind::Row(m))
    };
    if at_end
        && let Some((next, slot)) = neighbour(row + 1)
        && slot.is_empty()
    {
        let caret = stops.first_stop(next);
        return Outcome::moved(field.clone().select(Selection::caret(caret)));
    }
    if at_start
        && let Some(m) = row.checked_sub(1)
        && neighbour(m).is_some_and(|(_, slot)| slot.is_empty())
    {
        return Outcome::none(field);
    }
    // The break takes the spaces after the caret: the new row starts its
    // line.
    let rest = &field.source()[offset..];
    let spaces = rest.len() - rest.trim_start().len();
    let spliced = splice(field, line, offset..offset + spaces, &top_break(field));
    let path = SlotPath {
        row: row + 1,
        steps: Vec::new(),
    };
    finish(
        field,
        spliced.source,
        &Target::Slot {
            path,
            offset: spliced.end,
        },
    )
}
