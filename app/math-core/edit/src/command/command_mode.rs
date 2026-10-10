//! Command mode: `\` starts a command that lives outside the source (the
//! view draws it as a chip) until it is committed, so the source always
//! parses.
//!
//! Letters extend the name; Backspace shortens it and, once it is empty,
//! cancels; Esc cancels. Space, Enter, Tab or any other character
//! commits: `\name` goes in with its arguments as empty slots (the first
//! taking the selection), and that other character is then typed as
//! usual. A name that renders with none of zero to three arguments, or
//! that does not fit where the caret is, is not inserted and stays
//! pending, so the view can show it as wrong. Right after `\`, a
//! non-letter makes a control symbol (`\,`, `\{`, `\ `, `\\`) when that
//! renders, else the command is dropped and the character typed. A text
//! command (`\text`, `\textbf`, `\mbox`, …) leaves the caret in its text
//! run: text mode.

use super::{Command, compose, template::template};
use crate::{
    field::{Field, Outcome},
    parse::renders,
};

/// The most arguments a command is probed with.
const MAX_ARGUMENTS: usize = 3;

/// Command mode's answer to `command`, or `None` when the command is not
/// command mode's (the pending command is then dropped first).
pub fn run(field: &Field, command: &Command) -> Option<Outcome> {
    let name = field.pending()?.to_owned();
    Some(match command {
        Command::Insert(text) => typed(field, text),
        Command::Backspace if name.is_empty() => cancel(field),
        Command::Backspace => {
            let mut shorter = name;
            shorter.pop();
            Outcome::moved(field.clone().with_pending(Some(shorter)))
        }
        Command::Enter | Command::Tab => commit(field, &name, None),
        Command::Escape => cancel(field),
        _ => return None,
    })
}

fn cancel(field: &Field) -> Outcome {
    Outcome::moved(field.clone().with_pending(None))
}

/// Each character in turn: letters extend the pending name, anything else
/// commits it; after a commit the rest is typed as usual.
fn typed(field: &Field, text: &str) -> Outcome {
    let mut outcome = Outcome::none(field);
    for c in text.chars() {
        let current = &outcome.field;
        let next = match current.pending() {
            Some(name) if c.is_ascii_alphabetic() => {
                let mut longer = name.to_owned();
                longer.push(c);
                Outcome::moved(current.clone().with_pending(Some(longer)))
            }
            Some(name) => commit(current, name, Some(c)),
            None => current.run(&Command::Insert(c.to_string())),
        };
        outcome = Outcome {
            effect: next.effect,
            ..compose(field, next)
        };
    }
    outcome
}

/// Commits the pending `name`, then types `then` (a Space is used up by
/// the commit).
fn commit(field: &Field, name: &str, then: Option<char>) -> Outcome {
    let base = field.clone().with_pending(None);
    if name.is_empty() {
        return symbol(field, &base, then);
    }
    let arguments = (0..=MAX_ARGUMENTS).find(|&n| {
        let probe = format!(r"\{name}{}", "{}".repeat(n));
        renders(&probe, field.display()).is_ok()
    });
    let snippet = arguments.map_or_else(
        || format!(r"\{name}"),
        |n| {
            let mut snippet = format!(r"\{name}");
            for i in 0..n {
                snippet.push_str(if i == 0 { "{#0}" } else { "{#?}" });
            }
            snippet
        },
    );
    let inserted = template(&base, &snippet);
    if inserted.changes.is_empty() {
        return Outcome::none(field);
    }
    match then {
        Some(c) if c != ' ' => {
            let next = inserted.field.run(&Command::Insert(c.to_string()));
            Outcome {
                effect: next.effect,
                ..compose(field, next)
            }
        }
        _ => inserted,
    }
}

/// `\` then a non-letter: the control symbol when it renders, else the
/// character typed as usual. Enter or Tab right after `\` cancel it.
fn symbol(field: &Field, base: &Field, then: Option<char>) -> Outcome {
    let Some(c) = then else {
        return Outcome::moved(base.clone());
    };
    let symbol = format!(r"\{c}");
    if !c.is_control() && renders(&symbol, field.display()).is_ok() {
        let inserted = template(base, &symbol);
        if !inserted.changes.is_empty() {
            return inserted;
        }
    }
    let next = base.run(&Command::Insert(c.to_string()));
    Outcome {
        effect: next.effect,
        ..compose(field, next)
    }
}
