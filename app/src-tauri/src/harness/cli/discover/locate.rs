//! Locating an executable: override, PATH, well-known dirs, login shell.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::{binary_name, override_env};
use crate::harness::event::Provider;

pub(super) fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Where the installers put things, in the order worth trying.
pub(super) fn well_known_dirs() -> Vec<PathBuf> {
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

pub(super) fn is_executable(p: &Path) -> bool {
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

pub(super) fn search_path_env(name: &str) -> Option<PathBuf> {
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
    super::health::health_cache().lock().unwrap().clear();
    tool_cache().lock().unwrap().clear();
    crate::harness::cli::update::forget();
}
