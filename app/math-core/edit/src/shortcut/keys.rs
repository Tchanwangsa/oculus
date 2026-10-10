//! The key run: each key typed in maths lands, joins the run, and may
//! expand it.
//!
//! The run keeps the keys that can still be (the start of) a shortcut,
//! dropping the oldest once they cannot; an expansion keeps them all
//! (`(*` is `(\cdot`, and `)` makes it `\otimes`). After an expansion the
//! run carries on only while its keys lead to a longer shortcut (`sin`,
//! then `h`, is `\sinh`; `<=`, then `>`, `\iff`), typing them into the
//! field as typing alone would have left it too; the first key that leads
//! nowhere starts a new run in the field as it is (`sin`, `t`, `heta` is
//! `\sin\theta`). After Esc reverts an expansion, the keys that still lead
//! to a longer shortcut type as they are (`sin`, Esc, `h` is `sinh`); the
//! first that does not, or any other command, ends that, and the key
//! starts a new run, which may expand.

use std::sync::Arc;

use super::{Run, SHORTCUTS, Step, expand};
use crate::{
    command::{Command, diff},
    field::{Field, Mode, Outcome},
};

/// `key` typed in maths: it lands as typed, then may expand a shortcut.
pub fn typed(field: &Field, key: char) -> Outcome {
    let real = field.without_shortcut();
    let mut landed = real.run_plain(&Command::Insert(key.to_string()));
    landed.field.set_shortcut(None);
    if landed.changes.is_empty() || landed.field.mode() != Mode::Math {
        return landed;
    }
    let run = advance(field.shortcut(), &real, key);
    let expansion = if run.suppressed {
        None
    } else {
        expand::expand(&run, &landed.field)
    };
    match expansion {
        Some(mut expanded) if expanded.source() != landed.field.source() => {
            let rewrite = diff(landed.field.source(), expanded.source());
            let literal = run.literal.unwrap_or_else(|| landed.field.clone());
            expanded.set_shortcut(Some(Run {
                steps: run.steps,
                literal: Some(literal),
                expanded: true,
                suppressed: false,
            }));
            Outcome {
                rewrite: Some(vec![rewrite]),
                field: expanded,
                ..landed
            }
        }
        _ => {
            landed.field.set_shortcut(Some(run));
            landed
        }
    }
}

/// The run after `key`, typed at `real` (the field, without its run).
fn advance(previous: Option<&Run>, real: &Field, key: char) -> Run {
    let fresh = || Run {
        steps: vec![Step {
            key,
            before: Arc::new(real.clone()),
        }],
        literal: None,
        expanded: false,
        suppressed: false,
    };
    let Some(previous) = previous else {
        return fresh();
    };
    let mut keys = previous.keys();
    keys.push(key);
    if !leads_to_shortcut(&keys) && (previous.suppressed || previous.literal.is_some()) {
        return fresh();
    }
    let mut steps = previous.steps.clone();
    if let Some(literal) = &previous.literal {
        let next = literal.run_plain(&Command::Insert(key.to_string()));
        if next.changes.is_empty() || next.field.mode() != Mode::Math {
            return fresh();
        }
        let mut next = next.field;
        next.set_shortcut(None);
        steps.push(Step {
            key,
            before: Arc::new(literal.clone()),
        });
        return Run {
            steps,
            literal: Some(next),
            expanded: false,
            suppressed: false,
        };
    }
    steps.push(Step {
        key,
        before: Arc::new(real.clone()),
    });
    while steps.len() > 1 && !leads_to_shortcut(&steps.iter().map(|s| s.key).collect::<String>()) {
        steps.remove(0);
    }
    Run {
        steps,
        literal: None,
        expanded: false,
        suppressed: previous.suppressed,
    }
}

/// Whether `keys` start some shortcut's keys (or are them).
fn leads_to_shortcut(keys: &str) -> bool {
    SHORTCUTS.iter().any(|(k, _)| k.starts_with(keys))
}
