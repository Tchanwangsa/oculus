//! Matrices typed as in MATLAB (`[a b; c d]`): Space between terms starts
//! a cell, `;` a row, Backspace in an empty cell takes its column or row
//! back, and the closing bracket trims empty ends. A grid is a matrix
//! environment's cells ([`find::MATRIX_ENVS`]), or a bracket group as one
//! cell, which becomes the matrix its brackets draw at its first new cell
//! or row. Each key's rule is in its own file; `write` splices the result
//! into the source (one change, its own undo step), keeping the text of
//! every cell and separator it did not touch.

mod backspace;
mod close;
mod enter;
mod find;
mod layout;
mod model;
mod row;
mod space;
mod write;

use super::{
    Target, finish,
    template::{place, selection_slot},
};
use crate::{
    field::{Field, Outcome},
    slot::SlotKind,
};
use find::grid_at;
use model::Model;

pub use enter::array_row;
pub use layout::top_break;

/// Which end of a cell the caret goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum At {
    Start,
    End,
}

/// A cell, by row and column, and which end of it.
#[derive(Clone, Copy, Debug)]
pub struct CellTarget {
    pub row: usize,
    pub col: usize,
    pub at: At,
}

/// What a key does to a grid.
#[derive(Clone, Debug)]
pub enum GridEdit {
    /// The grid rewritten (a bracket group when one cell is left), the
    /// caret in a cell.
    Edit(Model, CellTarget),
    /// Only the caret moves.
    Move(CellTarget),
    /// A matrix closed: the caret after it.
    Close(Model),
}

/// The key that closes a matrix as its right bracket.
fn close_key(env: &str) -> Option<char> {
    Some(match env {
        "pmatrix" => ')',
        "bmatrix" => ']',
        "Bmatrix" => '}',
        "vmatrix" => '|',
        _ => return None,
    })
}

/// A plain key in the grid at the caret: Space, `;` or a matrix's closing
/// bracket (a bracket group's closer is typed as itself). `None` when it
/// isn't a grid key there, or would change nothing.
pub fn key(field: &Field, c: char) -> Option<Outcome> {
    if !matches!(c, ' ' | ';' | ')' | ']' | '}' | '|') {
        return None;
    }
    let at = grid_at(field)?;
    let edit = match c {
        ' ' => space::space(&at)?,
        ';' => row::semicolon(&at),
        _ if !at.group && close_key(at.env) == Some(c) => close::close(&at),
        _ => return None,
    };
    acted(field, write::apply(field, &at, edit))
}

/// Backspace at the start of an empty cell of a grid of more than one:
/// its column or row goes when all empty.
pub fn backspace(field: &Field) -> Option<Outcome> {
    let at = grid_at(field)?;
    let edit = backspace::backspace(&at)?;
    acted(field, write::apply(field, &at, edit))
}

/// Whether Space edits or moves in a grid here (so it is not the view's).
pub fn takes_space(field: &Field) -> bool {
    key(field, ' ').is_some()
}

/// `&`: in an array cell (any array: `aligned` too) a raw `&`, starting a
/// new cell, the caret at its start; elsewhere, or where a new cell would
/// not render, `\&`.
pub fn ampersand(field: &Field) -> Outcome {
    let (slot, range) = selection_slot(field);
    let stops = field.stops();
    if matches!(stops.slot(slot).kind, SlotKind::Cell { .. }) {
        let mut path = stops.slot(slot).path.clone();
        if let Some(last) = path.steps.last_mut() {
            last.1 += 1;
        }
        let spliced = super::splice::splice(field, slot, range.clone(), "&");
        let outcome = finish(
            field,
            spliced.source,
            &Target::Slot {
                path,
                offset: spliced.end,
            },
        );
        if !outcome.changes.is_empty() {
            return outcome;
        }
    }
    place(field, slot, range, r"\&", "")
}

/// The outcome, when it changed the source or moved the caret.
fn acted(field: &Field, outcome: Outcome) -> Option<Outcome> {
    (!outcome.changes.is_empty() || outcome.field.selection() != field.selection())
        .then_some(outcome)
}
