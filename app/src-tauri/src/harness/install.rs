//! Installing a missing CLI agent, from Settings → AI.
//!
//! Every route is a literal vendor command, shown verbatim with a Copy
//! button; the click on Run is the confirmation. The webview names only a
//! provider and a manager — never the string handed to `$SHELL -lc` — and
//! nothing needing `sudo` is offered or run. macOS routes only, matching
//! `discover.rs`.
//!
//! Commands run through a login shell because a Dock-launched app lacks the
//! profile's PATH. The discovery caches are not dropped here: the frontend
//! rechecks on the `done` event, which invalidates Rust's cache and its own
//! (`app/src/hooks/useBridgeHealth.ts`) together.

use std::io::{BufRead, BufReader};
use std::process::{ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use super::discover;
use super::event::Provider;

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

    fn label(self) -> &'static str {
        match self {
            Manager::Curl => "Install script",
            Manager::Brew => "Homebrew",
            Manager::Npm => "npm",
            Manager::Bun => "bun",
        }
    }
}

/// One way to install one provider.
struct Route {
    manager: Manager,
    /// Both what is shown and what is run.
    command: &'static str,
    /// Must be a directory `discover::well_known_dirs` searches, or a
    /// successful install still reads as missing.
    lands_in: &'static str,
}

// ── The routes ───────────────────────────────────────────────────────────────
//
// From the vendors' own documentation (URL beside each); they move, so
// re-check rather than trust. Vendor-recommended route first.

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

fn routes(provider: Provider) -> &'static [Route] {
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
pub fn command_for(provider: Provider, manager: Manager) -> Option<&'static str> {
    routes(provider)
        .iter()
        .find(|r| r.manager == manager)
        .map(|r| r.command)
}

/// Where an install's output reaches the webview; the dialog filters by
/// provider.
pub const INSTALL_EVENT: &str = "harness-install";

/// What the webview gets, line by line, while an install runs.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallLine {
    pub provider: Provider,
    /// One line of the child's output, stdout and stderr both.
    pub line: Option<String>,
    /// The last event of a run, and the one the frontend rechecks on.
    pub done: bool,
    pub ok: Option<bool>,
    /// How it ended, for the line the dialog draws when it did not end well.
    pub status: Option<String>,
}

/// One install at a time per provider.
fn running() -> &'static Mutex<std::collections::HashSet<Provider>> {
    static R: OnceLock<Mutex<std::collections::HashSet<Provider>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Run a route, streaming its output to `emit` and closing with one `done`
/// line. Returns as soon as the child is spawned; everything after that
/// happens on the supervisor thread.
pub fn start<F>(provider: Provider, manager: Manager, emit: F) -> Result<(), String>
where
    F: Fn(InstallLine) + Send + 'static,
{
    let command = command_for(provider, manager).ok_or_else(|| {
        format!(
            "there is no {} route for {}",
            manager.label(),
            provider.label()
        )
    })?;

    {
        let mut r = running().lock().unwrap();
        if !r.insert(provider) {
            return Err(format!("{} is already installing", provider.label()));
        }
    }

    let finish = move |p: Provider| {
        running().lock().unwrap().remove(&p);
    };

    match run_command(command, move |line| {
        let done = line.done;
        emit(InstallLine {
            provider,
            line: line.line,
            done,
            ok: line.ok,
            status: line.status,
        });
        if done {
            finish(provider);
        }
    }) {
        Ok(()) => Ok(()),
        Err(e) => {
            running().lock().unwrap().remove(&provider);
            Err(e)
        }
    }
}

/// [`InstallLine`] without the provider — what the runner produces, so a
/// test can drive it with a harmless command.
#[derive(Clone, Debug)]
pub struct Line {
    pub line: Option<String>,
    pub done: bool,
    pub ok: Option<bool>,
    pub status: Option<String>,
}

/// Spawn `$SHELL -lc "<command>"` and stream it. stdin is `/dev/null`, so
/// anything that asks a question (a `sudo` password, a prompt) fails at once
/// instead of hanging.
fn run_command<F>(command: &str, emit: F) -> Result<(), String>
where
    F: Fn(Line) + Send + 'static,
{
    // Guards the table against a future edit.
    if command.split_whitespace().any(|w| w == "sudo") {
        return Err("that command needs sudo, which this app cannot ask for".into());
    }

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let mut child = Command::new(&shell)
        .args(["-lc", command])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run {shell}: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    std::thread::spawn(move || {
        drain_lines(stdout, stderr, |line| {
            emit(Line {
                line: Some(line),
                done: false,
                ok: None,
                status: None,
            })
        });
        let (ok, status) = match child.wait() {
            Ok(s) if s.success() => (true, "installed".to_string()),
            Ok(s) => (false, exit_text(s)),
            Err(e) => (false, format!("could not be waited for: {e}")),
        };
        emit(Line {
            line: None,
            done: true,
            ok: Some(ok),
            status: Some(status),
        });
    });
    Ok(())
}

