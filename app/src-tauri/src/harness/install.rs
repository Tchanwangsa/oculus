//! Installing a missing CLI agent, from Settings → AI.
//!
//! `discover.rs` answers *where* a CLI is, or that it is nowhere. That left
//! the student's next move outside the app entirely: work out which of four
//! install routes this machine wants, run it in a terminal, come back. This
//! module is the way through, and it is deliberately **not** a package
//! manager wrapper:
//!
//! - **Nothing is installed for you that you were not shown.** Every route is
//!   a literal command string from the vendor's own documentation, displayed
//!   verbatim with a Copy button. The copy path is the one that always works —
//!   no package manager, a locked-down machine, or a student who would rather
//!   run it themselves — so it is offered even when the Run button is.
//! - **The click on a button showing the command is the confirmation.** There
//!   is no other one, and nothing runs without it.
//! - **Nothing that needs elevation is ever offered.** A GUI app has no
//!   terminal to put a password prompt in, and `sudo` under a pipe with no tty
//!   hangs rather than fails; so a provider whose only route on this machine
//!   would need it gets the command and nothing more. [`start`] refuses one
//!   anyway, and a test pins it.
//! - **The webview never names the command.** It names a provider and a
//!   manager; the string comes out of the table below. An invoke that could
//!   pass an arbitrary string to `$SHELL -lc` would be a shell for anything
//!   that reached the webview, in the one app that also holds the student's
//!   Canvas session.
//!
//! **macOS only.** The routes here are macOS ones and detection itself is
//! macOS-shaped (see `discover.rs`'s module docs: `binary_name` has no
//! `.cmd`/`.exe`, and npm on Windows installs `claude.cmd` resolved through
//! `PATHEXT`), as is the rest of the app. `scoop` and `winget` are real and
//! are not here; offering them would mean a button that installs something
//! discovery then cannot find.
//!
//! The commands run through `$SHELL -lc` for the same reason discovery asks a
//! login shell: a Dock-launched app has launchd's PATH, and `brew` is not on
//! it. Output is streamed line by line as it arrives — an install that prints
//! nothing for forty seconds is indistinguishable from one that hung.
//!
//! What this module does *not* do is forget the discovery cache when a run
//! finishes. `discover::binary` caches failures for the life of the process,
//! so a freshly installed CLI stays "missing" until something calls
//! `discover::forget()` — and the webview has a second cache of its own
//! (`app/src/hooks/useBridgeHealth.ts`). Dropping only Rust's here would leave
//! the module-level one stale, so the recheck is the frontend's to fire, on
//! the exit event, through the one path that invalidates both.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
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
    /// Verbatim, and it has to stay verbatim: it is both what is shown and
    /// what is run.
    command: &'static str,
    /// Where the binary lands, which has to be somewhere
    /// `discover::well_known_dirs` already looks — a route that installed
    /// out of discovery's sight would leave the row saying "missing" after a
    /// successful install.
    lands_in: &'static str,
}

// ── The routes ───────────────────────────────────────────────────────────────
//
// Verified 2026-09-17 against the vendors' own documentation; re-check these
// rather than trusting them, they move. The URL beside each is where it came
// from. Order is the vendor's own recommendation first, and the frontend shows
// whichever of them this machine can actually run.

/// Claude Code.
/// - script: <https://code.claude.com/docs/en/setup> ("Native Install
///   (Recommended)"). The same page says the launcher it manages is
///   `~/.local/bin/claude`, and that native installs auto-update.
/// - brew: same page, "Homebrew" tab. The `claude-code` cask tracks the stable
///   channel; `claude-code@latest` tracks latest. Casks do not auto-update.
/// - npm: same page, "Install with npm".
///
/// No bun route: the vendor documents none, and the npm package pulls its real
/// binary in through a per-platform optional dependency plus a postinstall
/// link step — bun does not run postinstall scripts for untrusted packages, so
/// a bun route would look like it worked and leave nothing to run.
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

/// Codex.
/// - script: <https://learn.chatgpt.com/docs/codex/cli> and the repo's README,
///   <https://github.com/openai/codex>. Reading the script itself:
///   `BIN_DIR="${CODEX_INSTALL_DIR:-$HOME/.local/bin}"`, and no `sudo`
///   anywhere in it.
/// - brew / npm: <https://github.com/openai/codex> README, install section.
///
/// No bun route, for the same reason as Claude's.
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

/// opencode.
/// - script: <https://opencode.ai/docs/>. Reading the script itself:
///   `INSTALL_DIR=$HOME/.opencode/bin`, no `sudo`.
/// - brew: <https://github.com/anomalyco/opencode> README. The tap moved with
///   the project — it is `anomalyco/tap`, not the `sst/tap` older writeups
///   name.
/// - npm / bun: same README (`npm i -g opencode-ai@latest`, "or bun/pnpm/
///   yarn"). This is the one vendor that documents bun, which is why it is the
///   one provider with a bun route.
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

