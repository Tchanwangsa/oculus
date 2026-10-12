//! Bringing a workspace folder with a sync record in step with Ed, per file,
//! from its bytes here, the record's hash and Ed's bytes (`table`). It runs
//! on every connect of a session, when another client commits while the
//! session is live (`pull`), and in the lesson sync (`sync`). A file both
//! sides changed differently is a conflict: Ed's version goes beside it
//! (`server_copy`) until the pane settles it (`resolve`). `docs/sync.md`.

pub mod command;
mod compare;
mod in_session;
mod pull;
mod registry;
mod resolve;
mod server_copy;
mod sync;
mod table;

#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod tests;

pub use compare::{Head, Outcome};
pub use in_session::{reconcile, Removes};
pub use pull::{spawn as spawn_puller, Puller};
pub use registry::{closed, opened, wait_for_sync};
pub use resolve::{resolve, Keep};
pub use server_copy::is_copy as is_server_copy;
pub use sync::{sync, Synced};
