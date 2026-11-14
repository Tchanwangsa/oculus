//! The queue's commands. Whether a message still waits is checked by the
//! commit itself, on the row: a message that went out a moment ago is
//! simply no longer there to change, which is no error for the student.

use super::super::commit::Change;
use super::super::state::State;
use super::{Decision, Plan};

fn plan(state: &State, change: Change) -> Result<Decision, String> {
    if state.thread.is_none() {
        return Err("the queue is a thread's".into());
    }
    Ok(Decision::Commit(Plan {
        changes: vec![change],
        text: None,
    }))
}

pub(super) fn edit(state: &State, run_id: i64, text: &str) -> Result<Decision, String> {
    if text.trim().is_empty() {
        return Err("a waiting message cannot be emptied; remove it instead".into());
    }
    let text = text.to_string();
    plan(state, Change::EditQueued { run_id, text })
}

pub(super) fn remove(state: &State, run_id: i64) -> Result<Decision, String> {
    let failed = false;
    plan(state, Change::CloseQueued { run_id, failed })
}

pub(super) fn reorder(state: &State, order: &[i64]) -> Result<Decision, String> {
    let order = order.to_vec();
    plan(state, Change::ReorderQueued { order })
}

/// Stop takes back everything that waits, then the turn is interrupted.
pub(super) fn stop(state: &State) -> Result<Decision, String> {
    plan(state, Change::ClearQueue)
}
