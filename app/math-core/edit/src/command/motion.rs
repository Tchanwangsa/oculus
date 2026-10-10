//! Moving the caret and extending the selection along the stops: ←/→,
//! Home/End, select all, Tab, Esc.

use super::{select::widen, template::place};
use crate::{
    field::{Direction, Effect, Field, Outcome, Selection},
    slot::StopId,
};

/// ← (`right` false) or → by one stop. Past the first or last stop the
/// caret leaves the field. A selection collapses to its left or right
/// end; with `extend` the head moves and the selection widens to whole
/// structures.
pub fn horizontal(field: &Field, right: bool, extend: bool) -> Outcome {
    let stops = field.stops();
    let selection = field.selection();
    if !extend && !selection.is_caret() {
        let (lo, hi) = ordered(selection);
        let caret = if right { hi } else { lo };
        return Outcome::moved(field.clone().select(Selection::caret(caret)));
    }
    let next = if right {
        stops.next(selection.head)
    } else {
        stops.prev(selection.head)
    };
    match next {
        Some(head) if extend => extend_to(field, head, right),
        Some(head) => Outcome::moved(field.clone().select(Selection::caret(head))),
        None if extend => Outcome::none(field),
        None => Outcome::effect(
            field,
            Effect::Leave(if right {
                Direction::Right
            } else {
                Direction::Left
            }),
        ),
    }
}

/// The selection with its head moved to `head` and widened, the head
/// going toward the end of the formula when `toward_end`.
fn extend_to(field: &Field, head: StopId, toward_end: bool) -> Outcome {
    let anchor = field.selection().anchor;
    let (anchor, head) = widen(field.stops(), anchor, head, Some(toward_end));
    let mut moved = field.clone();
    moved.set_selection(Selection { anchor, head });
    Outcome::moved(moved)
}

/// Home (`end` false) or End: the first or last stop of the caret's row
/// in display maths, of the whole field inline.
pub fn home_end(field: &Field, end: bool, extend: bool) -> Outcome {
    let stops = field.stops();
    let head = field.selection().head;
    let target = if field.display() {
        let row = stops.row_of(stops.stop(head).slot);
        if end {
            stops.last_stop(row)
        } else {
            stops.first_stop(row)
        }
    } else if end {
        StopId(stops.stops().len() - 1)
    } else {
        StopId(0)
    };
    if extend {
        extend_to(field, target, end)
    } else {
        Outcome::moved(field.clone().select(Selection::caret(target)))
    }
}

/// ⌘A: the whole field.
pub fn select_all(field: &Field) -> Outcome {
    let last = StopId(field.stops().stops().len() - 1);
    let mut all = field.clone();
    all.set_selection(Selection {
        anchor: StopId(0),
        head: last,
    });
    Outcome::moved(all)
}

/// Tab: out of a text run to just after it; else the next empty slot;
/// else a `\qquad` at the caret.
pub fn tab(field: &Field) -> Outcome {
    let stops = field.stops();
    let head = field.selection().head;
    let slot = stops.stop(head).slot;
    if stops.slot(slot).text
        && let Some((parent, atom)) = stops.owner(slot)
    {
        let after = stops.stop_after_atom(parent, atom);
        return Outcome::moved(field.clone().select(Selection::caret(after)));
    }
    if let Some(next) = stops.next_empty(head) {
        return Outcome::moved(field.clone().select(Selection::caret(next)));
    }
    let offset = stops.offset(head);
    place(field, slot, offset..offset, r"\qquad", "")
}

/// Shift+Tab: the previous empty slot, else nothing.
pub fn shift_tab(field: &Field) -> Outcome {
    let head = field.selection().head;
    field.stops().prev_empty(head).map_or_else(
        || Outcome::none(field),
        |prev| Outcome::moved(field.clone().select(Selection::caret(prev))),
    )
}

/// Esc (no command pending): leave the field forward.
pub fn escape(field: &Field) -> Outcome {
    Outcome::effect(field, Effect::Leave(Direction::Right))
}

/// The selection's ends in ←/→ order.
pub fn ordered(selection: Selection) -> (StopId, StopId) {
    (
        selection.anchor.min(selection.head),
        selection.anchor.max(selection.head),
    )
}
