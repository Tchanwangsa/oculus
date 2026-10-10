//! Updating an installed CLI agent, from Settings → Agents.
//!
//! The discovered binary's real path says how it was installed ([`Source`]);
//! that picks both the command that updates it and the free, unauthenticated
//! endpoint its newest version is read from — a Homebrew install is compared
//! against Homebrew's own version, which can trail npm's. Commands are built
//! here from literal strings; the webview names only a provider.
//!
//! Updates run one at a time across every provider (brew and npm take global
//! locks) through [`install::run_command`](super::install), and stream on
//! [`UPDATE_EVENT`] in [`InstallLine`](super::install::InstallLine)'s shape.
//! See `docs/harness.md`.

mod fetch;
mod run;
mod source;
#[cfg(test)]
mod tests;
mod version;

pub use fetch::forget;
pub use run::{check, check_all, start, UpdateInfo, UPDATE_EVENT};
pub use source::{command, source_of, Source};
pub use version::is_newer;
