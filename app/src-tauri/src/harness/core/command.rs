use serde::{Deserialize, Serialize};

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
    /// A message: a run that starts now on an idle chat, or waits its turn.
    Send {
        provider: Provider,
        text: String,
        options: SendOptions,
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
}

impl Kind {
    /// The name a receipt records.
    pub fn name(&self) -> &'static str {
        match self {
            Kind::Send { .. } => "send",
            Kind::Rewind { .. } => "rewind",
            Kind::EditResend { .. } => "edit_resend",
            Kind::DeleteThread => DELETE_THREAD,
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
    /// A rewind's question, handed back to the composer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}
