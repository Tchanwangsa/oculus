//! Shortcuts: keys typed in maths that expand to LaTeX (`sin` → `\sin`,
//! `->` → `\to`, `@a` → `\alpha`, `sqrt` → `\sqrt{|}`), from `table`.
//!
//! Only one character typed in maths takes part: never text mode, a
//! pending `\command`, an IME's string, a template or a paste, nor a
//! font's or `\operatorname`'s argument (a name, not maths). The key
//! lands as typed (the outcome's `changes`, joining the typing run), then
//! the expansion rewrites it as an undo step of its own (`rewrite`). A
//! letter key expands only when the whole run of letters before the caret
//! is the key (`xsin` stays); a key starting with a symbol matches the
//! keys just typed. A longer key re-expands from the field before the
//! first of its keys (`sin` then `h` is `\sinh`). Esc right after an
//! expansion puts the typed keys back. `keys` keeps the run, `expand`
//! matches it, `revert` is Esc.

mod expand;
mod keys;
mod revert;
mod table;

use std::sync::Arc;

use crate::{
    command::Command,
    field::{Field, Mode, Outcome},
    slot::SlotKind,
};

pub use table::SHORTCUTS;

/// The keys typed in a row in maths, kept on the field between commands.
#[derive(Clone, Debug)]
pub struct Run {
    /// The keys that can still be (part of) a shortcut, each with the
    /// field before it was typed, as typing alone would have left it;
    /// never empty.
    steps: Vec<Step>,
    /// While the field holds an expansion: the field as typing alone would
    /// have left it, which Esc restores and a longer key expands from.
    literal: Option<Field>,
    /// The last command expanded a shortcut.
    expanded: bool,
    /// Esc reverted an expansion and the keys typed since still lead to a
    /// longer key: none of them expands.
    suppressed: bool,
}

#[derive(Clone, Debug)]
struct Step {
    key: char,
    /// With no run of its own; shared, as every key copies the run.
    before: Arc<Field>,
}

impl Run {
    fn keys(&self) -> String {
        self.steps.iter().map(|step| step.key).collect()
    }
}

/// The shortcut layer's outcome for `command`; `None` leaves it to the
/// command itself.
pub fn run(field: &Field, command: &Command) -> Option<Outcome> {
    match command {
        Command::Escape => revert::escape(field),
        Command::Insert(text) if field.mode() == Mode::Math && !in_name(field) => {
            let mut chars = text.chars();
            let key = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            Some(keys::typed(field, key))
        }
        _ => None,
    }
}

/// After an expansion: the field its keys were typed into and the keys,
/// for the check that Esc gives back exactly what typing them would.
pub fn typed_keys(field: &Field) -> Option<(&Field, String)> {
    let run = field.shortcut().filter(|run| run.expanded)?;
    Some((&run.steps[0].before, run.keys()))
}

/// Font and operator-name commands whose argument is a name, where
/// letters are letters (`\mathbb{RR}`, `\operatorname{sinc}`).
const NAMES: &[&str] = &[
    "operatorname",
    "mathrm",
    "mathit",
    "mathsf",
    "mathtt",
    "mathcal",
    "mathbb",
    "mathfrak",
    "mathscr",
];

/// Whether the caret is in the argument of one of [`NAMES`].
fn in_name(field: &Field) -> bool {
    let stops = field.stops();
    let slot = stops.slot(stops.stop(field.selection().head).slot);
    if slot.kind != SlotKind::Body {
        return false;
    }
    let before = field.source()[..slot.interior.start]
        .trim_end_matches('{')
        .trim_end()
        .trim_end_matches('*');
    let rest = before.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    rest.ends_with('\\') && NAMES.contains(&&before[rest.len()..])
}
