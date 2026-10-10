//! Esc right after an expansion: the keys go back as typed, the caret
//! after them, as an undo step of its own. Should that field not render
//! (it always does: typing never stops a formula rendering), Esc leaves
//! the field as usual instead.

use super::Run;
use crate::{
    command::diff,
    field::{Field, Outcome},
};

pub fn escape(field: &Field) -> Option<Outcome> {
    let run = field.shortcut().filter(|run| run.expanded)?;
    let literal = run.literal.as_ref()?;
    if field.pending().is_some() || (field.renders() && !literal.renders()) {
        return None;
    }
    let mut back = literal.clone();
    back.set_shortcut(Some(Run {
        steps: run.steps.clone(),
        literal: None,
        expanded: false,
        suppressed: true,
    }));
    Some(Outcome {
        changes: vec![diff(field.source(), back.source())],
        field: back,
        isolate: true,
        rewrite: None,
        effect: None,
    })
}
