//! The install routes per provider, and which of them this machine can run.
//!
//! The routes are from the vendors' own documentation (URL beside each); they
//! move, so re-check rather than trust. Vendor-recommended route first.

use serde::{Deserialize, Serialize};

use crate::harness::cli::discover;
use crate::harness::event::Provider;

/// The tools a route can be run with. One route per manager per provider, so
/// this doubles as the route's id over the invoke boundary.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Manager {
    /// The vendor's own install script, piped to a shell.
    Curl,
    Brew,
    Npm,
    Bun,
}

impl Manager {
    /// The binary that has to be on the machine for the route to run.
    fn binary(self) -> &'static str {
        match self {
            Manager::Curl => "curl",
            Manager::Brew => "brew",
            Manager::Npm => "npm",
            Manager::Bun => "bun",
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Manager::Curl => "Install script",
            Manager::Brew => "Homebrew",
            Manager::Npm => "npm",
            Manager::Bun => "bun",
        }
    }
}

/// One way to install one provider.
pub(super) struct Route {
    pub(super) manager: Manager,
    /// Both what is shown and what is run.
    pub(super) command: &'static str,
    /// Must be a directory `discover::well_known_dirs` searches, or a
    /// successful install still reads as missing.
    lands_in: &'static str,
}

/// Claude Code: <https://code.claude.com/docs/en/setup> (script, brew, npm).
/// No bun route: bun skips the npm package's postinstall, which is what
/// links the real binary.
const CLAUDE: &[Route] = &[
    Route {
        manager: Manager::Curl,
        command: "curl -fsSL https://claude.ai/install.sh | bash",
        lands_in: "~/.local/bin",
    },
    Route {
        manager: Manager::Brew,
        command: "brew install --cask claude-code",
        lands_in: "/opt/homebrew/bin",
    },
    Route {
        manager: Manager::Npm,
        command: "npm install -g @anthropic-ai/claude-code",
        lands_in: "npm's global bin",
    },
];

/// Codex: <https://learn.chatgpt.com/docs/codex/cli>,
/// <https://github.com/openai/codex>. No bun route, as for Claude.
const CODEX: &[Route] = &[
    Route {
        manager: Manager::Curl,
        command: "curl -fsSL https://chatgpt.com/codex/install.sh | sh",
        lands_in: "~/.local/bin",
    },
    Route {
        manager: Manager::Brew,
        command: "brew install --cask codex",
        lands_in: "/opt/homebrew/bin",
    },
    Route {
        manager: Manager::Npm,
        command: "npm install -g @openai/codex",
        lands_in: "npm's global bin",
    },
];

/// opencode: <https://opencode.ai/docs/>,
/// <https://github.com/anomalyco/opencode> (tap `anomalyco/tap`, not
/// `sst/tap`). The one vendor that documents bun.
const OPENCODE: &[Route] = &[
    Route {
        manager: Manager::Curl,
        command: "curl -fsSL https://opencode.ai/install | bash",
        lands_in: "~/.opencode/bin",
    },
    Route {
        manager: Manager::Brew,
        command: "brew install anomalyco/tap/opencode",
        lands_in: "/opt/homebrew/bin",
    },
    Route {
        manager: Manager::Npm,
        command: "npm install -g opencode-ai@latest",
        lands_in: "npm's global bin",
    },
    Route {
        manager: Manager::Bun,
        command: "bun install -g opencode-ai@latest",
        lands_in: "~/.bun/bin",
    },
];

/// Antigravity: the vendor ships only its install script — no formula, no
/// npm package.
const ANTIGRAVITY: &[Route] = &[Route {
    manager: Manager::Curl,
    command: "curl -fsSL https://antigravity.google/cli/install.sh | bash",
    lands_in: "~/.local/bin",
}];

pub(super) fn routes(provider: Provider) -> &'static [Route] {
    match provider {
        Provider::Claude => CLAUDE,
        Provider::Codex => CODEX,
        Provider::Opencode => OPENCODE,
        Provider::Antigravity => ANTIGRAVITY,
    }
}

/// Which managers this machine has; a value so [`offer`] is testable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Managers {
    pub curl: bool,
    pub brew: bool,
    pub npm: bool,
    pub bun: bool,
}

impl Managers {
    fn has(&self, m: Manager) -> bool {
        match m {
            Manager::Curl => self.curl,
            Manager::Brew => self.brew,
            Manager::Npm => self.npm,
            Manager::Bun => self.bun,
        }
    }
}

/// What is on this machine, found and cached the way the CLIs are.
pub fn detect() -> Managers {
    let has = |m: Manager| discover::tool(m.binary()).is_some();
    Managers {
        curl: has(Manager::Curl),
        brew: has(Manager::Brew),
        npm: has(Manager::Npm),
        bun: has(Manager::Bun),
    }
}

/// A route as Settings draws it.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstallRoute {
    pub manager: Manager,
    pub label: &'static str,
    pub command: &'static str,
    pub lands_in: &'static str,
    /// Whether the tool it needs is here. An unavailable route is still
    /// shown, to copy, without a button.
    pub available: bool,
}

/// Every way to install one provider on this machine.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstallOffer {
    pub provider: Provider,
    pub label: &'static str,
    /// In the vendor's own order of preference. Never empty.
    pub routes: Vec<InstallRoute>,
    /// Whether any of them can be run from here; false means copy only.
    pub runnable: bool,
}

pub fn offer(provider: Provider, have: Managers) -> InstallOffer {
    let routes: Vec<InstallRoute> = routes(provider)
        .iter()
        .map(|r| InstallRoute {
            manager: r.manager,
            label: r.manager.label(),
            command: r.command,
            lands_in: r.lands_in,
            available: have.has(r.manager),
        })
        .collect();
    InstallOffer {
        provider,
        label: provider.label(),
        runnable: routes.iter().any(|r| r.available),
        routes,
    }
}

/// The literal command for one route, which is the only thing [`start`] will
/// run. `None` for a pairing that does not exist — bun and Claude Code, say.
///
/// [`start`]: super::start
pub fn command_for(provider: Provider, manager: Manager) -> Option<&'static str> {
    routes(provider)
        .iter()
        .find(|r| r.manager == manager)
        .map(|r| r.command)
}
