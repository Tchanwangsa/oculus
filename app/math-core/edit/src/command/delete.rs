//! Backspace, Delete and ⌘Backspace, structure-aware as MathLive's are:
//! deleting into a structure selects it first, and a second press deletes
//! the selection.

use core::ops::Range;

use super::{
    Target, finish, grid,
    splice::splice,
    template::selection_slot,
    text::{cluster_after, cluster_before},
};
use crate::{
    field::{Effect, Field, Outcome, Selection},
    slot::{Bounds, SlotId, SlotKind},
};

/// Backspace.
pub fn backward(field: &Field) -> Outcome {
    erase(field, true)
}

/// Delete.
pub fn forward(field: &Field) -> Outcome {
    erase(field, false)
}

fn erase(field: &Field, back: bool) -> Outcome {
    if !field.selection().is_caret() {
        let (slot, range) = selection_slot(field);
        return remove(field, slot, range);
    }
    if back && field.source().trim().is_empty() {
        return Outcome::effect(field, Effect::RemoveMaths);
    }
    let stops = field.stops();
    let head = field.selection().head;
    let slot = stops.stop(head).slot;
    let offset = stops.offset(head);
    let at_edge = if back {
        stops.at_slot_start(head)
    } else {
        stops.at_slot_end(head)
    };
    if at_edge {
        return slot_edge(field, slot, back);
    }
    let text = stops.slot(slot).text;
    let interior = stops.slot(slot).interior.clone();
    if text && stops.atom_around(slot, offset).is_some() {
        return remove(field, slot, cluster(field, interior, offset, back));
    }
    let atom = if back {
        stops.atom_before(slot, offset)
    } else {
        stops.atom_after(slot, offset)
    };
    let Some(atom) = atom else {
        return Outcome::none(field);
    };
    let range = stops.slot(slot).atoms[atom].clone();
    if text && plain(&field.source()[range]) {
        return remove(field, slot, cluster(field, interior, offset, back));
    }
    if text && let Some(pair) = word_and_group(field, slot, atom, back) {
        return remove(field, slot, pair);
    }
    if stops.is_structure(slot, atom) {
        return select_atom(field, slot, atom, back);
    }
    // From the neighbouring stop, so the spaces before the atom go too.
    let range = if back {
        stops.offset(stops.stop_before_atom(slot, atom))..offset
    } else {
        offset..stops.offset(stops.stop_after_atom(slot, atom))
    };
    remove(field, slot, range)
}

/// The character next to `offset` in a text run, with its combining
/// marks (which may be atoms of their own), within the run's `interior`.
fn cluster(field: &Field, interior: Range<usize>, offset: usize, back: bool) -> Range<usize> {
    let src = field.source();
    if back {
        cluster_before(src, offset).max(interior.start)..offset
    } else {
        offset..cluster_after(src, offset).min(interior.end)
    }
}

/// A text atom with no LaTeX syntax in it (characters typed as
/// themselves).
fn plain(text: &str) -> bool {
    !text.contains(['\\', '{', '}', '$', '&', '#', '^', '_', '~', '%'])
}

/// A text-mode command written with `{}` after it (`\textasciicircum{}`),
/// which deletes as one character: the empty group and the control word
/// before it (Backspace) or the word and the group after it (Delete).
fn word_and_group(field: &Field, slot: SlotId, atom: usize, back: bool) -> Option<Range<usize>> {
    let stops = field.stops();
    let atoms = &stops.slot(slot).atoms;
    let src = field.source();
    let (word, group) = if back {
        (atoms.get(atom.checked_sub(1)?)?, &atoms[atom])
    } else {
        (&atoms[atom], atoms.get(atom + 1)?)
    };
    let group_index = if back { atom } else { atom + 1 };
    let is_word = word.end == group.start
        && src[word.clone()].starts_with('\\')
        && src[word.start + 1..word.end]
            .chars()
            .all(|c| c.is_ascii_alphabetic());
    (is_word && &src[group.clone()] == "{}" && stops.all_empty(slot, group_index))
        .then_some(word.start..group.end)
}