/// Hand a child's stdout and stderr lines to `on_line` until both pipes
/// close — so nothing arrives after the caller's final `done`. One reader
/// thread per pipe: read in turn, a child filling the unread pipe deadlocks.
/// Order across the two streams is approximate.
pub(super) fn drain_lines(
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    mut on_line: impl FnMut(String),
) {
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let mut readers = Vec::new();
    for pipe in [
        stdout.map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        stderr.map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        }));
    }
    drop(tx);
    for line in rx {
        on_line(line);
    }
    for r in readers {
        let _ = r.join();
    }
}

/// How a child that did not succeed ended.
pub(super) fn exit_text(s: ExitStatus) -> String {
    match s.code() {
        Some(c) => format!("exited with status {c}"),
        None => "stopped by a signal".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    const ALL: [Provider; 4] = [
        Provider::Claude,
        Provider::Codex,
        Provider::Opencode,
        Provider::Antigravity,
    ];

    /// The agents a package manager can install (not Antigravity).
    const PACKAGED: [Provider; 3] = [Provider::Claude, Provider::Codex, Provider::Opencode];

    /// `curl` is gated like every other manager, though macOS always has it.
    #[test]
    fn brew_only_machine_runs_the_brew_route() {
        let have = Managers {
            brew: true,
            ..Default::default()
        };
        for p in PACKAGED {
            let o = offer(p, have);
            assert!(o.runnable, "{p:?}");
            let runnable: Vec<Manager> = o
                .routes
                .iter()
                .filter(|r| r.available)
                .map(|r| r.manager)
                .collect();
            assert_eq!(runnable, vec![Manager::Brew], "{p:?}");
        }
    }

    #[test]
    fn npm_only_machine_runs_the_npm_route() {
        let have = Managers {
            npm: true,
            ..Default::default()
        };
        for p in PACKAGED {
            let runnable: Vec<Manager> = offer(p, have)
                .routes
                .into_iter()
                .filter(|r| r.available)
                .map(|r| r.manager)
                .collect();
            assert_eq!(runnable, vec![Manager::Npm], "{p:?}");
        }
        // bun is only ever opencode's, and only when bun is there.
        let bun = Managers {
            bun: true,
            ..Default::default()
        };
        assert!(offer(Provider::Opencode, bun).runnable);
        assert!(!offer(Provider::Claude, bun).runnable);
        assert!(!offer(Provider::Codex, bun).runnable);
    }

    #[test]
    fn antigravity_is_curl_only() {
        let managers: Vec<Manager> = routes(Provider::Antigravity)
            .iter()
            .map(|r| r.manager)
            .collect();
        assert_eq!(managers, vec![Manager::Curl]);

        let curl = Managers {
            curl: true,
            ..Default::default()
        };
        assert!(offer(Provider::Antigravity, curl).runnable);
        // Homebrew and node buy nothing here.
        for have in [
            Managers { brew: true, ..Default::default() },
            Managers { npm: true, ..Default::default() },
            Managers { bun: true, ..Default::default() },
        ] {
            let o = offer(Provider::Antigravity, have);
            assert!(!o.runnable);
            assert_eq!(o.routes.len(), 1, "the line is still there to copy");
        }
    }

    /// No manager at all still leaves every command to copy.
    #[test]
    fn no_manager_offers_copy_only() {
        for p in ALL {
            let o = offer(p, Managers::default());
            assert!(!o.runnable, "{p:?}");
            assert!(!o.routes.is_empty(), "{p:?}");
            assert!(o.routes.iter().all(|r| !r.available), "{p:?}");
            assert!(o.routes.iter().all(|r| !r.command.is_empty()), "{p:?}");
        }
    }

    #[test]
    fn no_route_needs_sudo() {
        for p in ALL {
            for r in routes(p) {
                assert!(
                    !r.command.split_whitespace().any(|w| w == "sudo"),
                    "{p:?} {}",
                    r.command
                );
            }
        }
    }

    /// The manager is the route id over the invoke boundary.
    #[test]
    fn routes_are_keyed_by_manager() {
        for p in ALL {
            let mut seen = std::collections::HashSet::new();
            for r in routes(p) {
                assert!(seen.insert(r.manager), "{p:?} has two {:?} routes", r.manager);
                assert_eq!(command_for(p, r.manager), Some(r.command));
            }
        }
        assert_eq!(command_for(Provider::Claude, Manager::Bun), None);
        assert!(start(Provider::Claude, Manager::Bun, |_| {}).is_err());
    }

    /// Lines stream, a non-zero exit is a failure, and `done` is last.
    #[test]
    fn runner_streams_then_reports_the_exit() {
        let (tx, rx) = mpsc::channel::<Line>();
        run_command("echo hello; echo trouble 1>&2; exit 3", move |l| {
            let _ = tx.send(l);
        })
        .unwrap();
        let lines: Vec<Line> = rx.iter().collect();
        let (last, body) = lines.split_last().expect("at least the done line");
        assert!(body.iter().all(|l| !l.done));
        let text: Vec<&str> = body.iter().filter_map(|l| l.line.as_deref()).collect();
        assert!(text.contains(&"hello"), "{text:?}");
        assert!(text.contains(&"trouble"), "{text:?}");
        assert!(last.done);
        assert_eq!(last.ok, Some(false));
        assert_eq!(last.status.as_deref(), Some("exited with status 3"));
    }

    #[test]
    fn runner_refuses_sudo() {
        assert!(run_command("sudo make me a sandwich", |_| {}).is_err());
    }
}
