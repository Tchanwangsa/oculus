//! Finding the provider CLIs from inside a GUI app.
//!
//! A Dock-launched app inherits launchd's PATH (`/usr/bin`, `/bin`), so
//! `~/.local/bin/claude` or `/opt/homebrew/bin/codex` would not resolve. So:
//! an explicit override, PATH, the installers' usual dirs, then a login shell.
//! Results are cached for the process. macOS-shaped on purpose (no Windows
//! `.cmd`/`PATHEXT`), like [`install`](super::install).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use super::event::Provider;

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

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Where the installers put things, in the order worth trying.
fn well_known_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(h) = home() {
        dirs.push(h.join(".local/bin"));
        dirs.push(h.join(".claude/local"));
        dirs.push(h.join(".opencode/bin"));
        dirs.push(h.join(".bun/bin"));
        dirs.push(h.join(".npm-global/bin"));
        dirs.push(h.join(".volta/bin"));
        dirs.push(h.join(".cargo/bin"));
        // nvm keeps one bin dir per node version; take any that has it.
        if let Ok(rd) = std::fs::read_dir(h.join(".nvm/versions/node")) {
            for e in rd.flatten() {
                dirs.push(e.path().join("bin"));
            }
        }
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

fn search_path_env(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| is_executable(p))
}

/// Ask a login shell, which has the PATH a terminal would. Slow, so it is the
/// last step and cached.
fn ask_login_shell(name: &str) -> Option<PathBuf> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let out = std::process::Command::new(shell)
        .args(["-lc", &format!("command -v {name}")])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let p = PathBuf::from(s);
    is_executable(&p).then_some(p)
}

/// The same lookup order for provider CLIs and installer tools.
fn find_executable(name: &str) -> Option<PathBuf> {
    search_path_env(name)
        .or_else(|| {
            well_known_dirs()
                .into_iter()
                .map(|d| d.join(name))
                .find(|p| is_executable(p))
        })
        .or_else(|| ask_login_shell(name))
}

fn locate(provider: Provider) -> Result<PathBuf, String> {
    let name = binary_name(provider);
    if let Some(p) = std::env::var_os(override_env(provider)) {
        let p = PathBuf::from(p);
        return if is_executable(&p) {
            Ok(p)
        } else {
            Err(format!(
                "{} points at {}, which is not an executable",
                override_env(provider),
                p.display()
            ))
        };
    }
    if let Some(p) = find_executable(name) {
        return Ok(p);
    }
    Err(format!(
        "`{name}` is not installed, or not on PATH — set {} to its full path",
        override_env(provider)
    ))
}

fn cache() -> &'static Mutex<std::collections::HashMap<Provider, Result<PathBuf, String>>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<Provider, Result<PathBuf, String>>>> =
        OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The provider's binary, or why not. A miss is cached too, so a missing CLI
/// does not re-probe a login shell on every send; [`forget`] clears it.
pub fn binary(provider: Provider) -> Result<PathBuf, String> {
    let mut c = cache().lock().unwrap();
    c.entry(provider)
        .or_insert_with(|| locate(provider))
        .clone()
}

fn tool_cache() -> &'static Mutex<std::collections::HashMap<String, Option<PathBuf>>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<String, Option<PathBuf>>>> =
        OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Where some other tool (`brew`, `npm`, …) is, by the same steps and cache as
/// [`binary`]. The login shell matters for `brew`, whose dir launchd's PATH lacks.
pub fn tool(name: &str) -> Option<PathBuf> {
    if let Some(hit) = tool_cache().lock().unwrap().get(name) {
        return hit.clone();
    }
    let found = find_executable(name);
    tool_cache()
        .lock()
        .unwrap()
        .insert(name.to_string(), found.clone());
    found
}

/// Drop cached lookups, health and newest versions, for a recheck after an
/// install.
pub fn forget() {
    cache().lock().unwrap().clear();
    health_cache().lock().unwrap().clear();
    tool_cache().lock().unwrap().clear();
    super::update::forget();
}

/// Drop one provider's cached `--version`, after an update replaced it.
pub fn forget_health(provider: Provider) {
    health_cache().lock().unwrap().remove(&provider);
}

