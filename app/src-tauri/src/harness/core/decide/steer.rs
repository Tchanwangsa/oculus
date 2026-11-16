//! A message written into the running run: the pieces `Send` shares with
//! "Send now" (`SteerQueued`), which takes a waiting message out of the
//! queue and writes it in, keeping its identity.

use crate::harness::manager::policy::{resolve_delivery, Delivery, Mode};
use crate::harness::store::steer::Target;

use super::super::commit::Change;
use super::super::state::{OpenRun, State};
use super::{Decision, Plan};

pub(in crate::harness) const NOTHING_RUNNING: &str =
    "No turn is running to send this into. It goes out when its turn comes.";
pub(in crate::harness) const CANNOT_STEER: &str =
    "This agent cannot take a message mid-turn. It goes out when the turn ends.";
pub(in crate::harness) const NOT_YET: &str =
    "The agent has not started this turn yet. Try again in a moment.";
pub(in crate::harness) const STOPPING: &str =
    "This turn is being stopped. The message goes out when you resume.";
pub(in crate::harness) const QUESTION_WAITS: &str =
    "The agent is waiting on your answer to its question. Answer it first.";
pub(in crate::harness) const COMMAND: &str = "A command is never sent into a running turn.";

/// The run a steer joins: its row goes in the open run's active attempt.
pub(super) fn target(open: OpenRun) -> Target {
    Target {
        run_id: open.run_id,
        attempt_id: open.attempt_id,
        binding_id: open.binding_id,
    }
}

/// Every approval the open run waits on, declined: a message sent while
/// one waits is the student's answer to it.
pub(super) fn declines(state: &State) -> Vec<Change> {
    state
        .approvals
        .iter()
        .map(|(request_id, option_id)| Change::Respond {
            request_id: *request_id,
            option_id: option_id.clone(),
            answers: None,
        })
        .collect()
}

/// "Send now" on a waiting message: written into the running run as a
/// steer, or a refusal that says why it cannot be, which records nothing.
pub(super) fn send_now(state: &State, run_id: i64) -> Result<Decision, String> {
    state.thread.as_ref().ok_or("a steer names a thread")?;
    // A message that has just gone out on its own is no longer the
    // queue's to send: nothing changes, and it is no error.
    let Some(text) = state.queued.as_deref() else {
        return Ok(Decision::Commit(Plan::default()));
    };
    let open = state.open.ok_or(NOTHING_RUNNING)?;
    if state.questions > 0 {
        return Err(QUESTION_WAITS.into());
    }
    let slash = text.trim_start().starts_with('/');
    let caps = state.live.steering.of(open.provider);
    let restart = match resolve_delivery(Mode::Steer, &state.chat(), &caps, slash) {
        Delivery::Steer => false,
        Delivery::Restart => true,
        _ if slash => return Err(COMMAND.into()),
        _ if open.stop_requested => return Err(STOPPING.into()),
        _ if open.starting || !state.live.accepted => return Err(NOT_YET.into()),
        _ => return Err(CANNOT_STEER.into()),
    };
    let mut changes = declines(state);
    changes.push(Change::Steer {
        text: text.to_string(),
        options: Default::default(),
        target: target(open),
        from: Some(run_id),
        restart,
    });
    Ok(Decision::Commit(Plan {
        changes,
        text: None,
    }))
}
