#![allow(
    clippy::literal_string_with_formatting_args,
    reason = "LaTeX environment names, not formatting arguments"
)]
//! Splitting an alignment into plain rows and consecutive aligned runs.

use core::ops::Range;

use super::super::grid::read_array;
use crate::{
    field::Field,
    slot::{Bounds, SlotId, SlotKind},
    stops::Affinity,
};

struct Edit {
    range: Range<usize>,
    insert: String,
    /// At this boundary the caret belongs after the opening syntax,
    /// or before closing syntax, even when both share its source offset.
    after: bool,
}

pub(super) fn rewrite(field: &Field, slot: SlotId, atom: usize) -> Option<Field> {
    let stops = field.stops();
    let s = stops.slot(slot);
    let range = s.atoms.get(atom)?.clone();
    let array = read_array(field, slot, atom)?;
    let rows = &array.model.rows;
    if rows.is_empty() {
        return None;
    }
    let aligned: Vec<bool> = rows.iter().map(|row| !row.seps.is_empty()).collect();
    if aligned.iter().all(|&a| a) {
        return None;
    }
    let header_end = array.content.start;
    let open = &field.source()[range.start..header_end];
    let close = r"\end{aligned}";
    let top = field.display() && matches!(s.kind, SlotKind::Row(_)) && s.atoms.len() == 1;
    let scoped = has_switch(&field.source()[array.content.clone()]);
    let enclosed = s.bounds == Bounds::Delimited && s.atoms.len() == 1;
    let (outer_open, outer_close) = if rows.len() > 1 && (!top || scoped) {
        (r"\begin{gathered}", r"\end{gathered}")
    } else if rows.len() == 1 && !(enclosed || top && !scoped) {
        ("{", "}")
    } else {
        ("", "")
    };
    let mut edits = vec![Edit {
        range: range.start..header_end,
        insert: format!("{outer_open}{}", if aligned[0] { open } else { "" }),
        after: true,
    }];
    for r in 1..rows.len() {
        if aligned[r - 1] == aligned[r] {
            continue;
        }
        if aligned[r - 1] {
            let previous = *array.cells[r - 1].last()?;
            let at = stops.slot(previous).interior.end;
            edits.push(Edit {
                range: at..at,
                insert: close.to_owned(),
                after: false,
            });
        } else {
            let at = stops.slot(array.cells[r][0]).interior.start;
            edits.push(Edit {
                range: at..at,
                insert: open.to_owned(),
                after: true,
            });
        }
    }
    edits.push(Edit {
        range: array.content.end..range.end,
        insert: format!("{}{outer_close}", if *aligned.last()? { close } else { "" }),
        after: false,
    });
    let head = field.selection().head;
    let offset = stops.offset(head);
    let affinity = stops.affinity(head).unwrap_or(Affinity::Before);
    let caret = mapped(offset, &edits);
    let mut source = field.source().to_owned();
    for edit in edits.iter().rev() {
        source.replace_range(edit.range.clone(), &edit.insert);
    }
    if source == field.source() {
        return None;
    }
    let next = field.reparse(source).ok()?;
    if field.renders() && !next.renders() {
        return None;
    }
    Some(next.caret_at(caret, affinity))
}

fn mapped(offset: usize, edits: &[Edit]) -> usize {
    let mut removed = 0;
    let mut inserted = 0;
    for edit in edits {
        if offset < edit.range.start {
            break;
        }
        if offset < edit.range.end || (offset == edit.range.end && !edit.after) {
            return edit.range.start - removed
                + inserted
                + if edit.after { edit.insert.len() } else { 0 };
        }
        removed += edit.range.len();
        inserted += edit.insert.len();
    }
    offset - removed + inserted
}

/// Switches and local definitions need an enclosing group when the
/// alignment's scope is removed. Commands in comments and control symbols
/// cannot introduce one.
pub(super) fn has_switch(source: &str) -> bool {
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            for c in chars.by_ref() {
                if c == '\n' {
                    break;
                }
            }
        } else if c == '\\' {
            let mut name = String::new();
            while chars.peek().is_some_and(char::is_ascii_alphabetic) {
                name.push(chars.next().unwrap_or_default());
            }
            if name.is_empty() {
                chars.next();
            }
            if matches!(
                name.as_str(),
                "color"
                    | "bf"
                    | "it"
                    | "rm"
                    | "sf"
                    | "tt"
                    | "cal"
                    | "def"
                    | "gdef"
                    | "edef"
                    | "xdef"
                    | "let"
                    | "newcommand"
                    | "renewcommand"
                    | "providecommand"
                    | "displaystyle"
                    | "textstyle"
                    | "scriptstyle"
                    | "scriptscriptstyle"
                    | "tiny"
                    | "scriptsize"
                    | "footnotesize"
                    | "small"
                    | "normalsize"
                    | "large"
                    | "Large"
                    | "LARGE"
                    | "huge"
                    | "Huge"
            ) {
                return true;
            }
        }
    }
    false
}
