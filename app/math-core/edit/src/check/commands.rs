//! Random command runs and the invariants every command keeps, for this
//! crate's property tests and the oracle's `--commands` corpus run.
//!
//! A run starts a field on a formula and applies commands picked by a
//! sequence of numbers (from proptest, or from [`picks`] for a fixed
//! pseudo-random run), checking after each: the new source renders when
//! the old one did, the stops of the new source keep their layout
//! invariants, the selection names real stops, the outcome's change turns
//! the old source into the new, and an insertion undone by Backspace gives
//! the source back (where that rule applies: see `restores`).

use katex::types::ParseError;

use super::layout;
use crate::{
    command::{Command, ends_with_word, mark_at},
    field::{Field, Mode, Outcome, Selection},
    slot::{Bounds, StopId},
};

/// One broken command invariant, at a step of the run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The source rendered before the command and not after.
    Unrenderable(usize),
    /// The new source's stops break a layout invariant.
    Layout(usize, super::Failure),
    /// The selection names a stop the source does not have.
    Selection(usize),
    /// The change does not turn the old source into the new.
    Change(usize),
    /// Backspace after an insertion did not give the source back.
    NoRestore(usize),
}

impl Failure {
    #[must_use]
    pub const fn step(&self) -> usize {
        match self {
            Self::Unrenderable(step)
            | Self::Layout(step, _)
            | Self::Selection(step)
            | Self::Change(step)
            | Self::NoRestore(step) => *step,
        }
    }

    /// The failure's kind, for counting.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Unrenderable(_) => "stops rendering",
            Self::Layout(..) => "stop layout",
            Self::Selection(_) => "selection off the stops",
            Self::Change(_) => "change does not apply",
            Self::NoRestore(_) => "insert then Backspace differs",
        }
    }
}

/// What one run did.
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// Commands applied.
    pub steps: usize,
    /// Commands that changed the source.
    pub edits: usize,
    /// Insertions checked against Backspace.
    pub restores: usize,
    pub failures: Vec<Failure>,
}

/// Typed keys, IME strings and `\command`s the runs pick from.
const TYPED: &[&str] = &[
    "a",
    "x",
    "2",
    "+",
    "=",
    "'",
    "(",
    ")",
    "^",
    "_",
    "/",
    "{",
    "}",
    "#",
    "%",
    "$",
    "&",
    "~",
    "\\",
    " ",
    "\u{e9}",
    "\u{e01}",
    "\u{e31}",
    // A Thai word: an IME string.
    "\u{e2a}\u{e27}\u{e31}\u{e2a}\u{e14}\u{e35}",
    r"\alpha ",
    r"\frac ",
    r"\sqrt ",
    r"\text ",
    r"\zzz ",
    r"\,",
    r"\alpha+",
];

const TEMPLATES: &[&str] = &[
    r"\frac{#0}{#?}",
    r"\sqrt{#0}",
    "^{#?}",
    "_{#?}",
    r"\text{#0}",
    r"\left(#0\right)",
    r"\begin{pmatrix}#?&#?\end{pmatrix}",
    r"\alpha",
    r"\hat{#0}",
];

const PASTES: &[&str] = &["x^2", r"\frac12", r"a\\b", r"\frac{", "}"];

/// How many kinds of command [`command`] picks from.
const KINDS: u64 = 24;

/// The command `pick` stands for at `field`; `None` for a click (the
/// view placing the caret), which [`run`] does itself.
#[must_use]
pub fn command(field: &Field, pick: u64) -> Option<Command> {
    let arg = pick / KINDS;
    let choose = |list: &[&str]| list[(arg % list.len() as u64) as usize].to_owned();
    let xs = || {
        field
            .stops()
            .stops()
            .iter()
            .map(|stop| stop.index as f64)
            .collect()
    };
    let extend = arg % 2 == 1;
    Some(match pick % KINDS {
        0..=5 => Command::Insert(choose(TYPED)),
        6 => Command::Template(choose(TEMPLATES)),
        7 => Command::Paste(choose(PASTES)),
        8 | 9 => Command::Backspace,
        10 => Command::Delete,
        11 => Command::DeleteLine,
        12 | 13 => Command::Left { extend },
        14 | 15 => Command::Right { extend },
        16 => Command::Up { xs: xs() },
        17 => Command::Down { xs: xs() },
        18 if extend => Command::Home { extend },
        18 => Command::End { extend },
        19 => Command::SelectAll,
        20 if extend => Command::ShiftTab,
        20 => Command::Tab,
        21 => Command::Enter,
        22 => Command::Escape,
        _ => return None,
    })
}

