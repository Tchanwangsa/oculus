//! Drive's folder commands: the student's own files and folders created,
//! renamed, moved, copied and deleted, and the target folders every own-file
//! creator takes (`own_target`).
//!
//! Every path is data-dir-relative and passes `layout::own_path_ok` (a target
//! may also be a subject's root); synced and hidden paths are `read_only`.
//! The disk moves first, under a fence that refuses files a parse or embed
//! is running on or a device shares (`busy`); then the `files` rows follow
//! by id in one transaction, so pages, versions and task links stay; if they
//! cannot, the disk goes back (`journal`). A parsed file carries its
//! artifacts (`companions`), a note its pictures (`pictures`). Own files
//! that arrived without a row (an agent's, Finder's) get one (`reconcile`).
//! docs/frontend.md has the commands' contract.

mod busy;
pub(crate) mod commands;
mod companions;
mod copy;
mod create;
mod delete;
pub(crate) mod error;
mod events;
pub(crate) mod guard;
mod journal;
mod moves;
pub(crate) mod names;
mod pictures;
pub(crate) mod reconcile;
mod relink;
mod rows;

#[cfg(test)]
mod tests;
