//! Finding the provider CLIs from inside a GUI app.
//!
//! A Tauri app launched from the Dock inherits launchd's PATH — `/usr/bin`,
//! `/bin` and little else — so neither `~/.local/bin/claude` nor
//! `/opt/homebrew/bin/codex` resolves the way it does in a terminal. Same
//! class of problem as the sidecar's venv, same answer: an explicit override,
//! then the places the installers actually put things, then a login shell's
//! opinion as the last resort. Results are cached for the life of the
//! process; the binaries do not move mid-session.
//!
//! **This is macOS-shaped, and knowingly so.** `binary_name` answers a bare
//! `claude`, `is_executable` off unix is only "is it a file", and npm on
//! Windows writes `claude.cmd`, resolved through `PATHEXT` — so discovery
//! would miss an npm install there even before anything tried to run it. The
//! rest of the app leans the same way (launchd's PATH, `/opt/homebrew`, the
//! keep-alive LaunchAgent), so [`install`](super::install) offers macOS routes
//! only rather than pretending otherwise.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use super::event::Provider;

/// `OCULUS_CLAUDE_BIN` / `OCULUS_CODEX_BIN` / `OCULUS_OPENCODE_BIN` — bb's
/// `BB_CLAUDE_CODE_EXECUTABLE`, for a build that lives somewhere unusual.
pub fn override_env(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "OCULUS_CLAUDE_BIN",
        Provider::Codex => "OCULUS_CODEX_BIN",
        Provider::Opencode => "OCULUS_OPENCODE_BIN",
    }
}

fn binary_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "claude",
        Provider::Codex => "codex",
        Provider::Opencode => "opencode",
    }
}

/// Every provider, in the order Settings lists them. One array rather than a
/// literal at each call site: a provider added to the enum without being
/// added here is a bridge nobody can find.
pub const PROVIDERS: [Provider; 3] = [Provider::Claude, Provider::Codex, Provider::Opencode];

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Where the installers put things, in the order worth trying. `~/.claude/local`
/// is Claude's own migrate-installer target and `~/.opencode/bin` is where
/// opencode's install script puts its binary; the rest are npm-global, bun and
/// Homebrew defaults.
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

/// Ask a login shell, which sources the user's profile and therefore has the
/// PATH a terminal would. Slow (a few hundred ms), so it is the last step and
/// the result is cached.
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
    if let Some(p) = search_path_env(name) {
        return Ok(p);
    }
    if let Some(p) = well_known_dirs().into_iter().map(|d| d.join(name)).find(|p| is_executable(p)) {
        return Ok(p);
    }
    if let Some(p) = ask_login_shell(name) {
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

/// The provider's binary, or why it cannot be found. Cached per provider;
/// a failure is cached too, since re-probing a login shell on every send
/// would make a missing CLI slow as well as absent. [`forget`] clears it.
pub fn binary(provider: Provider) -> Result<PathBuf, String> {
    let mut c = cache().lock().unwrap();
    c.entry(provider).or_insert_with(|| locate(provider)).clone()
}

fn tool_cache() -> &'static Mutex<std::collections::HashMap<String, Option<PathBuf>>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<String, Option<PathBuf>>>> =
        OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Where some *other* tool is, by the same three steps and the same cache
/// discipline as [`binary`].
///
/// [`install`](super::install) asks this about `brew`, `npm`, `bun` and
/// `curl`, and the login-shell fallback is the whole point for the first of
/// them: `brew` lives in `/opt/homebrew/bin`, which launchd's PATH does not
/// have, so a Dock-launched app that only read PATH would tell a Homebrew
/// user they have no Homebrew. A miss is cached like a hit, for the same
/// reason it is on the providers; [`forget`] drops both.
pub fn tool(name: &str) -> Option<PathBuf> {
    if let Some(hit) = tool_cache().lock().unwrap().get(name) {
        return hit.clone();
    }
    let found = search_path_env(name)
        .or_else(|| {
            well_known_dirs()
                .into_iter()
                .map(|d| d.join(name))
                .find(|p| is_executable(p))
        })
        .or_else(|| ask_login_shell(name));
    tool_cache().lock().unwrap().insert(name.to_string(), found.clone());
    found
}

/// Drop the cached lookups *and* the cached health, for a recheck after the
/// user installs one.
pub fn forget() {
    cache().lock().unwrap().clear();
    health_cache().lock().unwrap().clear();
    tool_cache().lock().unwrap().clear();
}

/// The `oculus` binary the child should find on its PATH. In a bundle it is
/// a sibling of the app executable; in `tauri dev` it is the debug binary the
/// preflight builds (`app/scripts/predev.mjs`), the release one `bun run cli`
/// leaves, or the `~/.local/bin` symlink from `cli:install`.
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
    // Newest wins, rather than first-found. This is the second half of a fix
    // whose first half is `app/scripts/predev.mjs`: `bun run cli` builds the
    // *release* binary, and until that preflight existed nothing built
    // `target/debug/oculus` at all, so under `tauri dev` the sibling of the
    // running app was whatever a stray `cargo test` last left there —
    // measured, ten days old with no `project` subcommand. Its directory goes
    // to the front of the thread's PATH (see `child_env`), so a bare
    // `oculus project create` from an agent died on `unrecognized subcommand`
    // while the real CLI sat one entry further along, and the agent's own
    // notes learned to reach past the name to an absolute path that no
    // permission rule matches. The preflight should keep all the candidates
    // current now; ordering them is what makes that safe rather than
    // load-bearing. A bundle has a single candidate, so this changes nothing
    // there.
    found.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    let chosen = found.pop();
    if let Some(bin) = &chosen {
        warn_if_stale(bin);
    }
    chosen
}