/// A fixed pseudo-random sequence of `n` picks from `seed` (xorshift).
#[must_use]
pub fn picks(seed: u64, n: usize) -> Vec<u64> {
    let mut state = seed | 1;
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.push(state);
    }
    out
}

/// A seed from a formula's text (FNV-1a), so each formula's run is fixed.
#[must_use]
pub fn seed(source: &str) -> u64 {
    source.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Runs the commands `picks` choose on `source` from a caret at its end;
/// an error when `source` does not parse.
pub fn run<P: IntoIterator<Item = u64>>(
    source: &str,
    display: bool,
    picks: P,
) -> Result<Report, ParseError> {
    let mut field = Field::new(source, display)?;
    let mut report = Report::default();
    for (step, pick) in picks.into_iter().enumerate() {
        report.steps += 1;
        let Some(command) = command(&field, pick) else {
            let count = field.stops().stops().len() as u64;
            let caret = StopId(((pick / KINDS) % count) as usize);
            field = field.select(Selection::caret(caret));
            continue;
        };
        let outcome = field.run(&command);
        check_outcome(&field, &outcome, step, &mut report.failures);
        if !outcome.changes.is_empty() {
            report.edits += 1;
        }
        if restores(&field, &command, &outcome) {
            report.restores += 1;
            let back = outcome.field.run(&Command::Backspace);
            check_outcome(&outcome.field, &back, step, &mut report.failures);
            if back.field.source() != field.source() {
                report.failures.push(Failure::NoRestore(step));
            }
        }
        field = outcome.field;
    }
    Ok(report)
}

fn check_outcome(before: &Field, outcome: &Outcome, step: usize, failures: &mut Vec<Failure>) {
    let after = &outcome.field;
    if before.renders() && !after.renders() {
        failures.push(Failure::Unrenderable(step));
    }
    let mut source = before.source().to_owned();
    for change in outcome.changes.iter().rev() {
        source.replace_range(change.from..change.to, &change.insert);
    }
    if source != after.source() {
        failures.push(Failure::Change(step));
    }
    let count = after.stops().stops().len();
    let selection = after.selection();
    if selection.anchor.0 >= count || selection.head.0 >= count {
        failures.push(Failure::Selection(step));
    }
    if !outcome.changes.is_empty() {
        let layout = layout(after.source(), after.stops());
        failures.extend(
            layout
                .failures
                .into_iter()
                .map(|failure| Failure::Layout(step, failure)),
        );
    }
}

/// Whether Backspace right after `command` must give `field`'s source
/// back: a single typed character or a template typed at a caret, that
/// changed the source. Not when it went into a bare argument (which took
/// braces), after a control word's space and before a non-letter (where
/// Backspace drops that space as glue), as a control word before a space
/// (which Backspace deletes with it), as `/` (whose fraction takes the
/// term before it) or as a prime (which joins a script or macro output
/// next to it into one atom); nor a combining mark or a character before
/// one (Backspace takes the whole cluster).
fn restores(field: &Field, command: &Command, outcome: &Outcome) -> bool {
    let selection = field.selection();
    if outcome.changes.is_empty()
        || !selection.is_caret()
        || field.mode() == Mode::Command
        || outcome.field.pending().is_some()
    {
        return false;
    }
    let stops = field.stops();
    let slot = stops.slot(stops.stop(selection.head).slot);
    if slot.bounds == Bounds::Bare {
        return false;
    }
    let offset = stops.offset(selection.head);
    let source = field.source();
    let before = &source[..offset];
    let after = &source[offset..];
    let glue_space = before.strip_suffix(' ').is_some_and(ends_with_word)
        && after
            .chars()
            .next()
            .is_none_or(|c| !c.is_ascii_alphabetic() && !c.is_whitespace());
    // A control word typed before a space takes that space as its own
    // (the parse gives it to the word), and Backspace deletes it with it.
    let new = &outcome.field;
    let caret = new.stops().offset(new.selection().head);
    let word_takes_space =
        after.starts_with(char::is_whitespace) && ends_with_word(new.source()[..caret].trim_end());
    if glue_space || word_takes_space || mark_at(source, offset) {
        return false;
    }
    match command {
        Command::Template(_) => true,
        Command::Insert(text) => {
            let mut chars = text.chars();
            chars.next().is_some_and(|c| !matches!(c, '/' | '\''))
                && chars.next().is_none()
                && !mark_at(text, 0)
        }
        _ => false,
    }
}
