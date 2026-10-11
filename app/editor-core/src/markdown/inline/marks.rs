//! `injectMarks`: placing container marks into parsed inline elements.

use crate::markdown::tree::Elt;

/// `injectMarks`: puts container marks (quote marks on continuation lines)
/// into the elements, inside the innermost one that covers each.
pub(crate) fn inject_marks(elements: Vec<Elt>, marks: &[Elt]) -> Vec<Elt> {
    if marks.is_empty() {
        return elements;
    }
    if elements.is_empty() {
        return marks.to_vec();
    }
    let mut elts = elements;
    let mut ei = 0;
    for mark in marks {
        while ei < elts.len() && elts[ei].to < mark.to {
            ei += 1;
        }
        if ei < elts.len() && elts[ei].from < mark.from {
            inject_mark(&mut elts[ei].children, mark);
        } else {
            elts.insert(ei, mark.clone());
            ei += 1;
        }
    }
    elts
}

/// `injectMarks(children, [mark])`, walking down instead of recursing, so
/// deeply nested elements cannot exhaust the stack.
fn inject_mark(mut list: &mut Vec<Elt>, mark: &Elt) {
    loop {
        let mut ei = 0;
        while ei < list.len() && list[ei].to < mark.to {
            ei += 1;
        }
        if ei < list.len() && list[ei].from < mark.from {
            list = &mut list[ei].children;
        } else {
            list.insert(ei, mark.clone());
            return;
        }
    }
}
