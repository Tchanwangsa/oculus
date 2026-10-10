//! Space in a grid's cell.

use super::{
    At, CellTarget, GridEdit,
    find::{Follows, GridAt},
    model::Piece,
};

/// Most columns a row takes (MathLive's limit, which the field kept).
const MAX_COLS: usize = 10;

/// Space in a grid's cell. An operator alone in a cell after the row's
/// first rejoins the cell before it (`[a + b]` is one cell, `[1 -1]` two):
/// its column goes when nothing else is in it, else the row's later cells
/// shift left. After a term Space ends the cell: into the next one when
/// it is empty and the caret at the cell's end, else into a new column
/// holding what was after the caret. `None` leaves Space to the view: an
/// empty cell, a cell's start, after an operator, at ten columns.
pub fn space(at: &GridAt) -> Option<GridEdit> {
    let caret = &at.caret;
    if caret.before.is_empty() && caret.after.is_empty() {
        return None;
    }
    let (r, c) = (caret.row, caret.col);
    let mut g = at.model.clone();
    g.pad();
    if caret.lone && c > 0 {
        let column_empty = g
            .rows
            .iter()
            .enumerate()
            .all(|(i, row)| i == r || row.cells[c].trim().is_empty());
        g.join_back(r, c);
        if column_empty {
            for i in (0..g.rows.len()).filter(|&i| i != r) {
                g.remove_cell(i, c);
            }
        } else {
            g.rows[r].seps.push(Piece::New);
            g.rows[r].cells.push(String::new());
        }
        return Some(GridEdit::Edit(
            g,
            CellTarget {
                row: r,
                col: c - 1,
                at: At::End,
            },
        ));
    }
    if caret.follows != Follows::Term {
        return None;
    }
    if caret.after.is_empty() && c + 1 < g.width() && g.rows[r].cells[c + 1].trim().is_empty() {
        return Some(GridEdit::Move(CellTarget {
            row: r,
            col: c + 1,
            at: At::Start,
        }));
    }
    if g.width() >= MAX_COLS {
        return None;
    }
    g.insert_col(c);
    g.rows[r].cells[c].clone_from(&caret.before);
    g.rows[r].cells[c + 1].clone_from(&caret.after);
    Some(GridEdit::Edit(
        g,
        CellTarget {
            row: r,
            col: c + 1,
            at: At::Start,
        },
    ))
}
