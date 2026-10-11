//! `;` in a grid's cell.

use super::{At, CellTarget, GridEdit, find::GridAt};

/// `;`: the caret goes to the start of the row after its own, a new empty
/// row unless that row is already empty. Never typed inside a matrix; in
/// a bracket group the new row makes it a matrix.
pub fn semicolon(at: &GridAt) -> GridEdit {
    let r = at.caret.row;
    let mut g = at.model.clone();
    g.pad();
    let target = CellTarget {
        row: r + 1,
        col: 0,
        at: At::Start,
    };
    if r + 1 < g.rows.len() && g.row_empty(r + 1) {
        return GridEdit::Move(target);
    }
    let width = g.width();
    g.insert_row(r, width);
    GridEdit::Edit(g, target)
}
