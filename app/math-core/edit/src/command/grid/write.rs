//! Applying a grid edit to the source: the matrix's content rewritten in
//! place, a bracket group replaced by its matrix, or a one-cell matrix by
//! its bracket group. Whatever is outside (scripts on the closer, `^T`)
//! stays where it is.

use core::ops::Range;

use super::{At, CellTarget, GridEdit, find::GridAt, model::Model};
use crate::{
    command::{Target, finish, splice::splice},
    field::{Field, Outcome, Selection},
    slot::{SlotId, SlotPath},
};

/// The brackets a matrix is written with as a bracket group.
fn group_brackets(env: &str) -> Option<(&'static str, &'static str)> {
    Some(match env {
        "pmatrix" => ("(", ")"),
        "bmatrix" => ("[", "]"),
        "Bmatrix" => (r"\{", r"\}"),
        "vmatrix" => ("|", "|"),
        "Vmatrix" => (r"\|", r"\|"),
        _ => return None,
    })
}

pub fn apply(field: &Field, at: &GridAt, edit: GridEdit) -> Outcome {
    match edit {
        GridEdit::Move(target) => {
            let cell = at.cells.get(target.row).and_then(|row| row.get(target.col));
            if let Some(&cell) = cell {
                let stops = field.stops();
                let stop = match target.at {
                    At::Start => stops.first_stop(cell),
                    At::End => stops.last_stop(cell),
                };
                return Outcome::moved(field.clone().select(Selection::caret(stop)));
            }
            // A row shorter than the grid: padded, then the caret goes there.
            let mut model = at.model.clone();
            model.pad();
            rewrite(field, at, &model, target)
        }
        GridEdit::Edit(model, target) => rewrite(field, at, &model, target),
        GridEdit::Close(model) => close(field, at, &model),
    }
}

/// The grid written back, the caret in `target`. One cell left of a
/// matrix that has brackets is its bracket group again, left open (the
/// closer is the user's to type) unless scripts follow it.
fn rewrite(field: &Field, at: &GridAt, model: &Model, target: CellTarget) -> Outcome {
    let path = field.stops().slot(at.slot).path.clone();
    if model.one_cell()
        && let Some((open, close)) = group_brackets(at.env)
    {
        let cell = &model.rows[0].cells[0];
        let close = if at.scripts_end.is_some() { close } else { "" };
        let text = format!("{open}{cell}{close}");
        let rel = open.len() + if target.at == At::End { cell.len() } else { 0 };
        return replace(field, at.slot, at.range.clone(), &text, |start, _| {
            Target::Slot {
                path,
                offset: start + rel,
            }
        });
    }
    let written = model.write();
    let (range, text, base) = if at.group {
        let begin = format!(r"\begin{{{}}}", at.env);
        let text = format!(r"{begin}{}\end{{{}}}", written.text, at.env);
        (at.range.clone(), text, begin.len())
    } else {
        (at.content.clone(), written.text, 0)
    };
    let cell = written.cells[target.row][target.col].clone();
    let rel = base
        + match target.at {
            At::Start => cell.start,
            At::End => cell.end,
        };
    let cell_path = cell_path(&path, at.atom, model.index(target.row, target.col));
    replace(field, at.slot, range, &text, |start, _| Target::Slot {
        path: cell_path,
        offset: start + rel,
    })
}

/// The closed matrix (or its closed bracket group, at one cell), the
/// caret after it and its scripts.
fn close(field: &Field, at: &GridAt, model: &Model) -> Outcome {
    let path = field.stops().slot(at.slot).path.clone();
    let (range, text) = match group_brackets(at.env) {
        Some((open, close)) if model.one_cell() => (
            at.range.clone(),
            format!("{open}{}{close}", model.rows[0].cells[0]),
        ),
        _ => (at.content.clone(), model.write().text),
    };
    let old = field.source().len();
    let after = at.scripts_end.unwrap_or(at.range.end);
    replace(field, at.slot, range, &text, |_, source| Target::Slot {
        path,
        offset: source.len() - (old - after),
    })
}

/// `text` in place of `range` of `slot` as one undo step, the caret where
/// `target` says (given where `text` starts in the new source, and that
/// source).
pub fn replace<T: FnOnce(usize, &str) -> Target>(
    field: &Field,
    slot: SlotId,
    range: Range<usize>,
    text: &str,
    target: T,
) -> Outcome {
    let spliced = splice(field, slot, range, text);
    let target = target(spliced.at, &spliced.source);
    let outcome = finish(field, spliced.source, &target);
    Outcome {
        isolate: !outcome.changes.is_empty(),
        ..outcome
    }
}

/// The path of cell `index` (row by row) of atom `atom` of the slot at
/// `path`.
pub fn cell_path(path: &SlotPath, atom: usize, index: usize) -> SlotPath {
    let mut path = path.clone();
    path.steps.push((atom, index));
    path
}
