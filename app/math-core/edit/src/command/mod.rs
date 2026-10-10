//! The editing commands: each takes a [`Field`] and returns an
//! [`Outcome`], with no DOM and no undo stack.
//!
//! Every command keeps four rules. The new source renders whenever the
//! old one did ([`finish`] refuses any edit that would not, so a command
//! that cannot be made safely does nothing). A control word never runs
//! into a letter after it (`glue`). A bare argument (`x^2`) gets braces
//! before it holds a second atom, and `{}` when it is emptied (`splice`).
//! Deleting what was just inserted gives the source back, except where an
//! insertion braced a bare argument, or a deletion drops the space after a
//! control word that it can no longer tell from a typed one (`glue`).
//!
//! [`Field::run`] gives a typed key and Esc to `crate::shortcut` first;
//! the commands here call [`Field::run_plain`], which never expands one.

mod command_mode;
mod delete;
mod glue;
mod grid;
mod insert;
mod motion;
mod rows;
mod select;
mod splice;
mod template;
mod text;
mod vertical;

use core::{cmp::Reverse, ops::Range};

use crate::{
    field::{Change, Field, Outcome, Selection},
    shortcut,
    slot::{SlotPath, StopId},
    stops::{Affinity, Stops},
};

pub use glue::ends_with_word;
pub use grid::takes_space;
pub use insert::family;
pub use select::widen;
pub use template::{place_shortcut, selection_slot};
pub use text::mark_at;

/// One key or input the field handles.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Typed text: one key's character, or an IME's whole string. In
    /// maths each character is typed in turn (`\` starts a command, `^`,
    /// `_`, `/`, `{`, `}` build structure, `#$%&` are escaped, `~` is
    /// `\sim`, Space types nothing); in a text run the string goes in
    /// whole, escaped for text mode.
    Insert(String),
    /// A LaTeX snippet whose `#0` takes the selection and whose `#?` are
    /// empty slots (`\frac{#0}{#?}`); the caret goes to the first empty
    /// slot, or after the snippet.
    Template(String),
    /// Pasted LaTeX, inserted as written when the result renders;
    /// otherwise nothing changes (the view can fall back to TeX mode).
    Paste(String),
    Backspace,
    Delete,
    /// ⌘Backspace: the caret's row (or array cell) up to the caret.
    DeleteLine,
    Left {
        extend: bool,
    },
    Right {
        extend: bool,
    },
    /// ↑ and ↓: `xs` is the rendered x of each stop, by stop index (NaN
    /// where the view has none); the caret goes to the stacked slot's
    /// stop nearest its own x.
    Up {
        xs: Vec<f64>,
    },
    Down {
        xs: Vec<f64>,
    },
    Home {
        extend: bool,
    },
    End {
        extend: bool,
    },
    SelectAll,
    Tab,
    ShiftTab,
    /// Enter or Shift+Enter.
    Enter,
    Escape,
}

impl Field {
    /// Applies `command`. A key typed in maths may expand a shortcut
    /// (the outcome's `rewrite`), and Esc right after one reverts it.
    #[must_use]
    pub fn run(&self, command: &Command) -> Outcome {
        if let Some(outcome) = shortcut::run(self, command) {
            return outcome;
        }
        let mut outcome = self.run_plain(command);
        outcome.field.set_shortcut(None);
        outcome
    }

    /// Applies `command` with no shortcut: the commands themselves.
    pub(crate) fn run_plain(&self, command: &Command) -> Outcome {
        if self.pending().is_some() {
            if let Some(outcome) = command_mode::run(self, command) {
                return outcome;
            }
            // Any other key drops the command being typed, then acts.
            return dispatch(&self.clone().with_pending(None), command);
        }
        dispatch(self, command)
    }
}

fn dispatch(field: &Field, command: &Command) -> Outcome {
    match command {
        Command::Insert(text) => insert::insert(field, text),
        Command::Template(template) => template::template(field, template),
        Command::Paste(latex) => template::paste(field, latex),
        Command::Backspace => delete::backward(field),
        Command::Delete => delete::forward(field),
        Command::DeleteLine => delete::line(field),
        Command::Left { extend } => motion::horizontal(field, false, *extend),
        Command::Right { extend } => motion::horizontal(field, true, *extend),
        Command::Up { xs } => vertical::vertical(field, false, xs),
        Command::Down { xs } => vertical::vertical(field, true, xs),
        Command::Home { extend } => motion::home_end(field, false, *extend),
        Command::End { extend } => motion::home_end(field, true, *extend),
        Command::SelectAll => motion::select_all(field),
        Command::Tab => motion::tab(field),
        Command::ShiftTab => motion::shift_tab(field),
        Command::Enter => rows::enter(field),
        Command::Escape => motion::escape(field),
    }
}

/// Where the caret goes in the new source.
#[derive(Clone, Debug)]
pub enum Target {
    /// In the slot at `path` (paths survive edits inside that slot), at
    /// `offset` or the first stop after it.
    Slot { path: SlotPath, offset: usize },
    /// In the empty slot whose content starts at `offset`.
    Empty(usize),
    /// Just after inserted text, in the innermost slot holding all of it
    /// (text typed at a row's end after an infix `\over` lands in its
    /// denominator, and so does the caret).
    After(Range<usize>),
    /// The empty slot whose content starts at `offset`, else the stop at
    /// `offset` in the innermost slot around it: a shortcut's placeholder
    /// that is not a whole argument (`\lim_{#0\to#?}`).
    Hole(usize),
}

