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
//! [`UPDATE_EVENT`] in [`InstallLine`]'s shape. See `docs/harness.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::discover;
use super::event::Provider;
use super::install::{self, InstallLine};

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

/// The one command [`start`] will run for this provider. A pairing with no
/// package-manager route (Antigravity from npm, say) falls back to the
/// CLI's own subcommand.
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
fn effective_source(provider: Provider, source: Source) -> Source {
    match (provider, source) {
        (Provider::Antigravity, _) => Source::SelfManaged,
        (Provider::Claude | Provider::Codex, Source::Bun) => Source::SelfManaged,
        _ => source,
    }
}

/// How an endpoint's body carries the version.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Body {
    /// The whole body is the version (`downloads.claude.ai`).
    Text,
    /// A JSON object's `version` (npm, Homebrew, Antigravity's manifest).
    JsonVersion,
}

/// Where the newest version for this install lives. `channel` is Claude's
/// `autoUpdatesChannel`; `arch` is `arm64` or `amd64`.
fn latest_endpoint(provider: Provider, source: Source, channel: &str, arch: &str) -> (String, Body) {
    let npm = |pkg: &str| (format!("https://registry.npmjs.org/{pkg}/latest"), Body::JsonVersion);
    let cask = |name: &str| (format!("https://formulae.brew.sh/api/cask/{name}.json"), Body::JsonVersion);
    match (provider, effective_source(provider, source)) {
        (Provider::Claude, Source::Brew) => cask("claude-code"),
        (Provider::Claude, Source::Npm) => npm("@anthropic-ai/claude-code"),
        (Provider::Claude, _) => (
            format!("https://downloads.claude.ai/claude-code-releases/{channel}"),
            Body::Text,
        ),
        (Provider::Codex, Source::Brew) => cask("codex"),
        // The curl installer pulls the same release npm publishes.
        (Provider::Codex, _) => npm("@openai/codex"),
        // opencode's formula is in its own tap, which formulae.brew.sh does
        // not serve; every route ships the same GitHub release as npm.
        (Provider::Opencode, _) => npm("opencode-ai"),
        (Provider::Antigravity, _) => (
            format!(
                "https://antigravity-cli-auto-updater-974169037036.us-central1.run.app/manifests/darwin_{arch}.json"
            ),
            Body::JsonVersion,
        ),
    }
}

/// `stable` when `~/.claude/settings.json` asks for it, else `latest`.
fn claude_channel(home: Option<&Path>) -> &'static str {
    let stable = home
        .and_then(|h| std::fs::read_to_string(h.join(".claude/settings.json")).ok())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .is_some_and(|v| v.get("autoUpdatesChannel").and_then(|c| c.as_str()) == Some("stable"));
    if stable { "stable" } else { "latest" }
}

fn arch() -> &'static str {
    if std::env::consts::ARCH == "x86_64" { "amd64" } else { "arm64" }
}

/// The numeric core of a version: `v` dropped, then everything from the
/// first `-`/`+`/`,` (prerelease, build, a cask's `,sha`) ignored — so
/// `1.2.0-beta` orders as `1.2.0`.
fn version_core(v: &str) -> Vec<u64> {
    let v = v.trim();
    let v = v.strip_prefix('v').unwrap_or(v);
    let core = v.split(['-', '+', ',']).next().unwrap_or("");
    core.split('.')
        .map(|part| {
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse().unwrap_or(0)
        })
        .collect()
}

/// Whether `latest` is strictly newer than `installed`, missing parts read
/// as zero. A local build ahead of the registry is not an update.
pub fn is_newer(latest: &str, installed: &str) -> bool {
    let (a, b) = (version_core(latest), version_core(installed));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

fn read_version(body: &str, shape: Body) -> Result<String, String> {
    let v = match shape {
        Body::Text => body.trim().to_string(),
        Body::JsonVersion => serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|j| j.get("version").and_then(|v| v.as_str()).map(str::to_string))
            .ok_or("the answer had no version")?,
    };
    if v.trim_start_matches('v').starts_with(|c: char| c.is_ascii_digit()) {
        Ok(v)
    } else {
        Err(format!("unexpected answer {v:?}"))
    }
}

