//! Matching the run against the table, and the expansion.
//!
//! A key starting with a symbol (`->`, `<=`, `@a`, `-lt`) matches the
//! run's last keys, the longest first, and expands from the field before
//! the first of them, whatever those keys built (`^^` is `\wedge`, not a
//! script in a script). A letter key matches the run of single-letter
//! atoms that ends at the caret, only whole: `sin` and `x\sin` expand,
//! `xsin` and `card` stay. Anything but a lone letter ends that run (a
//! digit, a control word, a script, a bracket), so `2pi` is `2\pi`. A
//! power (`sr`, `cb`, `rd`, `invs`) takes the atom before it as its base:
//! one letter of the run (`xsr` is `x^2`), else an operand before the run.
//! `!=` after an operand is a factorial and `=` (`n!=`).

use core::ops::Range;

use katex::symbols::Atom;

use super::{
    Run,
    table::{POWERS, SHORTCUTS},
};
use crate::{
    command::{ends_with_word, family, place_shortcut, selection_slot},
    field::Field,
    slot::SlotId,
};

/// The shortcut the run ends in, expanded; `landed` is the field after
/// its last key.
pub fn expand(run: &Run, landed: &Field) -> Option<Field> {
    symbol(run).or_else(|| letters(run, run.literal.as_ref().unwrap_or(landed)))
}

/// The longest symbol key the run's keys end with. After an expansion
/// only the whole run counts: a later start would undo it.
fn symbol(run: &Run) -> Option<Field> {
    let keys: Vec<char> = run.steps.iter().map(|step| step.key).collect();
    let starts = if run.literal.is_some() {
        0..1
    } else {
        0..keys.len()
    };
    starts.into_iter().find_map(|from| {
        let tail: String = keys[from..].iter().collect();
        if tail.starts_with(|c: char| c.is_ascii_alphabetic()) {
            return None;
        }
        let value = value(&tail)?;
        let before = &run.steps[from].before;
        let (slot, range) = selection_slot(before);
        if tail == "!=" && operand_before(before, slot, range.start) {
            return None;
        }
        placed(before, slot, range, value)
    })
}

/// The letter key the run of letters at `field`'s caret is.
fn letters(run: &Run, field: &Field) -> Option<Field> {
    let stops = field.stops();
    let head = field.selection().head;
    let slot = stops.stop(head).slot;
    let word = letter_run(field, slot, stops.offset(head))?;
    let text = &field.source()[word.clone()];
    // After an expansion the run of letters must hold all its keys.
    if run.literal.is_some() && text.len() < run.steps.len() {
        return None;
    }
    let (range, value) = if POWERS.contains(&text) {
        if !operand_before(field, slot, word.start) {
            return None;
        }
        (word, value(text)?)
    } else if let Some(value) = value(text) {
        (word, value)
    } else {
        // One letter, then a power: the letter is its base.
        let power = &text[1..];
        if !POWERS.contains(&power) {
            return None;
        }
        (word.start + 1..word.end, value(power)?)
    };
    placed(field, slot, unglued(field.source(), range, value), value)
}

fn value(keys: &str) -> Option<&'static str> {
    SHORTCUTS
        .iter()
        .find(|(k, _)| *k == keys)
        .map(|&(_, value)| value)
}

/// `value` in place of `range`; `None` when it would not go in.
fn placed(field: &Field, slot: SlotId, range: Range<usize>, value: &str) -> Option<Field> {
    let outcome = place_shortcut(field, slot, range, value);
    (!outcome.changes.is_empty()).then_some(outcome.field)
}

/// `range` with the space before it that only kept a control word off its
/// first letter (`\sin theta`), when `value` needs none (`\sin\theta`).
fn unglued(source: &str, range: Range<usize>, value: &str) -> Range<usize> {
    let before = &source[..range.start];
    match before.strip_suffix(' ') {
        Some(word)
            if ends_with_word(word) && !value.starts_with(|c: char| c.is_ascii_alphabetic()) =>
        {
            range.start - 1..range.end
        }
        _ => range,
    }
}

/// The run of single-letter atoms of `slot` that ends at `caret`.
fn letter_run(field: &Field, slot: SlotId, caret: usize) -> Option<Range<usize>> {
    let src = field.source();
    let atoms = &field.stops().slot(slot).atoms;
    let letter = |k: usize| {
        let text = &src[atoms[k].clone()];
        text.len() == 1 && text.as_bytes()[0].is_ascii_alphabetic()
    };
    let mut k = field.stops().atom_before(slot, caret)?;
    if atoms[k].end != caret || !letter(k) {
        return None;
    }
    while k > 0 && letter(k - 1) && atoms[k - 1].end == atoms[k].start {
        k -= 1;
    }
    Some(atoms[k].start..caret)
}

/// Whether the atom of `slot` before `offset` is something a script or a
/// factorial attaches to: not an operator, a relation, an opening
/// bracket or punctuation, nor nothing.
fn operand_before(field: &Field, slot: SlotId, offset: usize) -> bool {
    let Some(k) = field.stops().atom_before(slot, offset) else {
        return false;
    };
    let range = field.stops().slot(slot).atoms[k].clone();
    field.source()[range.clone()].starts_with(['^', '_', '\''])
        || !matches!(
            family(field, range),
            Some(Atom::Bin | Atom::Rel | Atom::Open | Atom::Punct)
        )
}
