//! Deciders: one file per command. Each reads a [`State`] and returns what
//! to save ([`Change`]) or what to ask the provider first ([`Ask`]). None
//! writes anything, and none waits on a provider: that is the commit's work
//! and the effects'.

mod delete;
mod queue;
pub(in crate::harness) mod respond;
mod rewind;
mod select;
mod send;
pub(in crate::harness) mod steer;

use crate::harness::{Provider, SendOptions};

use super::command::{Command, Kind};
use super::commit::Change;
use super::state::State;

/// What a command comes to.
pub(in crate::harness) enum Decision {
    /// Everything is known: save these.
    Commit(Plan),
    /// The provider has to answer first; the command is decided again with
    /// its [`Answer`].
    Ask(Ask),
}

/// The changes one command commits, and what its caller is handed back.
#[derive(Default)]
pub(in crate::harness) struct Plan {
    pub changes: Vec<Change>,
    /// A rewind's question, for the composer.
    pub text: Option<String>,
}

/// A question only the provider can answer, asked before the commit.
pub(in crate::harness) enum Ask {
    /// Forget this question and everything after it.
    Rewind(Box<RewindAsk>),
}

pub(in crate::harness) struct RewindAsk {
    pub thread_id: i64,
    pub provider: Provider,
    /// The provider's handle for the question to take out.
    pub anchor: String,
    /// The newest question's handle.
    pub last_seen: Option<String>,
    /// The session to rewind, resumed if its process has gone.
    pub resume: String,
    /// What a respawn for the rewind binds: the model and reasoning level.
    pub options: SendOptions,
}

/// The provider's answer to an [`Ask`].
#[derive(Clone, Copy, Debug)]
pub(in crate::harness) enum Answer {
    /// Whether its context went back with the rows.
    Rewound { context: bool },
}

/// Decide `command` against `state`. An `Err` is a refusal the student can
/// act on; nothing is recorded for it, so the same id may be tried again.
pub(in crate::harness) fn decide(
    state: &State,
    command: &Command,
    answer: Option<Answer>,
) -> Result<Decision, String> {
    match &command.kind {
        Kind::Send {
            provider,
            text,
            options,
            mode,
        } => send::decide(state, command, (*provider, *mode), text, options),
        Kind::Rewind { item_id } => rewind::decide(state, command, *item_id, None, answer),
        Kind::EditResend {
            item_id,
            text,
            options,
            provider,
        } => {
            let resend = rewind::Resend {
                text,
                options,
                provider: *provider,
            };
            rewind::decide(state, command, *item_id, Some(resend), answer)
        }
        Kind::SelectModel {
            provider,
            model,
            effort,
        } => select::decide(state, *provider, model.as_ref(), effort.as_ref()),
        Kind::CompleteThread | Kind::ReopenThread => {
            state.thread.as_ref().ok_or("completion with no thread")?;
            Ok(Decision::Commit(Plan {
                changes: vec![Change::SetCompleted {
                    completed: matches!(command.kind, Kind::CompleteThread),
                }],
                text: None,
            }))
        }
        Kind::DeleteThread => delete::decide(state),
        Kind::EditQueued { run_id, text } => queue::edit(state, *run_id, text),
        Kind::RemoveQueued { run_id } => queue::remove(state, *run_id),
        Kind::ReorderQueued { order } => queue::reorder(state, order),
        Kind::SteerQueued { run_id } => steer::send_now(state, *run_id),
        Kind::Stop { run_id } => queue::stop(state, *run_id),
        Kind::Resume => queue::resume(state),
        Kind::ForceStop { run_id } => queue::force_stop(state, *run_id),
        Kind::Respond {
            request_id,
            option_id,
            feedback,
            answers,
        } => respond::decide(
            state,
            command,
            *request_id,
            option_id,
            feedback.as_ref(),
            answers.as_deref(),
        ),
        Kind::SetPermissionMode { mode } => respond::mode(state, *mode),
    }
}
