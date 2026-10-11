//! Projects: boards, tasks and one level of subtask, written headlessly — the
//! same SQL as `app/src/lib/planning/projects/`, so the `oculus` CLI (and the chat
//! agent) can plan work the board picks up. Change a table and both writers
//! change. Rules mirrored from that module:
//!
//! - **A column id is checked against a board** ({@link board_of}): a task in a
//!   column no view renders is invisible, and `--column` is free text.
//! - **`done_at` is derived from the destination column's `kind`**, never
//!   passed in, on create and on move.
//! - **Subtasks are one level deep**, enforced in code (SQLite cannot express it).
//! - **A task may belong to no project** (`project_id` NULL): its board is the
//!   default one.
//!
//! Schema: migrations 27, 33 and 37 in `db/migrations.rs`. Nothing here creates it.

mod columns;
mod migration37;
mod project_writes;
mod read;
mod rows;
mod tasks;
#[cfg(test)]
mod tests;
mod time;
#[cfg(test)]
mod unfiled_tests;

pub use columns::*;
pub use migration37::UNFILED_TASKS_SQL;
pub use project_writes::*;
pub use read::*;
pub use rows::*;
pub use tasks::*;
pub use time::*;
