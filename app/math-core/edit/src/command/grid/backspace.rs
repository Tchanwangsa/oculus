//! Backspace at the start of an empty cell.

use super::{At, CellTarget, GridEdit, find::GridAt};

/// Backspace in an empty cell of a grid of more than one: its column goes
/// when all empty, else its row when all empty, else the caret steps back
/// to the end of the cell before. `None` in a cell with anything in it,
/// a one-cell grid, or the first cell (Backspace's own rule).
pub fn backspace(at: &GridAt) -> Option<GridEdit> {
    let caret = &at.caret;
    if !caret.before.is_empty() || !caret.after.is_empty() {
        return None;
    }
    let (r, c) = (caret.row, caret.col);
    let mut g = at.model.clone();
    g.pad();
    let rows = g.rows.len();
    let cols = g.width();
    if rows * cols <= 1 {
        return None;
    }
    let end = |row, col| CellTarget {
        row,
        col,
        at: At::End,
    };
    if cols > 1 && g.col_empty(c) {
        g.remove_col(c);
        let target = if c > 0 {
            end(r, c - 1)
        } else {
            back(r, g.width())
        };
        return Some(GridEdit::Edit(g, target));
    }
    if rows > 1 && g.row_empty(r) {
        g.remove_row(r);
        let target = back(r, g.width());
        return Some(GridEdit::Edit(g, target));
    }
    if c > 0 {
        return Some(GridEdit::Move(end(r, c - 1)));
    }
    (r > 0).then(|| GridEdit::Move(end(r - 1, cols - 1)))
}

/// After a removal at the start of row `r`: the previous row's end, or
/// the first cell's start.
const fn back(r: usize, width: usize) -> CellTarget {
    if r > 0 {
        CellTarget {
            row: r - 1,
            col: width - 1,
            at: At::End,
        }
    } else {
        CellTarget {
            row: 0,
            col: 0,
            at: At::Start,
        }
    }
}
