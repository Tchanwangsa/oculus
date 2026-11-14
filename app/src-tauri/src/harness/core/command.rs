use serde::{Deserialize, Serialize};

use crate::harness::manager::policy::Mode;
use crate::harness::store::RunStart;
use crate::harness::{Provider, SendOptions};

/// One thing the student did to a chat.
pub(in crate::harness) struct Command {
    /// The caller's id for this action. The same id again returns the first
    /// result and changes nothing; on another chat it is refused.
    pub id: String,
    /// The chat it acts on; `None` only for a send that starts one.
    pub thread_id: Option<i64>,
    pub kind: Kind,
}

pub(in crate::harness) enum Kind {
    /// A message: a run that starts now on an idle chat, or waits its turn
    /// in the chat's queue.
    Send {
        provider: Provider,
        text: String,
        options: SendOptions,
        /// How the student wants it delivered to a chat that is mid-turn.
        mode: Mode,
    },
    /// Take the chat back to just before a question; nothing is sent.
    Rewind {
        item_id: i64,
    },
    /// Take the chat back to a question and ask `text` in its place.
    EditResend {
        item_id: i64,
        text: String,
        options: SendOptions,
    },
    DeleteThread,
    /// Rewrite a message that still waits.
    EditQueued {
        run_id: i64,
        text: String,
    },
    /// Take a waiting message out of the queue.
    RemoveQueued {
        run_id: i64,
    },
    /// Put the waiting messages in this order, first to go out first.
    ReorderQueued {
        order: Vec<i64>,
    },
    /// Stop a run and hold what waits behind it. `run_id` names the run
    /// the student saw running: one that has since finished is left alone,
    /// and so is whatever came after it. `None` stops whatever turn the
    /// chat has open, a turn its CLI started itself included.
    Stop {
        run_id: Option<i64>,
    },
    /// Lift the hold on the queue: the waiting messages go out again.
    Resume,
    /// Kill what is answering this run, for a CLI that ignored Stop. It
    /// names the run, so it cannot reach a newer one.
    ForceStop {
        run_id: i64,
    },
}

impl Kind {
    /// The name a receipt records.
    pub fn name(&self) -> &'static str {
        match self {
            Kind::Send { .. } => "send",
            Kind::Rewind { .. } => "rewind",
            Kind::EditResend { .. } => "edit_resend",
            Kind::DeleteThread => DELETE_THREAD,
            Kind::EditQueued { .. } => "edit_queued",
            Kind::RemoveQueued { .. } => "remove_queued",
            Kind::ReorderQueued { .. } => "reorder_queued",
            Kind::Stop { .. } => "stop",
            Kind::Resume => "resume",
            Kind::ForceStop { .. } => "force_stop",
        }
    }
}

/// The one command whose receipt outlives its thread's rows.
pub(in crate::harness) const DELETE_THREAD: &str = "delete_thread";

/// What a command did, stored on its receipt and returned to its caller,
/// the first time and every repeat.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub(in crate::harness) struct Outcome {
    pub thread_id: i64,
    /// The run a send started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunStart>,
    /// The waiting run a send on a busy chat became.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued: Option<i64>,
    /// A rewind's question, handed back to the composer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The run a stop reached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped: Option<i64>,
}
