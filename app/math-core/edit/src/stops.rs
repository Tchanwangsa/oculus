//! A formula's caret stops and the moves between them.

use crate::{
    build::build,
    parse::parse,
    slot::{Slot, SlotId, Stop, StopId},
};
use katex::types::ParseError;

/// Which of the stops that share an offset is meant.
///
/// Stops in different slots can share an offset: `\frac ab`'s offset 7 is
/// the numerator's end and the denominator's start. At one offset the
/// stops run, in ←/→ order, from the slots that end there (innermost
/// first) to the slots that start there (outermost first). `Before` is the
/// first of them, the one that goes with the text before the offset;
/// `After` the last, the one that goes with the text after it. A run of
/// three or more (`\sqrt\frac ab` at its end) has stops in between that
/// only [`Stops::stop_in`] names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Affinity {
    Before,
    After,
}

/// The caret stops of one formula, in ←/→ order (which is source order),
/// and the slots they sit in.
#[derive(Clone, Debug)]
pub struct Stops {
    slots: Vec<Slot>,
    stops: Vec<Stop>,
}

/// Parses `source` for editing and lays out its stops. Unparseable source
/// has no stops: the error is returned and the view edits it as TeX.
pub fn stops(source: &str, display: bool) -> Result<Stops, ParseError> {
    let nodes = parse(source, display)?;
    let (slots, stops) = build(source, &nodes);
    Ok(Stops { slots, stops })
}

impl Stops {
    /// Every stop, in ←/→ order; offsets never decrease.
    #[must_use]
    pub fn stops(&self) -> &[Stop] {
        &self.stops
    }

    /// Every slot, each before the slots inside it.
    #[must_use]
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    #[must_use]
    pub fn stop(&self, id: StopId) -> &Stop {
        &self.stops[id.0]
    }

    #[must_use]
    pub fn slot(&self, id: SlotId) -> &Slot {
        &self.slots[id.0]
    }

    /// The stop's byte offset in the source.
    #[must_use]
    pub fn offset(&self, id: StopId) -> usize {
        self.stops[id.0].offset
    }

    /// The stop at `offset`, chosen among stops sharing it by `affinity`.
    /// An offset with no stop (inside `\alpha`) gives the nearest stop
    /// before it (`Before`) or after it (`After`).
    #[must_use]
    pub fn stop_at(&self, offset: usize, affinity: Affinity) -> StopId {
        let first = self.stops.partition_point(|stop| stop.offset < offset);
        let past = self.stops.partition_point(|stop| stop.offset <= offset);
        let last = self.stops.len() - 1;
        StopId(match affinity {
            Affinity::Before if first < past => first,
            Affinity::After if first < past => past - 1,
            Affinity::Before => first.saturating_sub(1),
            Affinity::After => first.min(last),
        })
    }

    /// The affinity that [`Self::stop_at`] maps this stop's offset back to
    /// it with; `None` for a stop in the middle of three or more.
    #[must_use]
    pub fn affinity(&self, id: StopId) -> Option<Affinity> {
        let offset = self.offset(id);
        if self.stop_at(offset, Affinity::Before) == id {
            Some(Affinity::Before)
        } else if self.stop_at(offset, Affinity::After) == id {
            Some(Affinity::After)
        } else {
            None
        }
    }

    /// The stop of `slot` at `offset`: one at most, since a slot's stops
    /// have distinct offsets.
    #[must_use]
    pub fn stop_in(&self, slot: SlotId, offset: usize) -> Option<StopId> {
        self.slots[slot.0]
            .stops
            .iter()
            .copied()
            .find(|&id| self.offset(id) == offset)
    }

    /// The stop → of this one.
    #[must_use]
    pub fn next(&self, id: StopId) -> Option<StopId> {
        (id.0 + 1 < self.stops.len()).then(|| StopId(id.0 + 1))
    }

    /// The stop ← of this one.
    #[must_use]
    pub fn prev(&self, id: StopId) -> Option<StopId> {
        id.0.checked_sub(1).map(StopId)
    }

    /// The slot holding the atom `slot` belongs to; `None` for a row.
    #[must_use]
    pub fn parent(&self, slot: SlotId) -> Option<SlotId> {
        self.slots[slot.0].parent
    }

    #[must_use]
    pub fn first_stop(&self, slot: SlotId) -> StopId {
        self.slots[slot.0].stops[0]
    }

    #[must_use]
    pub fn last_stop(&self, slot: SlotId) -> StopId {
        let stops = &self.slots[slot.0].stops;
        stops[stops.len() - 1]
    }

    /// The stop of the first empty slot after this stop (Tab).
    #[must_use]
    pub fn next_empty(&self, id: StopId) -> Option<StopId> {
        (id.0 + 1..self.stops.len())
            .map(StopId)
            .find(|&next| self.in_empty_slot(next))
    }

    /// The stop of the last empty slot before this stop (Shift+Tab).
    #[must_use]
    pub fn prev_empty(&self, id: StopId) -> Option<StopId> {
        (0..id.0)
            .rev()
            .map(StopId)
            .find(|&prev| self.in_empty_slot(prev))
    }

    fn in_empty_slot(&self, id: StopId) -> bool {
        self.slots[self.stops[id.0].slot.0].is_empty()
    }
}