fn fetch(url: &str, shape: Body) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build();
    let body = agent
        .get(url)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("{url} answered {code}"),
            ureq::Error::Transport(t) => format!("could not reach {url}: {t}"),
        })?
        .into_string()
        .map_err(|e| format!("could not read {url}: {e}"))?;
    read_version(&body, shape)
}

const FRESH_FOR: Duration = Duration::from_secs(6 * 60 * 60);
/// A failed check is retried sooner, but not on every visit while offline.
const FAILURE_FRESH_FOR: Duration = Duration::from_secs(10 * 60);

/// Newest versions by endpoint URL. Only the registry's answer is cached;
/// the installed side comes from `discover::health` on every read.
fn latest_cache() -> &'static Mutex<HashMap<String, (Instant, Result<String, String>)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Result<String, String>)>>> =
        OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Drop the cached newest versions; `discover::forget` calls it.
pub fn forget() {
    latest_cache().lock().unwrap().clear();
}

fn latest(url: &str, shape: Body) -> Result<String, String> {
    if let Some((at, hit)) = latest_cache().lock().unwrap().get(url) {
        let ttl = if hit.is_ok() { FRESH_FOR } else { FAILURE_FRESH_FOR };
        if at.elapsed() < ttl {
            return hit.clone();
        }
    }
    let got = fetch(url, shape);
    latest_cache().lock().unwrap().insert(url.to_string(), (Instant::now(), got.clone()));
    got
}

/// One CLI's row in Settings: what is installed, what is published, and the
/// command that would close the gap.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub provider: Provider,
    pub installed: Option<String>,
    pub latest: Option<String>,
    /// Only when `latest` is strictly newer; never on a failed check.
    pub available: bool,
    pub source: Source,
    pub command: String,
    pub error: Option<String>,
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// The binary, its source and its update command; `None` when not installed.
fn plan(provider: Provider) -> Option<(PathBuf, Source, String)> {
    let bin = discover::binary(provider).ok()?;
    let real = bin.canonicalize().unwrap_or_else(|_| bin.clone());
    let source = source_of(&real, home().as_deref());
    let cmd = command(provider, source, &bin);
    Some((bin, source, cmd))
}

/// The update check for one provider, `None` when its CLI is missing.
pub fn check(provider: Provider) -> Option<UpdateInfo> {
    let (_, source, command) = plan(provider)?;
    let installed = discover::health(provider).version;
    let home = home();
    let (url, shape) = latest_endpoint(provider, source, claude_channel(home.as_deref()), arch());
    let (latest, error) = match latest(&url, shape) {
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(e)),
    };
    let available = matches!((&latest, &installed), (Some(l), Some(i)) if is_newer(l, i));
    Some(UpdateInfo {
        provider,
        installed,
        latest,
        available,
        source,
        command,
        error,
    })
}

/// Every installed provider's check, the four requests side by side.
pub fn check_all() -> Vec<UpdateInfo> {
    let handles: Vec<_> = discover::PROVIDERS
        .iter()
        .map(|&p| std::thread::spawn(move || check(p)))
        .collect();
    handles.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
}

/// Where an update's output reaches the webview.
pub const UPDATE_EVENT: &str = "harness-update";

/// The provider updating now; one at a time across all of them.
fn running() -> &'static Mutex<Option<Provider>> {
    static R: OnceLock<Mutex<Option<Provider>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Run the provider's update command, streaming its output to `emit` and
