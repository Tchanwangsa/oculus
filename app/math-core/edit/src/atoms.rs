//! Atoms and the slots around them: what the editing commands ask of a
//! formula's stops.
//!
//! An atom is named by its slot and its index in [`Slot::atoms`]; a
//! structure is an atom with slots of its own (a fraction, a script atom,
//! a `\text{}`).

use crate::{
    slot::{Slot, SlotId, StopId},
    stops::Stops,
};

impl Stops {
    /// The slot holding the atom `slot` belongs to, and that atom's index
    /// there; `None` for a row.
    #[must_use]
    pub fn owner(&self, slot: SlotId) -> Option<(SlotId, usize)> {
        let s = self.slot(slot);
        Some((s.parent?, s.path.steps.last()?.0))
    }

    /// The slots of atom `atom` of `slot`, in source order.
    #[must_use]
    pub fn atom_slots(&self, slot: SlotId, atom: usize) -> Vec<SlotId> {
        (slot.0 + 1..self.slots().len())
            .map(SlotId)
            .filter(|&child| self.owner(child) == Some((slot, atom)))
            .collect()
    }

    /// Whether atom `atom` of `slot` has slots of its own.
    #[must_use]
    pub fn is_structure(&self, slot: SlotId, atom: usize) -> bool {
        !self.atom_slots(slot, atom).is_empty()
    }

    /// `slot`, then each slot holding the one before, up to its row.
    #[must_use]
    pub fn ancestors(&self, slot: SlotId) -> Vec<SlotId> {
        let mut out = vec![slot];
        let mut at = slot;
        while let Some(parent) = self.parent(at) {
            out.push(parent);
            at = parent;
        }
        out
    }

    /// The top-level row `slot` is in.
    #[must_use]
    pub fn row_of(&self, slot: SlotId) -> SlotId {
        let chain = self.ancestors(slot);
        chain[chain.len() - 1]
    }

    /// The last atom of `slot` that ends at or before `offset`.
    #[must_use]
    pub fn atom_before(&self, slot: SlotId, offset: usize) -> Option<usize> {
        self.slot(slot)
            .atoms
            .iter()
            .rposition(|atom| atom.end <= offset)
    }

    /// The first atom of `slot` that starts at or after `offset`.
    #[must_use]
    pub fn atom_after(&self, slot: SlotId, offset: usize) -> Option<usize> {
        self.slot(slot)
            .atoms
            .iter()
            .position(|atom| atom.start >= offset)
    }

    /// The atom of `slot` holding `offset` strictly inside it (a text
    /// run's stop between two characters of one atom).
    #[must_use]
    pub fn atom_around(&self, slot: SlotId, offset: usize) -> Option<usize> {
        self.slot(slot)
            .atoms
            .iter()
            .position(|atom| atom.start < offset && offset < atom.end)
    }

    /// The stop of `slot` just before its atom `atom`.
    #[must_use]
    pub fn stop_before_atom(&self, slot: SlotId, atom: usize) -> StopId {
        let s = self.slot(slot);
        let start = s.atoms[atom].start;
        s.stops
            .iter()
            .copied()
            .rev()
            .find(|&id| self.offset(id) <= start)
            .unwrap_or(s.stops[0])
    }

    /// The stop of `slot` just after its atom `atom`.
    #[must_use]
    pub fn stop_after_atom(&self, slot: SlotId, atom: usize) -> StopId {
        let s = self.slot(slot);
        let end = s.atoms[atom].end;
        s.stops
            .iter()
            .copied()
            .find(|&id| self.offset(id) >= end)
            .unwrap_or(s.stops[s.stops.len() - 1])
    }

    /// Whether every slot of atom `atom` of `slot` is empty.
    #[must_use]
    pub fn all_empty(&self, slot: SlotId, atom: usize) -> bool {
        self.atom_slots(slot, atom)
            .iter()
            .all(|&child| self.slot(child).is_empty())
    }

    /// Whether the stop is its slot's first.
    #[must_use]
    pub fn at_slot_start(&self, id: StopId) -> bool {
        self.stop(id).index == 0
    }

    /// Whether the stop is its slot's last.
    #[must_use]
    pub fn at_slot_end(&self, id: StopId) -> bool {
        let stop = self.stop(id);
        stop.index + 1 == self.slot(stop.slot).stops.len()
    }

    /// The slots in order, with their ids.
    pub fn slots_with_ids(&self) -> impl Iterator<Item = (SlotId, &Slot)> {
        self.slots()
            .iter()
            .enumerate()
            .map(|(i, slot)| (SlotId(i), slot))
    }
}
