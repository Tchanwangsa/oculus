//! Thread and timeline rows, written here and read by the frontend. Rust owns
//! the writes because events arrive here in order, so a crash mid-tool still
//! leaves a row that says so.

mod apply;
mod items;
mod naming;
mod runs;
pub mod threads;

#[cfg(test)]
mod tests;

pub use apply::{apply, apply_event, Stored};
pub use items::{newest_anchor, truncate_from, user_item, Question};
pub use naming::{claim_naming, reconcile, save_rate_limits, NamingSeed, Reconciled};
pub use runs::{
    append_draft, begin as begin_run, bind_session, clear_draft, draft, fresh_receipt, mark_sent,
    receipt_run, settle as settle_run, Owner, RunStart, SendIntent, Settlement, NOT_CLAIMED,
    NO_ANSWER, PART_MISSING,
};
pub use threads::{create_thread, delete_thread, set_model, thread, LectureRef, ThreadRow};
