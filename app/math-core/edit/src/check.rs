//! The invariants every formula's stops keep, run by this crate's tests
//! and by the oracle's `--stops` corpus check.

use std::collections::BTreeSet;

use katex::types::ParseError;

use crate::{
    parse::renders,
    slot::{Bounds, StopId},
    stops::{Affinity, Stops, stops},
};

/// One broken invariant, at a byte offset of the formula.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// A stop's offset is below the one before it.
    Unsorted(usize),
    /// A stop splits a character.
    OffBoundary(usize),
    /// A stop lies outside its slot's interior.
    OutsideSlot(usize),
    /// The stop's offset does not map back to it.
    RoundTrip(usize),
    /// A slot has no stop.
    NoStop(usize),
    /// Inserting a letter at the stop leaves source that does not parse.
    Unparseable(usize),
}

impl Failure {
    /// Where in the formula it failed.
    #[must_use]
    pub const fn offset(&self) -> usize {
        match self {
            Self::Unsorted(offset)
            | Self::OffBoundary(offset)
            | Self::OutsideSlot(offset)
            | Self::RoundTrip(offset)
            | Self::NoStop(offset)
            | Self::Unparseable(offset) => *offset,
        }
    }

    /// The failure's kind, for counting.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Unsorted(_) => "unsorted",
            Self::OffBoundary(_) => "off a char boundary",
            Self::OutsideSlot(_) => "outside its slot",
            Self::RoundTrip(_) => "no round trip",
            Self::NoStop(_) => "slot without a stop",
            Self::Unparseable(_) => "insert breaks the parse",
        }
    }
}

/// What [`check`] found in one formula.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub stops: usize,
    pub slots: usize,
    /// Stops in the middle of three or more at one offset, which only
    /// [`Stops::stop_in`] names.
    pub between: usize,
    pub failures: Vec<Failure>,
}

/// Lays out the stops of `source`, which must render, and checks them.
///
/// Offsets never decrease, sit on char boundaries and inside their slot;
/// each stop maps back to itself from its offset; every slot has a stop;
/// and inserting `x` at any stop of a maths slot that is not a bare
/// argument still renders (after a control word the letter takes a
/// space, as typing will).
pub fn check(source: &str, display: bool) -> Result<Report, ParseError> {
    renders(source, display)?;
    let stops = stops(source, display)?;
    let mut report = Report {
        stops: stops.stops().len(),
        slots: stops.slots().len(),
        ..Report::default()
    };
    let failures = &mut report.failures;
    let mut previous = 0;
    let mut inserts = BTreeSet::new();
    for (i, stop) in stops.stops().iter().enumerate() {
        let id = StopId(i);
        let offset = stop.offset;
        if offset < previous {
            failures.push(Failure::Unsorted(offset));
        }
        previous = offset;
        if !source.is_char_boundary(offset) {
            failures.push(Failure::OffBoundary(offset));
        }
        let slot = stops.slot(stop.slot);
        if offset < slot.interior.start || slot.interior.end < offset {
            failures.push(Failure::OutsideSlot(offset));
        }
        match round_trip(&stops, id) {
            Some(back) if back == id => {}
            Some(_) => failures.push(Failure::RoundTrip(offset)),
            None => report.between += 1,
        }
        if stops.stop_in(stop.slot, offset) != Some(id) {
            failures.push(Failure::RoundTrip(offset));
        }
        if !slot.text && slot.bounds != Bounds::Bare {
            inserts.insert(offset);
        }
    }
    for slot in stops.slots() {
        if slot.stops.is_empty() {
            failures.push(Failure::NoStop(slot.interior.start));
        }
    }
    for offset in inserts {
        if source.is_char_boundary(offset)
            && renders(&insert_letter(source, offset), display).is_err()
        {
            failures.push(Failure::Unparseable(offset));
        }
    }
    Ok(report)
}

/// The stop found from `id`'s offset with the affinity its place among
/// the stops at that offset calls for: `Before` for the first, `After`
/// for the last; `None` for one in between.
fn round_trip(stops: &Stops, id: StopId) -> Option<StopId> {
    let offset = stops.offset(id);
    let first = stops
        .prev(id)
        .is_none_or(|prev| stops.offset(prev) != offset);
    let last = stops
        .next(id)
        .is_none_or(|next| stops.offset(next) != offset);
    if first {
        Some(stops.stop_at(offset, Affinity::Before))
    } else if last {
        Some(stops.stop_at(offset, Affinity::After))
    } else {
        None
    }
}

/// `source` with `x` typed at `offset`, spaced off a control word before
/// it (`\alpha|` → `\alpha x`).
#[must_use]
pub fn insert_letter(source: &str, offset: usize) -> String {
    let before = &source[..offset];
    let letters = before.len()
        - before
            .trim_end_matches(|c: char| c.is_ascii_alphabetic())
            .len();
    let slashes =
        before[..offset - letters].len() - before[..offset - letters].trim_end_matches('\\').len();
    let letter = if letters > 0 && slashes % 2 == 1 {
        " x"
    } else {
        "x"
    };
    format!("{before}{letter}{}", &source[offset..])
}
