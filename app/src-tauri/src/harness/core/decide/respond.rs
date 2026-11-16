//! The student's answer to a request, and how the chat asks from now on.
//! An answer is checked against the saved request and its live route
//! here; the commit's conditional write is what claims it, so of two
//! answers racing from two panes one lands and the other is refused.

use crate::harness::requests::{PermissionMode, Status};

use super::super::commit::Change;
use super::super::state::State;
use super::{Decision, Plan};

pub(in crate::harness) const NOT_HERE: &str = "This request is not in this chat.";
pub(in crate::harness) const ANSWERED: &str = "This request was already answered.";
pub(in crate::harness) const GONE: &str =
    "The agent that asked has gone, so this request can no longer be answered.";
pub(in crate::harness) const NO_SUCH_OPTION: &str = "This request does not offer that answer.";
pub(in crate::harness) const NO_FEEDBACK: &str =
    "This request cannot take a message with its answer.";
pub(in crate::harness) const MODE_WHILE_PENDING: &str =
    "Answer the waiting request before changing how this chat asks.";

pub(super) fn decide(
    state: &State,
    command: &super::super::command::Command,
    request_id: i64,
    option_id: &str,
    feedback: Option<&String>,
    answers: Option<&[crate::harness::requests::QuestionAnswer]>,
) -> Result<Decision, String> {
    let request = state.request.as_ref().ok_or(NOT_HERE)?;
    match request.status {
        Status::Pending => {}
        Status::Resolved => return Err(ANSWERED.into()),
        Status::Expired | Status::Cancelled => {
            let reason = request.reason.as_deref().unwrap_or("It has ended.");
            return Err(format!("This request can no longer be answered. {reason}"));
        }
    }
    let message = request.kind == "question" && request.responder == "message";
    if !message && (request.responder != "live" || !state.live.route) {
        return Err(GONE.into());
    }
    if !request.options.iter().any(|o| o.id == option_id) {
        return Err(NO_SUCH_OPTION.into());
    }
    if feedback.is_some_and(|f| !f.trim().is_empty()) {
        return Err(NO_FEEDBACK.into());
    }
    let answers = answers.unwrap_or_default();
    if request.kind == "question" {
        crate::harness::requests::QuestionPayload::read(&request.payload)?
            .validate(option_id, answers)?;
    } else if !answers.is_empty() {
        return Err("An approval cannot take question answers.".into());
    }
    let mut changes = vec![Change::Respond {
        request_id,
        option_id: option_id.to_string(),
        answers: (request.kind == "question").then(|| answers.to_vec()),
    }];
    if message && option_id == "answer" {
        let payload = crate::harness::requests::QuestionPayload::read(&request.payload)?;
        let text = payload.message(answers);
        if state.held > 0 {
            changes.push(Change::Unhold);
        }
        changes.push(if state.busy() || state.waiting > 0 {
            Change::Enqueue {
                text,
                options: Default::default(),
                front: false,
            }
        } else {
            Change::Begin {
                text,
                options: Default::default(),
                receipt: command.id.clone(),
                queued: None,
            }
        });
    }
    Ok(Decision::Commit(Plan {
        changes,
        text: None,
    }))
}

pub(super) fn mode(state: &State, mode: PermissionMode) -> Result<Decision, String> {
    state.thread.as_ref().ok_or("a mode names a thread")?;
    if state.pending > 0 {
        return Err(MODE_WHILE_PENDING.into());
    }
    Ok(Decision::Commit(Plan {
        changes: vec![Change::SetMode { mode }],
        text: None,
    }))
}
