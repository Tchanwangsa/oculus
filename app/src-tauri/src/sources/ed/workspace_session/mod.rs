//! The editor's live session on an Ed challenge workspace: a socket thread
//! that keeps the workspace open on Ed's server, and the commands that read,
//! save and delete its files. The protocol, the state file and the event
//! stream are described in `docs/sync.md`.

mod bootstrap;
pub mod commands;
mod commits;
mod context;
pub mod events;
pub mod frames;
mod paths;
mod queue;
mod session;
mod slide;
mod socket;

#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod tests;

pub use commands::EdWorkspaces;
