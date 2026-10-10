//! How new separators are written, so a display block keeps one row per
//! line (`a \\`, a newline, `b`), as the note stores it, and everything
//! else keeps the style it is written in.

use core::ops::Range;

use super::model::{Model, Style};
use crate::{
    field::Field,
    slot::{SlotId, SlotKind},
};

/// A display block's line break between rows.
const BLOCK_BREAK: &str = " \\\\\n";

/// The style of an array's new separators: its own first `&` and `\\`
/// where it has them. A new `\\` in an environment that is a whole
/// display formula goes at a line's end; elsewhere rows stay on one line.
pub fn style(field: &Field, slot: SlotId, range: &Range<usize>, model: &Model) -> Style {
    let (col, row) = model.kept_seps();
    let col = if col == Some("&") { "&" } else { " & " };
    let whole = field.display()
        && matches!(field.stops().slot(slot).kind, SlotKind::Row(_))
        && field.source().trim() == &field.source()[range.clone()];
    let row = match row {
        Some(row) if row.trim() == r"\\" => row.to_owned(),
        _ if whole => BLOCK_BREAK.to_owned(),
        _ if col == "&" => r"\\".to_owned(),
        _ => r" \\ ".to_owned(),
    };
    Style {
        col,
        row,
        block: whole && model.breaks.is_empty(),
    }
}

/// The break Enter writes between top-level rows of display maths: the
/// formula's first one, when it is a plain `\\` with its spaces, else a
/// `\\` ending the line.
pub fn top_break(field: &Field) -> String {
    let stops = field.stops();
    let row = |n: usize| {
        stops
            .slots()
            .iter()
            .find(|slot| slot.kind == SlotKind::Row(n))
    };
    match (row(0), row(1)) {
        (Some(first), Some(second)) => {
            let text = &field.source()[first.interior.end..second.interior.start];
            if text.trim() == r"\\" {
                text.to_owned()
            } else {
                BLOCK_BREAK.to_owned()
            }
        }
        _ => BLOCK_BREAK.to_owned(),
    }
}
