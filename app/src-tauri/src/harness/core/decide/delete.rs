use super::super::commit::Change;
use super::super::state::State;
use super::{Decision, Plan};

/// A chat is deleted whatever it is doing: its session is ended and
/// everything it left open goes with its rows.
pub(super) fn decide(state: &State) -> Result<Decision, String> {
    if state.thread.is_none() {
        return Err("a delete names a thread".into());
    }
    Ok(Decision::Commit(Plan {
        changes: vec![Change::DeleteThread],
        text: None,
    }))
}
