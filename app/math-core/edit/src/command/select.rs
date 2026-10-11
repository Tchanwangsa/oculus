//! Widening a selection to whole structures.

use crate::{
    slot::{SlotId, StopId},
    stops::Stops,
};

/// A selection widened to whole structures.
///
/// When the ends sit in different slots, both move to the slot they
/// share and every structure the selection reaches into is taken whole:
/// an end inside a structure moves to before or after the structure's atom
/// in the slot the two ends share (a drag from beside a matrix into a
/// cell takes the matrix). Ends in different top-level rows stay in
/// their rows, each widened there: a block's rows select as one run.
///
/// The head moves the way `toward_end` says when it is given (Shift+→
/// takes the structure it enters, Shift+← gives back the one it leaves),
/// else away from the anchor; the anchor moves away from the head.
#[must_use]
pub fn widen(
    stops: &Stops,
    anchor: StopId,
    head: StopId,
    toward_end: Option<bool>,
) -> (StopId, StopId) {
    let a_slot = stops.stop(anchor).slot;
    let h_slot = stops.stop(head).slot;
    if a_slot == h_slot {
        return (anchor, head);
    }
    let a_chain = stops.ancestors(a_slot);
    let h_chain = stops.ancestors(h_slot);
    let common = a_chain
        .iter()
        .enumerate()
        .find_map(|(i, slot)| h_chain.iter().position(|s| s == slot).map(|j| (i, j)));
    let (i, j) = common.unwrap_or((a_chain.len() - 1, h_chain.len() - 1));
    // The atom each end lifts to, in the slot it lifts into.
    let lifted = |chain: &[SlotId], n: usize| -> Option<(SlotId, usize)> {
        (n > 0).then(|| stops.owner(chain[n - 1])).flatten()
    };
    let a_atom = lifted(&a_chain, i);
    let h_atom = lifted(&h_chain, j);
    let forward = toward_end.unwrap_or(head > anchor);
    let new_head = h_atom.map_or(head, |(slot, atom)| {
        if forward {
            stops.stop_after_atom(slot, atom)
        } else {
            stops.stop_before_atom(slot, atom)
        }
    });
    let new_anchor = a_atom.map_or(anchor, |(slot, atom)| {
        let same = h_atom == Some((slot, atom));
        let anchor_first = if same {
            forward
        } else {
            new_head >= stops.stop_after_atom(slot, atom)
        };
        if anchor_first {
            stops.stop_before_atom(slot, atom)
        } else {
            stops.stop_after_atom(slot, atom)
        }
    });
    (new_anchor, new_head)
}
