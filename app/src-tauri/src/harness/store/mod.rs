//! Thread and timeline rows, written here and read by the frontend. Rust owns
//! the writes because events arrive here in order, so a crash mid-tool still
//! leaves a row that says so.

mod apply;
pub mod bindings;
mod items;
mod naming;
pub mod queue;
mod runs;
pub mod threads;

#[cfg(test)]
mod tests;

pub use apply::{apply, apply_event, apply_with_draft, writes, Stored};
pub use bindings::Binding;
pub use items::{newest_anchor, truncate, truncate_from, user_item, Question};
pub use naming::{claim_naming, reconcile, save_rate_limits, NamingSeed, Reconciled};
pub use queue::QueuedMessage;
#[cfg(test)]
pub use runs::begin_run;
pub use runs::{
    begin, bind_session, claim_start, fresh_receipt, last_selection, mark_sent, receipt_run,
    set_draft, settle as settle_run, Owner, RunStart, SendIntent, Settlement, NOT_CLAIMED,
    NO_ANSWER, PART_MISSING,
};
pub use threads::{
    create_thread, delete_thread, raise_revision, revision, thread, LectureRef, ThreadRow,
};
