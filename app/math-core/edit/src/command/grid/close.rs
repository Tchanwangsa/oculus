//! The closing bracket in a matrix's cell.

use super::{GridEdit, find::GridAt};

/// The matrix without trailing empty rows and columns, the caret after
/// it; a single cell left is a closed bracket group.
pub fn close(at: &GridAt) -> GridEdit {
    let mut g = at.model.clone();
    g.trim();
    GridEdit::Close(g)
}
