//! Option-arrow moves through sibling runs without entering structures.

use super::motion::{extend_to, horizontal as step, ordered};
use crate::{
    field::{Field, Outcome, Selection},
    slot::{SlotKind, StopId},
};

pub fn horizontal(field: &Field, right: bool, extend: bool) -> Outcome {
    let stops = field.stops();
    let selection = field.selection();
    if !extend && !selection.is_caret() {
        let (lo, hi) = ordered(selection);
        return moved(field, if right { hi } else { lo }, right, false);
    }
    let head = selection.head;
    let slot = stops.stop(head).slot;
    let s = stops.slot(slot);
    let index = stops.stop(head).index;
    let edge = if right {
        index + 1 == s.stops.len()
    } else {
        index == 0
    };
    if edge {
        if let Some((parent, atom)) = stops.owner(slot) {
            // A cell boundary moves to the next cell rather than outside
            // the entire alignment or matrix.
            if matches!(s.kind, SlotKind::Cell { .. }) {
                return step(field, right, extend);
            }
            let target = if right {
                stops.stop_after_atom(parent, atom)
            } else {
                stops.stop_before_atom(parent, atom)
            };
            return moved(field, target, right, extend);
        }
        return step(field, right, extend);
    }
    let mut i = index;
    let mut category = None;
    loop {
        let next = if right { i + 1 } else { i - 1 };
        let a = stops.offset(s.stops[i.min(next)]);
        let b = stops.offset(s.stops[i.max(next)]);
        let source = field.source()[a..b].trim();
        let structure = s
            .atoms
            .iter()
            .enumerate()
            .any(|(n, atom)| a <= atom.start && atom.end <= b && stops.is_structure(slot, n));
        let kind = if structure {
            3
        } else if source.is_empty() || source == "~" {
            0
        } else if source
            .trim_start_matches('\\')
            .chars()
            .all(|c| c.is_alphanumeric() || c == '\\' || !c.is_ascii())
        {
            1
        } else {
            2
        };
        if let Some(previous) = category {
            // Consume leading whitespace then one run, as word movement
            // does in prose. Structures and punctuation are one jump each.
            if previous != 0 && (kind != previous || kind >= 2) {
                break;
            }
        }
        i = next;
        category = Some(kind);
        if kind >= 2 || (right && i + 1 == s.stops.len()) || (!right && i == 0) {
            break;
        }
    }
    moved(field, s.stops[i], right, extend)
}

fn moved(field: &Field, target: StopId, right: bool, extend: bool) -> Outcome {
    if extend {
        extend_to(field, target, right)
    } else {
        Outcome::moved(field.clone().select(Selection::caret(target)))
    }
}
