//! Durable chat cards. Lifecycle transitions and the writes they authorize
//! share one SQLite transaction; the journal retains the affected rows for Undo.

mod amendments;
mod apply;
mod changes;
mod cheats;
mod edits;
mod inspect;
mod journal;
mod lifecycle;
mod mutations;
mod notes;
mod state;
#[cfg(test)]
mod tests;

pub use apply::{apply, apply_in, undo, undo_in};
pub use inspect::inspect;
pub use lifecycle::{discard, edit, get, list, open};
pub use state::{Made, Proposal};
