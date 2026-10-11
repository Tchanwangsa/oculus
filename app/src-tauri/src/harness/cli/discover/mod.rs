//! Finding the provider CLIs from inside a GUI app.
//!
//! A Dock-launched app inherits launchd's PATH (`/usr/bin`, `/bin`), so
//! `~/.local/bin/claude` or `/opt/homebrew/bin/codex` would not resolve. So:
//! an explicit override, PATH, the installers' usual dirs, then a login shell.
//! Results are cached for the process. macOS-shaped on purpose (no Windows
//! `.cmd`/`PATHEXT`), like [`install`](super::install).

mod env;
mod health;
mod locate;
mod oculus_bin;
#[cfg(test)]
mod tests;

pub use env::child_env;
pub use health::{forget_health, health, BridgeHealth};
pub use locate::{binary, forget, tool};
pub use oculus_bin::oculus_cli;

use crate::harness::event::Provider;

/// The env var that points discovery at a build somewhere unusual.
pub fn override_env(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "OCULUS_CLAUDE_BIN",
        Provider::Codex => "OCULUS_CODEX_BIN",
        Provider::Opencode => "OCULUS_OPENCODE_BIN",
        Provider::Antigravity => "OCULUS_ANTIGRAVITY_BIN",
    }
}

fn binary_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "claude",
        Provider::Codex => "codex",
        Provider::Opencode => "opencode",
        // The one place Antigravity's binary name is written.
        Provider::Antigravity => "agy",
    }
}

/// Every provider, in the order Settings lists them. A provider missing here
/// is a bridge nobody can find.
pub const PROVIDERS: [Provider; 4] = [
    Provider::Claude,
    Provider::Codex,
    Provider::Opencode,
    Provider::Antigravity,
];