/// Backspace at a slot's start or Delete at its end. In a row: joins the
/// row before (or after). In a cell: Backspace in an empty one takes its
/// column or row when all empty (`grid`); else it steps to the cell
/// before (Delete: after).
/// An empty script goes alone, even beside another empty one (`\cos^{}`
/// → `\cos`, `x_{}^{}` → `x^{}`), the caret just after its base. Else
/// the structure the slot belongs to goes when all its slots are empty,
/// and is selected when they are not.
fn slot_edge(field: &Field, slot: SlotId, back: bool) -> Outcome {
    let stops = field.stops();
    let Some((parent, atom)) = stops.owner(slot) else {
        return join_rows(field, slot, back);
    };
    let s = stops.slot(slot);
    if let SlotKind::Cell { row, col } = s.kind {
        if back && let Some(outcome) = grid::backspace(field) {
            return outcome;
        }
        let head = field.selection().head;
        let first = row == 0 && col == 0;
        let last = stops.atom_slots(parent, atom).last() == Some(&slot);
        let step = if back && !first {
            stops.prev(head)
        } else if !back && !last {
            stops.next(head)
        } else {
            None
        };
        if let Some(step) = step {
            return Outcome::moved(field.clone().select(Selection::caret(step)));
        }
    }
    if s.is_empty()
        && matches!(s.kind, SlotKind::Sup | SlotKind::Sub)
        && let Some(range) = script_range(field, slot)
    {
        return remove(field, parent, range);
    }
    if stops.all_empty(parent, atom) {
        let range = stops.slot(parent).atoms[atom].clone();
        return remove(field, parent, range);
    }
    select_atom(field, parent, atom, back)
}

/// An empty script's source: its `^` or `_` through its `{}`.
fn script_range(field: &Field, slot: SlotId) -> Option<Range<usize>> {
    let s = field.stops().slot(slot);
    if s.bounds != Bounds::Delimited {
        return None;
    }
    let src = field.source();
    let open = s.interior.start.checked_sub(1)?;
    let before = src[..open].trim_end();
    before
        .ends_with(['^', '_'])
        .then(|| before.len() - 1..s.interior.end + 1)
}

/// Backspace at a top-level row's start joins it to the row before;
/// Delete at its end joins the row after. The first row's start and the
/// last row's end do nothing.
fn join_rows(field: &Field, row: SlotId, back: bool) -> Outcome {
    let stops = field.stops();
    let SlotKind::Row(n) = stops.slot(row).kind else {
        return Outcome::none(field);
    };
    let rows: Vec<SlotId> = stops
        .slots_with_ids()
        .filter(|(_, slot)| slot.parent.is_none())
        .map(|(id, _)| id)
        .collect();
    let pair = if back {
        n.checked_sub(1).map(|m| (rows[m], row))
    } else {
        rows.get(n + 1).map(|&next| (row, next))
    };
    let Some((upper, lower)) = pair else {
        return Outcome::none(field);
    };
    let range = stops.slot(upper).interior.end..stops.slot(lower).interior.start;
    remove(field, upper, range)
}

/// Selects atom `atom` of `slot`, the head on the side the key deletes
/// toward.
fn select_atom(field: &Field, slot: SlotId, atom: usize, back: bool) -> Outcome {
    let stops = field.stops();
    let before = stops.stop_before_atom(slot, atom);
    let after = stops.stop_after_atom(slot, atom);
    let (anchor, head) = if back {
        (after, before)
    } else {
        (before, after)
    };
    let mut selected = field.clone();
    selected.set_selection(Selection { anchor, head });
    Outcome::moved(selected)
}

/// `range` of `slot` deleted, the caret where it was.
fn remove(field: &Field, slot: SlotId, range: Range<usize>) -> Outcome {
    let spliced = splice(field, slot, range, "");
    let path = field.stops().slot(slot).path.clone();
    finish(
        field,
        spliced.source,
        &Target::Slot {
            path,
            offset: spliced.at,
        },
    )
}

/// ⌘Backspace: from the start of the caret's row (or array cell) to the
/// caret, taking whole the structure the caret is in; at the row's start
/// it is Backspace.
pub fn line(field: &Field) -> Outcome {
    if !field.selection().is_caret() || field.source().trim().is_empty() {
        return backward(field);
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
        return backward(field);
    };
    let line = chain[n];
    let end = if n == 0 {
        stops.offset(head)
    } else {
        stops.owner(chain[n - 1]).map_or_else(
            || stops.offset(head),
            |(slot, atom)| stops.slot(slot).atoms[atom].end,
        )
    };
    let start = stops.offset(stops.first_stop(line));
    if end <= start {
        return backward(field);
    }
    remove(field, line, start..end)
}
