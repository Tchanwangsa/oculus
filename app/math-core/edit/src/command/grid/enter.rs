//! Enter in an array cell (`aligned`, `cases`, a matrix): a new row.

use super::{find::read_array, write::cell_path};
use crate::{
    command::{Target, finish, splice::splice},
    field::{Field, Outcome, Selection},
    slot::{SlotId, SlotKind},
};

/// A new row of empty cells after `cell`'s row, as wide as that row, the
/// caret in its first cell; the caret just goes to the next row when that
/// one is empty, and an empty row adds nothing. KaTeX drops a last row
/// of one empty cell, so a one-column array gets no new last row.
pub fn array_row(field: &Field, cell: SlotId) -> Outcome {
    let stops = field.stops();
    let (Some((slot, atom)), SlotKind::Cell { row, .. }) =
        (stops.owner(cell), stops.slot(cell).kind)
    else {
        return Outcome::none(field);
    };
    let Some(array) = read_array(field, slot, atom) else {
        return Outcome::none(field);
    };
    let empty = |cells: &[SlotId]| cells.iter().all(|&c| stops.slot(c).is_empty());
    let current = &array.cells[row];
    if empty(current) {
        return Outcome::none(field);
    }
    if let Some(next) = array.cells.get(row + 1)
        && empty(next)
    {
        let caret = stops.first_stop(next[0]);
        return Outcome::moved(field.clone().select(Selection::caret(caret)));
    }
    let width = current.len();
    if width == 1 && row + 1 == array.cells.len() {
        return Outcome::none(field);
    }
    let mut model = array.model;
    model.insert_row(row, width);
    let written = model.write();
    let spliced = splice(field, slot, array.content, &written.text);
    let target = Target::Slot {
        path: cell_path(&stops.slot(slot).path, atom, model.index(row + 1, 0)),
        offset: spliced.at + written.cells[row + 1][0].start,
    };
    let outcome = finish(field, spliced.source, &target);
    let new = outcome.field.stops();
    let landed = new.stop(outcome.field.selection().head).slot;
    if outcome.changes.is_empty()
        || !matches!(new.slot(landed).kind, SlotKind::Cell { row: r, col: 0 } if r == row + 1)
    {
        return Outcome::none(field);
    }
    outcome
}
