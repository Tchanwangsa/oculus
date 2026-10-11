//! Installing a missing CLI agent, from Settings → AI.
//!
//! Every route is a literal vendor command, shown verbatim with a Copy
//! button; the click on Run is the confirmation. The webview names only a
//! provider and a manager — never the string handed to `$SHELL -lc` — and
//! nothing needing `sudo` is offered or run. macOS routes only, matching
//! `discover`.
//!
//! Commands run through a login shell because a Dock-launched app lacks the
//! profile's PATH. The discovery caches are not dropped here: the frontend
//! rechecks on the `done` event, which invalidates Rust's cache and its own
//! (`app/src/hooks/agents/useBridgeHealth.ts`) together.

mod routes;
mod run;
#[cfg(test)]
mod tests;

pub use routes::{command_for, detect, offer, InstallOffer, InstallRoute, Manager, Managers};
pub use run::{start, InstallLine, Line, INSTALL_EVENT};

pub(in crate::harness::cli) use run::{drain_lines, exit_text, run_command};