/// The field after replacing its source with `source` (from a command's
/// edit) and placing the caret at `target`; the field unchanged when the
/// new source does not parse, or does not render though the old did.
pub fn finish(field: &Field, source: String, target: &Target) -> Outcome {
    if source == field.source() {
        let Some(caret) = resolve(field.stops(), target) else {
            return Outcome::none(field);
        };
        return Outcome::moved(field.clone().select(Selection::caret(caret)));
    }
    let Ok(mut new) = field.reparse(source) else {
        return Outcome::none(field);
    };
    if field.renders() && !new.renders() {
        return Outcome::none(field);
    }
    // A template whose slot has no stop here (inside `CD`'s arrow
    // syntax) could not be typed into: it is not inserted.
    let Some(caret) = resolve(new.stops(), target) else {
        return Outcome::none(field);
    };
    new.set_selection(Selection::caret(caret));
    Outcome {
        changes: vec![diff(field.source(), new.source())],
        field: new,
        isolate: false,
        rewrite: None,
        effect: None,
    }
}

/// The outcome of a run of commands as one: the field after the last,
/// with the changes from the first field's source. The runs composed are
/// `run_plain`'s, so there is never a shortcut's rewrite to fold in.
pub fn compose(field: &Field, last: Outcome) -> Outcome {
    debug_assert!(last.rewrite.is_none(), "a composed run expanded a shortcut");
    let changes = if last.field.source() == field.source() {
        Vec::new()
    } else {
        vec![diff(field.source(), last.field.source())]
    };
    Outcome { changes, ..last }
}

fn resolve(stops: &Stops, target: &Target) -> Option<StopId> {
    Some(match target {
        Target::Slot { path, offset } => {
            let slot = stops
                .slots_with_ids()
                .find(|(_, slot)| slot.path == *path)
                .map(|(id, _)| id);
            slot.map_or_else(
                || stops.stop_at(*offset, Affinity::Before),
                |slot| {
                    let ids = &stops.slot(slot).stops;
                    ids.iter()
                        .copied()
                        .find(|&id| stops.offset(id) >= *offset)
                        .unwrap_or(ids[ids.len() - 1])
                },
            )
        }
        Target::After(range) => {
            let slot = stops
                .slots_with_ids()
                .filter(|(_, slot)| {
                    slot.interior.start <= range.start && range.end <= slot.interior.end
                })
                .min_by_key(|(id, slot)| (slot.interior.len(), Reverse(*id)))
                .map(|(id, _)| id);
            slot.map_or_else(
                || stops.stop_at(range.end, Affinity::Before),
                |slot| {
                    let ids = &stops.slot(slot).stops;
                    ids.iter()
                        .copied()
                        .find(|&id| stops.offset(id) >= range.end)
                        .unwrap_or(ids[ids.len() - 1])
                },
            )
        }
        Target::Empty(offset) => empty_at(stops, *offset)?,
        Target::Hole(offset) => {
            empty_at(stops, *offset).or_else(|| resolve(stops, &Target::After(*offset..*offset)))?
        }
    })
}

/// The stop of the empty slot whose content starts at `offset`.
fn empty_at(stops: &Stops, offset: usize) -> Option<StopId> {
    let first = stops.stop_at(offset, Affinity::Before);
    (first.0..stops.stops().len())
        .map(StopId)
        .take_while(|&id| stops.offset(id) == offset)
        .find(|&id| {
            let slot = stops.slot(stops.stop(id).slot);
            slot.is_empty() && slot.interior.start == offset
        })
}

/// The one change that turns `old` into `new`: what lies between their
/// common prefix and suffix, on char boundaries.
pub fn diff(old: &str, new: &str) -> Change {
    let prefix: usize = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum();
    let suffix: usize = old[prefix..]
        .chars()
        .rev()
        .zip(new[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum();
    Change {
        from: prefix,
        to: old.len() - suffix,
        insert: new[prefix..new.len() - suffix].to_owned(),
    }
}

/// The stop of the caret's slot the selection starts at (its lower end),
/// for edits that replace the selection.
fn low(field: &Field) -> StopId {
    let selection = field.selection();
    selection.anchor.min(selection.head)
}

#[cfg(test)]
mod tests {
    use super::diff;
    use crate::field::Change;

    #[test]
    fn diff_trims_the_common_ends() {
        let change = |from, to, insert: &str| Change {
            from,
            to,
            insert: insert.to_owned(),
        };
        assert_eq!(diff("x^2", "x^{23}"), change(2, 3, "{23}"));
        assert_eq!(diff("ab", "ab"), change(2, 2, ""));
        assert_eq!(diff("aab", "ab"), change(1, 2, ""));
        assert_eq!(diff(r"\alpha", r"\alpha x"), change(6, 6, " x"));
    }
}
