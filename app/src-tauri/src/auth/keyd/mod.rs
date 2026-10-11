//! Installs and inspects `oculus-keyd`, the credential broker (its own crate
//! in `app/keyd/`; see docs/architecture.md and docs/development.md).
//!
//! An install registers keyd with the OS through `keyd_core`'s registrar,
//! pointing it at a fixed binary inside keyd's helper app — the copy in
//! `<data_dir>/bin/` for a dev build, or the helper nested in a bundle, which
//! must run in place for its caller check — never at `target/` or a worktree. The source-hash stamp
//! in `<data_dir>/bin/` records what the registration runs, so a rebuild with
//! unchanged source never reinstalls. Nothing here is OS-specific: that is
//! `keyd_core::platform`.

mod candidate;
mod install;
mod status;

pub use candidate::{candidate, installed_stamp, no_candidate_reason, source_hash_of};
pub use install::{ensure_installed, install, install_if_changed, uninstall, Installed};
pub use status::{status, Status};