/// The `src/` directory of the checkout that built a
/// `<root>/target/{debug,release}/oculus`, or `None` for a CLI that came from
/// anywhere else — a bundle's sidecar, `~/.local/bin`, the PATH. Only a
/// checkout can be *out of date with its own sources*; an installed binary has
/// no sources to be behind.
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

/// The newest mtime under `dir`, counting `.rs` files only — the set cargo
/// would rebuild from.
fn newest_rs_mtime(dir: &Path) -> Option<std::time::SystemTime> {
    let mut newest: Option<std::time::SystemTime> = None;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
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

/// Say so, once, when the CLI a coding agent is about to be handed was built
/// before the sources sitting next to it.
///
/// `scripts/predev.mjs` builds the CLI at the start of every dev session and
/// `scripts/watch-cli.mjs` keeps it current through one, so this should never
/// fire. It exists because the failure it names is invisible from the other
/// end: a stale `oculus` runs, answers `--version`, and rejects a subcommand
/// it has simply never heard of, which reads as the agent doing something
/// wrong. A line in the dev terminal is the difference between that and an
/// afternoon. Nothing outside a checkout can reach here, so a bundle is
/// silent by construction.
fn warn_if_stale(bin: &Path) {
    static CHECKED: OnceLock<()> = OnceLock::new();
    CHECKED.get_or_init(|| {
        let Some(src) = dev_checkout_src(bin) else { return };
        let Ok(built) = std::fs::metadata(bin).and_then(|m| m.modified()) else { return };
        let Some(newest) = newest_rs_mtime(&src) else { return };
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

/// Where each binary is, which version it reports, and if it is not there,
/// why — cached for the life of the process beside the lookup itself.
///
/// The cache is what makes reading this cheap enough to ask on every composer.
/// The picker now dims a provider whose CLI is missing rather than offering
/// its catalogue, so `health` is read wherever a model is chosen, not only on
/// the Settings page; `binary` was already cached, but the `--version` spawn
/// behind it was not, and three of those on every menu is the same cost the
/// login-shell fallback was avoided for. [`forget`] clears both, which is what
/// Settings' *Recheck* does after an install.
pub fn health(provider: Provider) -> BridgeHealth {
    if let Some(h) = health_cache().lock().unwrap().get(&provider) {
        return h.clone();
    }
    let h = probe_health(provider);
    // Probed outside the lock: a `--version` is a process spawn, and two
    // callers racing it write the same answer twice rather than block.
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
                    // `2.1.267 (Claude Code)` / `codex-cli 0.153.4` — keep the
                    // number, drop the branding.
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

/// The environment a provider child gets.
///
/// Two deliberate edits to the inherited env. The API keys are stripped so
/// that a `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` in the user's shell cannot
/// silently move a subscription session onto per-token billing — the entire
/// reason this layer drives the CLIs rather than the APIs. The same strip is
/// what keeps opencode on the student's own `opencode auth` store rather than
/// on a key that happens to be in a shell: measured, a server started with
/// either name set grows a whole provider (16 anthropic models, 61 openai)
/// that nobody chose, so the same app would offer a different catalogue
/// launched from a terminal than from the Dock. And the `oculus` binary's
/// directory is put in front of PATH, because `AGENTS.md` tells the agent to
/// run `oculus grep`, and advice that resolves to "command not found" is
/// worse than none.
pub fn child_env() -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| {
            !matches!(
                k.as_str(),
                "ANTHROPIC_API_KEY"
                    | "OPENAI_API_KEY"
                    | "CLAUDE_AGENT_SDK_CLIENT_APP"
                    // Set when *this* process is itself a Claude Code child
                    // (a dev session run from inside one); the CLI refuses
                    // to nest and would exit immediately.
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

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("oculus-discover-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// The whole point of the guard is that it fires in a checkout and nowhere
    /// else: an installed binary has no `src/` to be behind, so reporting it as
    /// stale would be a warning nobody could act on.
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

        std::fs::remove_dir_all(&root).ok();
        std::fs::remove_dir_all(&bare).ok();
    }

    #[test]
    fn a_touched_source_is_newer_than_the_binary() {
        let root = scratch("mtime");
        std::fs::create_dir_all(root.join("src/harness")).unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        let bin = root.join("target/debug/oculus");
        std::fs::write(&bin, b"binary").unwrap();
        let built = std::fs::metadata(&bin).unwrap().modified().unwrap();

        // Nested, so the walk has to recurse to find it, and written after the
        // binary the way an edit lands after a build.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(root.join("src/harness/discover.rs"), b"fn main() {}").unwrap();
        let newest = newest_rs_mtime(&root.join("src")).unwrap();
        assert!(newest > built, "an edit after the build reads as newer");

        // Non-Rust files are not what cargo rebuilds from.
        std::fs::remove_file(root.join("src/harness/discover.rs")).unwrap();
        std::fs::write(root.join("src/notes.md"), b"# not a rebuild").unwrap();
        assert_eq!(newest_rs_mtime(&root.join("src")), None);

        std::fs::remove_dir_all(&root).ok();
    }
}
