//! Templates (`\frac{#0}{#?}`) and pasted LaTeX.

use core::ops::Range;

use super::{Target, finish, low, splice::splice};
use crate::{
    field::{Field, Outcome},
    slot::SlotId,
};

/// The template at the selection, its `#0` taking the selected source.
pub fn template(field: &Field, template: &str) -> Outcome {
    let (slot, range) = selection_slot(field);
    let fill = field.source()[range.clone()].to_owned();
    place(field, slot, range, template, &fill)
}

/// Pasted LaTeX in place of the selection, as written; nothing changes
/// when the result does not render.
pub fn paste(field: &Field, latex: &str) -> Outcome {
    let (slot, range) = selection_slot(field);
    let spliced = splice(field, slot, range, latex);
    finish(
        field,
        spliced.source,
        &Target::After(spliced.at..spliced.end),
    )
}

/// `template` in place of `range` of `slot`, `#0` filled with `fill` and
/// `#?` left empty; the caret goes to the first slot left empty, else
/// after the insertion.
pub fn place(
    field: &Field,
    slot: SlotId,
    range: Range<usize>,
    template: &str,
    fill: &str,
) -> Outcome {
    let (text, holes) = expand(template, fill);
    let spliced = splice(field, slot, range, &text);
    let target = holes
        .first()
        .map_or(Target::After(spliced.at..spliced.end), |hole| {
            Target::Empty(spliced.at + hole)
        });
    finish(field, spliced.source, &target)
}

/// The template's text, and where its empty slots are in it. `\#` is an
/// escaped `#`, not a slot.
fn expand(template: &str, fill: &str) -> (String, Vec<usize>) {
    let mut text = String::new();
    let mut holes = Vec::new();
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                text.push(c);
                if let Some(next) = chars.next() {
                    text.push(next);
                }
            }
            '#' => match chars.clone().next() {
                Some('0') => {
                    chars.next();
                    if fill.is_empty() {
                        holes.push(text.len());
                    }
                    text.push_str(fill);
                }
                Some('?') => {
                    chars.next();
                    holes.push(text.len());
                }
                _ => text.push(c),
            },
            _ => text.push(c),
        }
    }
    (text, holes)
}

/// The slot an edit of the selection happens in (its lower end's), and
/// the selected range.
pub fn selection_slot(field: &Field) -> (SlotId, Range<usize>) {
    let slot = field.stops().stop(low(field)).slot;
    (slot, field.selected())
}

#[cfg(test)]
mod tests {
    use super::expand;

    #[test]
    fn templates_expand_their_slots() {
        assert_eq!(
            expand(r"\frac{#0}{#?}", ""),
            (r"\frac{}{}".to_owned(), vec![6, 8])
        );
        assert_eq!(
            expand(r"\frac{#0}{#?}", "ab"),
            (r"\frac{ab}{}".to_owned(), vec![10])
        );
        assert_eq!(expand(r"\#x", ""), (r"\#x".to_owned(), vec![]));
    }
}