/// The `oculus` binary the child should find on its PATH: the app's sibling in a
/// bundle; in dev the preflight's debug build (`app/scripts/predev.mjs`), the
/// release one `bun run cli` leaves, or the `~/.local/bin` symlink.
pub fn oculus_cli() -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for c in [dir.join("oculus"), dir.join("../release/oculus")] {
                if is_executable(&c) {
                    found.push(c.canonicalize().unwrap_or(c));
                }
            }
        }
    }
    if let Some(h) = home() {
        let c = h.join(".local/bin/oculus");
        if is_executable(&c) {
            found.push(c.canonicalize().unwrap_or(c));
        }
    }
    if found.is_empty() {
        return search_path_env("oculus");
    }
    // Newest wins: its dir heads the thread's PATH (`child_env`), so a stale dev
    // build would shadow the current CLI. See `docs/development.md`.
    found.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    let chosen = found.pop();
    if let Some(bin) = &chosen {
        warn_if_stale(bin);
    }
    chosen
}

/// The `src/` of the checkout that built a `<root>/target/{debug,release}/oculus`;
/// `None` for an installed CLI, which has no sources to be behind.
fn dev_checkout_src(bin: &Path) -> Option<PathBuf> {
    let profile = bin.parent()?;
    match profile.file_name()?.to_str()? {
        "debug" | "release" => {}
        _ => return None,
    }
    let target = profile.parent()?;
    if target.file_name()?.to_str()? != "target" {
        return None;
    }
    let src = target.parent()?.join("src");
    src.is_dir().then_some(src)
}

/// The newest `.rs` mtime under `dir` — the set cargo rebuilds from.
fn newest_rs_mtime(dir: &Path) -> Option<std::time::SystemTime> {
    let mut newest: Option<std::time::SystemTime> = None;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                if let Ok(t) = meta.modified() {
                    if newest.is_none_or(|n| t > n) {
                        newest = Some(t);
                    }
                }
            }
        }
    }
    newest
}

/// Warn once when the CLI an agent is about to get predates its sources. The dev
/// scripts should prevent it, but a stale `oculus` just rejects new
/// subcommands, which reads as the agent's mistake.
fn warn_if_stale(bin: &Path) {
    static CHECKED: OnceLock<()> = OnceLock::new();
    CHECKED.get_or_init(|| {
        let Some(src) = dev_checkout_src(bin) else {
            return;
        };
        let Ok(built) = std::fs::metadata(bin).and_then(|m| m.modified()) else {
            return;
        };
        let Some(newest) = newest_rs_mtime(&src) else {
            return;
        };
        if newest > built {
            eprintln!("[oculus] {} is older than {}", bin.display(), src.display());
            eprintln!("[oculus] agents this session will run a stale CLI — `bun run cli:dev`");
        }
    });
}

/// What Settings shows: found where, which version, and if not, why.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BridgeHealth {
    pub provider: Provider,
    pub label: &'static str,
    pub path: Option<String>,
    pub version: Option<String>,
    pub error: Option<String>,
    /// Which env var overrides discovery, for the Settings hint.
    pub override_env: &'static str,
}

fn health_cache() -> &'static Mutex<std::collections::HashMap<Provider, BridgeHealth>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<Provider, BridgeHealth>>> =
        OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Where each binary is, its version, or why not. Cached, because the model
/// picker reads it on every composer; [`forget`] clears it.
pub fn health(provider: Provider) -> BridgeHealth {
    if let Some(h) = health_cache().lock().unwrap().get(&provider) {
        return h.clone();
    }
    let h = probe_health(provider);
    // Probed outside the lock: racing callers write the same answer rather than block.
    health_cache().lock().unwrap().insert(provider, h.clone());
    h
}

