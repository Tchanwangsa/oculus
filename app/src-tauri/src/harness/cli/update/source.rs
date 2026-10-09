//! How a CLI got onto the machine, and the command that updates it.

use std::path::Path;

use serde::Serialize;

use crate::harness::event::Provider;

/// How a CLI got onto the machine, read from where its binary really lives.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Brew,
    Npm,
    Bun,
    /// The vendor's script or native installer; the CLI updates itself.
    SelfManaged,
}

/// The source of a binary from its canonical path. A Homebrew formula can
/// keep a node package under `Cellar/…/node_modules`, so `Caskroom`/`Cellar`
/// is checked first; npm's global prefix under `/opt/homebrew/lib` is npm's.
pub fn source_of(real: &Path, home: Option<&Path>) -> Source {
    let has = |name: &str| real.components().any(|c| c.as_os_str() == name);
    if has("Caskroom") || has("Cellar") {
        return Source::Brew;
    }
    if let Some(h) = home {
        if real.starts_with(h.join(".bun")) {
            return Source::Bun;
        }
    }
    if has("node_modules") {
        return Source::Npm;
    }
    if real.starts_with("/opt/homebrew") || real.starts_with("/home/linuxbrew/.linuxbrew") {
        return Source::Brew;
    }
    Source::SelfManaged
}

/// `'…'`, with any `'` closed, escaped and reopened.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The one command [`start`](super::start) will run for this provider. A
/// pairing with no package-manager route (Antigravity from npm, say) falls
/// back to the CLI's own subcommand.
pub fn command(provider: Provider, source: Source, bin: &Path) -> String {
    let literal = match (provider, source) {
        (Provider::Claude, Source::Brew) => Some("brew upgrade --cask claude-code"),
        (Provider::Codex, Source::Brew) => Some("brew upgrade --cask codex"),
        (Provider::Opencode, Source::Brew) => Some("brew upgrade anomalyco/tap/opencode"),
        (Provider::Claude, Source::Npm) => Some("npm install -g @anthropic-ai/claude-code@latest"),
        (Provider::Codex, Source::Npm) => Some("npm install -g @openai/codex@latest"),
        (Provider::Opencode, Source::Npm) => Some("npm install -g opencode-ai@latest"),
        (Provider::Opencode, Source::Bun) => Some("bun install -g opencode-ai@latest"),
        _ => None,
    };
    if let Some(c) = literal {
        return c.to_string();
    }
    let sub = match provider {
        Provider::Opencode => "upgrade",
        Provider::Claude | Provider::Codex | Provider::Antigravity => "update",
    };
    format!("{} {sub}", shell_quote(&bin.display().to_string()))
}

/// The source [`command`] actually uses, so the version compared against is
/// the one that command installs.
pub(super) fn effective_source(provider: Provider, source: Source) -> Source {
    match (provider, source) {
        (Provider::Antigravity, _) => Source::SelfManaged,
        (Provider::Claude | Provider::Codex, Source::Bun) => Source::SelfManaged,
        _ => source,
    }
}