fn routes(provider: Provider) -> &'static [Route] {
    match provider {
        Provider::Claude => CLAUDE,
        Provider::Codex => CODEX,
        Provider::Opencode => OPENCODE,
    }
}

/// Which of the four this machine has. Taken as a value rather than read
/// inside [`offer`] so the choice is testable without a machine that has them.
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

/// What is on this machine, found the same way the CLIs themselves are — PATH,
/// then the well-known directories, then a login shell — and cached beside
/// them, because `brew` does not move mid-session either.
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
    /// Whether the tool it needs is on this machine. A route that is not
    /// available is still worth showing the command for; it just has no
    /// button.
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
    /// Whether any of them can be run from here. False means the dialog is a
    /// command to copy and nothing else — which is the honest answer on a
    /// machine with neither Homebrew, nor node, nor bun, nor `curl`.
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

/// Where an install's output reaches the webview. One event for the whole
/// run, the way `chapters.rs` streams a job's progress — the dialog that
/// asked for the install is the only listener, and it filters by provider.
pub const INSTALL_EVENT: &str = "harness-install";

/// What the webview gets, line by line, while an install runs.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallLine {
    pub provider: Provider,
    /// One line of the child's output — stdout and stderr both, because an
    /// installer's progress and its complaints are one story.
    pub line: Option<String>,
    /// The last event of a run, and the one the frontend rechecks on.
    pub done: bool,
    pub ok: Option<bool>,
    /// How it ended, for the line the dialog draws when it did not end well.
    pub status: Option<String>,
}

/// One install at a time per provider. Two runs of the same command racing
/// each other through the same package manager is a lock file's problem at
/// best.
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

/// The same shape as [`InstallLine`] with no provider on it — what the runner
/// itself produces, so it can be driven by a test with a harmless command.
#[derive(Clone, Debug)]
pub struct Line {
    pub line: Option<String>,
    pub done: bool,
    pub ok: Option<bool>,
    pub status: Option<String>,
}

/// Spawn `$SHELL -lc "<command>"` and stream it.
///
/// Three details are load-bearing. The **login** shell is what has the
/// profile's PATH, which is the only reason `brew` resolves from a
/// Dock-launched app. **stdin is `/dev/null`**, so anything that decides to
/// ask a question — a `sudo` password, a confirmation prompt — fails
/// immediately instead of hanging forever behind a dialog that would go on
/// saying "working". And stdout and stderr are drained by **two threads into
/// one channel**: read one after the other, a child that fills the pipe
/// nobody is reading yet deadlocks. The ordering between the two streams is
/// therefore approximate, which is the right trade for a log nobody parses.
fn run_command<F>(command: &str, emit: F) -> Result<(), String>
where
    F: Fn(Line) + Send + 'static,
{
    // Belt and braces over the table above: the rule is that nothing needing
    // elevation is ever run, and the table is the thing a future edit changes.
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
        // Ends when both readers have hung up, which is after the child has
        // closed both pipes — so no output can arrive after the `done` below.
        for line in rx {
            emit(Line {
                line: Some(line),
                done: false,
                ok: None,
                status: None,
            });
        }
        for r in readers {
            let _ = r.join();
        }
        let (ok, status) = match child.wait() {
            Ok(s) if s.success() => (true, "installed".to_string()),
            Ok(s) => (
                false,
                match s.code() {
                    Some(c) => format!("exited with status {c}"),
                    None => "stopped by a signal".to_string(),
                },
            ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    const ALL: [Provider; 3] = [Provider::Claude, Provider::Codex, Provider::Opencode];

    /// A machine with Homebrew and nothing else runs the brew route, and is
    /// offered the others as text. `curl` is the interesting half: it is on
    /// every macOS, so the script route is nearly always the one a student
    /// sees — but "nearly always" is not "assume", and it is gated like the
    /// rest.
    #[test]
    fn brew_only_machine_runs_the_brew_route() {
        let have = Managers {
            brew: true,
            ..Default::default()
        };
        for p in ALL {
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

    /// The offer follows the machine, not the provider: node and nothing else
    /// gets the npm route, and opencode's bun route stays dark.
    #[test]
    fn npm_only_machine_runs_the_npm_route() {
        let have = Managers {
            npm: true,
            ..Default::default()
        };
        for p in ALL {
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

    /// A machine with no package manager at all — not even `curl` — is still
    /// given something to copy. This is the case the whole "show the command"
    /// half exists for: no route can be offered, no button is drawn, and the
    /// commands are still there.
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

    /// Nothing offered may need elevation: a GUI app cannot answer a password
    /// prompt, and the prompt arrives on a pipe nobody is reading.
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

    /// One route per manager per provider — the manager *is* the route id
    /// over the invoke boundary, so a duplicate would make a run ambiguous.
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

    /// The runner itself, against something harmless: output arrives as lines
    /// while it runs, a non-zero exit is reported as a failure rather than as
    /// silence, and the `done` line is always last.
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