/// closing with one `done` line. The lock and the provider's cached health
/// are released before that line, so the next queued update can start and a
/// recheck reads the new version.
pub fn start<F>(provider: Provider, emit: F) -> Result<(), String>
where
    F: Fn(InstallLine) + Send + 'static,
{
    let (_, _, command) =
        plan(provider).ok_or_else(|| format!("{} is not installed", provider.label()))?;
    {
        let mut r = running().lock().unwrap();
        if let Some(busy) = *r {
            return Err(format!("{} is updating; updates run one at a time", busy.label()));
        }
        *r = Some(provider);
    }
    let result = install::run_command(&command, "updated", move |line| {
        if line.done {
            discover::forget_health(provider);
            *running().lock().unwrap() = None;
        }
        emit(InstallLine {
            provider,
            line: line.line,
            done: line.done,
            ok: line.ok,
            status: line.status,
        });
    });
    if result.is_err() {
        *running().lock().unwrap() = None;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/Users/s";

    fn src(p: &str) -> Source {
        source_of(Path::new(p), Some(Path::new(HOME)))
    }

    #[test]
    fn source_follows_the_real_path() {
        assert_eq!(src("/opt/homebrew/Caskroom/codex/0.153.4/codex-aarch64-apple-darwin"), Source::Brew);
        assert_eq!(src("/usr/local/Caskroom/claude-code/2.1.289/claude"), Source::Brew);
        assert_eq!(src("/opt/homebrew/Cellar/opencode/1.18.31/bin/opencode"), Source::Brew);
        // A formula that vendors a node package is still Homebrew's.
        assert_eq!(src("/opt/homebrew/Cellar/x/1/libexec/lib/node_modules/x/cli.js"), Source::Brew);
        // npm's global prefix under Homebrew's node is npm's.
        assert_eq!(src("/opt/homebrew/lib/node_modules/@openai/codex/bin/codex.js"), Source::Npm);
        assert_eq!(src("/Users/s/.nvm/versions/node/v22.1.0/lib/node_modules/opencode-ai/bin/opencode"), Source::Npm);
        assert_eq!(src("/Users/s/.bun/install/global/node_modules/opencode-ai/bin/opencode"), Source::Bun);
        assert_eq!(src("/Users/s/.local/share/claude/versions/2.1.289"), Source::SelfManaged);
        assert_eq!(src("/Users/s/.opencode/bin/opencode"), Source::SelfManaged);
        assert_eq!(src("/Users/s/.local/bin/agy"), Source::SelfManaged);
        assert_eq!(src("/opt/homebrew/bin/codex"), Source::Brew);
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("1.18.34", "1.18.31"));
        assert!(is_newer("0.160.0", "0.153.4"));
        assert!(is_newer("v0.160.1", "0.153.4"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("1.2.1", "1.2"));
        // Older or equal is never an update — a local build ahead included.
        assert!(!is_newer("2.1.285", "2.1.289"));
        assert!(!is_newer("2.1.289", "v2.1.289"));
        assert!(!is_newer("1.2", "1.2.0"));
        // Prerelease and build suffixes are ignored for ordering.
        assert!(!is_newer("1.2.0", "1.2.0-beta.1"));
        assert!(is_newer("1.3.0-rc.1", "1.2.9"));
        assert!(!is_newer("0.160.0,abc123", "0.160.0"));
    }

    #[test]
    fn commands_match_the_install() {
        let p = Path::new("/Users/s/.local/bin/claude");
        assert_eq!(command(Provider::Claude, Source::SelfManaged, p), "'/Users/s/.local/bin/claude' update");
        assert_eq!(command(Provider::Codex, Source::Brew, p), "brew upgrade --cask codex");
        assert_eq!(command(Provider::Claude, Source::Brew, p), "brew upgrade --cask claude-code");
        assert_eq!(command(Provider::Opencode, Source::Brew, p), "brew upgrade anomalyco/tap/opencode");
        assert_eq!(command(Provider::Codex, Source::Npm, p), "npm install -g @openai/codex@latest");
        assert_eq!(command(Provider::Opencode, Source::Bun, p), "bun install -g opencode-ai@latest");
        let oc = Path::new("/Users/s/.opencode/bin/opencode");
        assert_eq!(command(Provider::Opencode, Source::SelfManaged, oc), "'/Users/s/.opencode/bin/opencode' upgrade");
        let quoted = Path::new("/Users/it's/bin/agy");
        assert_eq!(command(Provider::Antigravity, Source::SelfManaged, quoted), r"'/Users/it'\''s/bin/agy' update");
    }

    #[test]
    fn unknown_pairings_fall_back_to_the_cli() {
        let agy = Path::new("/opt/homebrew/bin/agy");
        assert_eq!(command(Provider::Antigravity, Source::Brew, agy), "'/opt/homebrew/bin/agy' update");
        assert_eq!(command(Provider::Antigravity, Source::Npm, agy), "'/opt/homebrew/bin/agy' update");
        let claude = Path::new("/Users/s/.bun/bin/claude");
        assert_eq!(command(Provider::Claude, Source::Bun, claude), "'/Users/s/.bun/bin/claude' update");
        // And they are compared against what that command installs.
        let (url, _) = latest_endpoint(Provider::Claude, Source::Bun, "latest", "arm64");
        assert!(url.starts_with("https://downloads.claude.ai/"), "{url}");
    }

    #[test]
    fn no_command_needs_sudo() {
        let p = Path::new("/x/bin/cli");
        for provider in discover::PROVIDERS {
            for source in [Source::Brew, Source::Npm, Source::Bun, Source::SelfManaged] {
                let c = command(provider, source, p);
                assert!(!c.split_whitespace().any(|w| w == "sudo"), "{c}");
            }
        }
    }

    /// Homebrew's cask can trail npm, so a brew install is compared to brew.
    #[test]
    fn each_source_reads_its_own_registry() {
        let url = |p, s| latest_endpoint(p, s, "latest", "arm64").0;
        assert_eq!(url(Provider::Codex, Source::Brew), "https://formulae.brew.sh/api/cask/codex.json");
        assert_eq!(url(Provider::Codex, Source::Npm), "https://registry.npmjs.org/@openai/codex/latest");
        assert_eq!(url(Provider::Claude, Source::Brew), "https://formulae.brew.sh/api/cask/claude-code.json");
        assert_eq!(url(Provider::Claude, Source::Npm), "https://registry.npmjs.org/@anthropic-ai/claude-code/latest");
        assert_eq!(url(Provider::Claude, Source::SelfManaged), "https://downloads.claude.ai/claude-code-releases/latest");
        assert_eq!(
            latest_endpoint(Provider::Claude, Source::SelfManaged, "stable", "arm64").0,
            "https://downloads.claude.ai/claude-code-releases/stable"
        );
        assert_eq!(url(Provider::Opencode, Source::Brew), "https://registry.npmjs.org/opencode-ai/latest");
        assert!(url(Provider::Antigravity, Source::SelfManaged).ends_with("/manifests/darwin_arm64.json"));
    }

    #[test]
    fn bodies_yield_a_version_or_an_error() {
        assert_eq!(read_version("2.1.289\n", Body::Text).unwrap(), "2.1.289");
        assert_eq!(read_version(r#"{"name":"x","version":"1.18.34"}"#, Body::JsonVersion).unwrap(), "1.18.34");
        assert!(read_version("<html>", Body::Text).is_err());
        assert!(read_version(r#"{"error":"not found"}"#, Body::JsonVersion).is_err());
    }

    #[test]
    fn claude_channel_reads_its_settings() {
        let dir = crate::test_support::Scratch::new("update-channel");
        assert_eq!(claude_channel(Some(&*dir)), "latest");
        std::fs::create_dir_all(dir.join(".claude")).unwrap();
        std::fs::write(dir.join(".claude/settings.json"), r#"{"autoUpdatesChannel":"stable"}"#).unwrap();
        assert_eq!(claude_channel(Some(&*dir)), "stable");
    }

    /// Hits the real endpoints: `cargo test --lib harness::update -- --ignored`.
    #[test]
    #[ignore]
    fn live_endpoints_answer() {
        for (p, s) in [
            (Provider::Claude, Source::SelfManaged),
            (Provider::Claude, Source::Brew),
            (Provider::Claude, Source::Npm),
            (Provider::Codex, Source::Brew),
            (Provider::Codex, Source::Npm),
            (Provider::Opencode, Source::SelfManaged),
            (Provider::Antigravity, Source::SelfManaged),
        ] {
            let (url, shape) = latest_endpoint(p, s, "latest", arch());
            let v = fetch(&url, shape).unwrap_or_else(|e| panic!("{p:?} {s:?}: {e}"));
            assert!(!version_core(&v).is_empty(), "{url}: {v}");
        }
    }
}
