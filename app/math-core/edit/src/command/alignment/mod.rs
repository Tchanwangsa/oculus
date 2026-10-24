//! Removing alignment points and keeping only the rows that need them.

mod insert;
mod merge;
mod normalize;

pub(super) use insert::insert;

use core::ops::Range;

use super::{Target, diff, finish, grid::read_array, splice::splice};
use crate::{
    field::{Field, Outcome},
    slot::{SlotId, SlotKind},
};

/// Backspace just after, or Delete just before, an aligned cell separator.
/// Other environments keep their grid navigation and deletion rules.
pub fn boundary(field: &Field, cell: SlotId, back: bool) -> Option<Outcome> {
    let stops = field.stops();
    let SlotKind::Cell { row, col } = stops.slot(cell).kind else {
        return None;
    };
    let (parent, atom) = stops.owner(cell)?;
    let array = read_array(field, parent, atom)?;
    if array.name != "aligned" {
        return None;
    }
    let cells = array.cells.get(row)?;
    let left_col = if back { col.checked_sub(1)? } else { col };
    let left = *cells.get(left_col)?;
    let right = *cells.get(left_col + 1)?;
    let from = stops.slot(left).interior.end;
    let to = stops.slot(right).interior.start;
    let marker = separator(field.source(), from..to)?;
    let spliced = splice(field, parent, marker..marker + 1, "");
    let mut outcome = finish(
        field,
        spliced.source,
        &Target::Slot {
            path: stops.slot(left).path.clone(),
            offset: from,
        },
    );
    outcome.isolate = !outcome.changes.is_empty();
    Some(outcome)
}

/// The structural `&` between two parsed cells, ignoring comments and
/// escaped characters rather than mistaking a literal ampersand for it.
fn separator(source: &str, range: Range<usize>) -> Option<usize> {
    let mut chars = source[range.clone()].char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '%' => {
                for (_, c) in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '&' => return Some(range.start + i),
            _ => {}
        }
    }
    None
}

/// Clean up only an alignment whose own points were removed by this
/// edit. Opening imported source or editing its prose never tidies it.
pub fn normalize(before: &Field, mut outcome: Outcome) -> Outcome {
    if !outcome
        .changes
        .iter()
        .any(|change| before.source()[change.from..change.to].contains('&'))
    {
        return outcome;
    }
    loop {
        let after = &outcome.field;
        let change = diff(before.source(), after.source());
        let mut candidates = Vec::new();
        for (slot, s) in after.stops().slots_with_ids() {
            for (atom, range) in s.atoms.iter().enumerate() {
                if !after.source()[range.clone()].starts_with(r"\begin{aligned}") {
                    continue;
                }
                let Some(array) = read_array(after, slot, atom) else {
                    continue;
                };
                let affected = before.stops().slots_with_ids().any(|(old_slot, old)| {
                    old.atoms.iter().enumerate().any(|(old_atom, old_range)| {
                        let start = if old_range.start <= change.from {
                            old_range.start
                        } else if old_range.start >= change.to {
                            old_range.start - (change.to - change.from) + change.insert.len()
                        } else {
                            return false;
                        };
                        if start != range.start {
                            return false;
                        }
                        let Some(old_array) = read_array(before, old_slot, old_atom) else {
                            return false;
                        };
                        old_array.name == "aligned"
                            && old_array.cells.iter().any(|cells| {
                                cells.windows(2).any(|pair| {
                                    let [left, right] = pair else {
                                        return false;
                                    };
                                    let from = before.stops().slot(*left).interior.end;
                                    let to = before.stops().slot(*right).interior.start;
                                    separator(before.source(), from..to)
                                        .is_some_and(|at| change.from <= at && at < change.to)
                                })
                            })
                    })
                });
                if affected && array.model.rows.iter().any(|r| r.seps.is_empty()) {
                    candidates.push((slot, atom, range.len()));
                }
            }
        }
        // An inner environment is rewritten before any affected outer one.
        candidates.sort_by_key(|&(_, _, len)| len);
        let next = candidates
            .into_iter()
            .find_map(|(slot, atom, _)| normalize::rewrite(after, slot, atom));
        let Some(next) = next else {
            break;
        };
        outcome.field = next;
        outcome.changes = vec![diff(before.source(), outcome.field.source())];
        outcome.isolate = true;
    }
    outcome
}
