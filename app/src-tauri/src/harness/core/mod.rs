//! One path for everything the student does to a chat: a [`Command`] with
//! an id is decided against what is saved ([`decide`], which writes nothing
//! and calls no provider), its changes are saved in one transaction
//! ([`commit`], the only place a command writes), and only then is the
//! webview told and the provider called ([`effect`]). The consumer
//! (`app/consumer/`) runs each step, so a chat's commands and its provider's
//! events are one ordered stream. docs/harness.md, "One command path".

pub(in crate::harness) mod command;
pub(in crate::harness) mod commit;
pub(in crate::harness) mod decide;
pub(in crate::harness) mod effect;
pub(in crate::harness) mod load;
pub(in crate::harness) mod receipts;
pub(in crate::harness) mod state;

pub(in crate::harness) use command::{Command, Kind, Outcome};
