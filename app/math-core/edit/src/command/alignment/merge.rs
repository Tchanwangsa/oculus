//! Adjacent aligned display rows share one environment after re-alignment.

use core::ops::Range;

use super::{super::grid::read_array, normalize::has_switch};
use crate::{
    field::Field,
    slot::{SlotId, SlotKind},
    stops::Affinity,
};

pub(super) fn adjacent(field: Field) -> Field {
    let stops = field.stops();
    let head = field.selection().head;
    let row = stops.row_of(stops.stop(head).slot);
    let rows: Vec<_> = stops
        .slots_with_ids()
        .filter_map(|(id, s)| matches!(s.kind, SlotKind::Row(_)).then_some(id))
        .collect();
    let Some(index) = rows.iter().position(|&id| id == row) else {
        return field;
    };
    if extent(&field, row).is_none() {
        return field;
    }
    let mut first = index;
    let mut last = index;
    while first > 0 && extent(&field, rows[first - 1]).is_some() {
        first -= 1;
    }
    while last + 1 < rows.len() && extent(&field, rows[last + 1]).is_some() {
        last += 1;
    }
    if first == last {
        return field;
    }
    let mut remove = Vec::new();
    for &row in &rows[first..=last] {
        let Some((range, body)) = extent(&field, row) else {
            return field;
        };
        if row != rows[first] {
            remove.push(range.start..body.start);
        }
        if row != rows[last] {
            remove.push(body.end..range.end);
        }
    }
    let offset = stops.offset(head);
    let caret = offset
        - remove
            .iter()
            .filter(|r| r.end <= offset)
            .map(Range::len)
            .sum::<usize>();
    let affinity = stops.affinity(head).unwrap_or(Affinity::Before);
    let mut source = field.source().to_owned();
    for range in remove.into_iter().rev() {
        source.replace_range(range, "");
    }
    let Ok(next) = field.reparse(source) else {
        return field;
    };
    if field.renders() && !next.renders() {
        return field;
    }
    next.caret_at(caret, affinity)
}

/// The sole aligned environment of a row. Local definitions and style
/// switches keep their scope rather than crossing a newly joined boundary.
fn extent(field: &Field, row: SlotId) -> Option<(Range<usize>, Range<usize>)> {
    let s = field.stops().slot(row);
    if s.atoms.len() != 1 {
        return None;
    }
    let array = read_array(field, row, 0)?;
    if array.name != "aligned" || has_switch(&field.source()[array.content.clone()]) {
        return None;
    }
    Some((s.atoms[0].clone(), array.content))
}
