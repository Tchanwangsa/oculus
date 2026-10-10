//! The `oculus` CLI agents are told to run, and a warning when it is stale.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::locate::{home, is_executable, search_path_env};

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
pub(super) fn dev_checkout_src(bin: &Path) -> Option<PathBuf> {
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
pub(super) fn newest_rs_mtime(dir: &Path) -> Option<std::time::SystemTime> {
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