fn probe_health(provider: Provider) -> BridgeHealth {
    let mut h = BridgeHealth {
        provider,
        label: provider.label(),
        path: None,
        version: None,
        error: None,
        override_env: override_env(provider),
    };
    match binary(provider) {
        Ok(p) => {
            h.path = Some(p.display().to_string());
            match std::process::Command::new(&p).arg("--version").output() {
                Ok(out) if out.status.success() => {
                    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    // `2.1.267 (Claude Code)` / `codex-cli 0.153.4` — keep the number.
                    let num = v
                        .split_whitespace()
                        .find(|w| w.chars().next().map_or(false, |c| c.is_ascii_digit()))
                        .unwrap_or(&v);
                    h.version = Some(num.to_string());
                }
                Ok(out) => {
                    h.error = Some(format!(
                        "`--version` failed: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    ))
                }
                Err(e) => h.error = Some(format!("cannot run {}: {e}", p.display())),
            }
        }
        Err(e) => h.error = Some(e),
    }
    h
}

/// The environment a provider child gets. API keys are stripped so a key in the
/// user's shell cannot move a subscription onto per-token billing, or grow
/// opencode a provider nobody chose. The `oculus` binary's dir heads PATH,
/// because `AGENTS.md` tells the agent to run it.
pub fn child_env() -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| {
            !matches!(
                k.as_str(),
                "ANTHROPIC_API_KEY"
                    | "OPENAI_API_KEY"
                    | "CLAUDE_AGENT_SDK_CLIENT_APP"
                    // Set inside a Claude Code session; the CLI refuses to nest.
                    | "CLAUDECODE"
                    | "CLAUDE_CODE_ENTRYPOINT"
            )
        })
        .collect();

    let mut path_parts: Vec<PathBuf> = Vec::new();
    if let Some(cli) = oculus_cli() {
        if let Some(d) = cli.parent() {
            path_parts.push(d.to_path_buf());
        }
    }
    for p in PROVIDERS {
        if let Ok(b) = binary(p) {
            if let Some(d) = b.parent() {
                path_parts.push(d.to_path_buf());
            }
        }
    }
    if let Some(cur) = std::env::var_os("PATH") {
        path_parts.extend(std::env::split_paths(&cur));
    } else {
        path_parts.extend(well_known_dirs());
        path_parts.push(PathBuf::from("/usr/bin"));
        path_parts.push(PathBuf::from("/bin"));
    }
    let mut seen = std::collections::HashSet::new();
    path_parts.retain(|p| seen.insert(p.clone()));
    if let Ok(joined) = std::env::join_paths(&path_parts) {
        env.retain(|(k, _)| k != "PATH");
        env.push(("PATH".into(), joined.to_string_lossy().into_owned()));
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    fn scratch(name: &str) -> Scratch {
        Scratch::new(&format!("discover-{name}"))
    }

    #[test]
    fn only_a_checkout_has_sources_to_be_behind() {
        let root = scratch("shapes");
        std::fs::create_dir_all(root.join("src")).unwrap();
        for profile in ["debug", "release"] {
            std::fs::create_dir_all(root.join("target").join(profile)).unwrap();
            let bin = root.join("target").join(profile).join("oculus");
            assert_eq!(dev_checkout_src(&bin), Some(root.join("src")));
        }

        // A bundle's sidecar, and a cargo layout with no sources beside it.
        assert_eq!(
            dev_checkout_src(Path::new("/Applications/Oculus.app/Contents/MacOS/oculus")),
            None
        );
        let bare = scratch("bare");
        std::fs::create_dir_all(bare.join("target/debug")).unwrap();
        assert_eq!(dev_checkout_src(&bare.join("target/debug/oculus")), None);
    }

    #[test]
    fn a_touched_source_is_newer_than_the_binary() {
        let root = scratch("mtime");
        std::fs::create_dir_all(root.join("src/harness")).unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        let bin = root.join("target/debug/oculus");
        std::fs::write(&bin, b"binary").unwrap();
        let built = std::fs::metadata(&bin).unwrap().modified().unwrap();

        // Nested, so the walk must recurse, and written after the binary.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(root.join("src/harness/discover.rs"), b"fn main() {}").unwrap();
        let newest = newest_rs_mtime(&root.join("src")).unwrap();
        assert!(newest > built, "an edit after the build reads as newer");

        // Non-Rust files are not what cargo rebuilds from.
        std::fs::remove_file(root.join("src/harness/discover.rs")).unwrap();
        std::fs::write(root.join("src/notes.md"), b"# not a rebuild").unwrap();
        assert_eq!(newest_rs_mtime(&root.join("src")), None);
    }
}
