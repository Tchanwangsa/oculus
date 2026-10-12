//! The editor's live session on an Ed challenge workspace: a socket thread
//! that keeps the workspace open on Ed's server, and the commands that read,
//! save and delete its files, its terminals, Run and Mark. The protocol,
//! the state file and the event stream are described in `docs/sync.md`.

mod bootstrap;
pub mod commands;
mod commits;
mod context;
pub mod events;
pub mod frames;
pub mod mark;
pub mod mark_commands;
mod paths;
mod queue;
pub mod run;
pub mod run_commands;
mod session;
mod slide;
mod snapshot;
mod socket;
mod terminal;
pub mod terminal_commands;

#[cfg(test)]
mod mark_tests;
#[cfg(test)]
mod program_tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod tests;

pub use commands::EdWorkspaces;
