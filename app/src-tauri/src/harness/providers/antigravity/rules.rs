//! Oculus's permission rules, kept in `agy`'s own global settings file
//! (`~/.gemini/antigravity-cli/settings.json`, `permissions.allow`/`deny`) —
//! the only place agy 1.2.9 reads rules from. See docs/harness.md.
//!
//! Because the file is the student's: Oculus records which entries it wrote
//! ([`state_path`]) and only ever removes those; every other key keeps its
//! value and order; invalid JSON is refused (and the spawn fails) rather than
//! overwritten; the write is atomic and skipped when nothing changed. A live
//! `agy` never re-reads the file, so rules are written before every spawn.
//!
//! Rule syntax is agy's: `write_file(/abs)` (recursive, implies read),
//! `read_file(/abs)`, `command(prefix)` matched word by word, `read_url(host)`.
//! Deny beats allow. File grants also widen the terminal sandbox.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::harness::protected::{named_root_files, LIBRARY_DIRS, WORKSPACE_DIRS};

pub use super::install::install;

/// The `settings` row holding the student's approvals, a JSON array of rules.
pub const SETTINGS_KEY: &str = "antigravity_allowed_rules";

/// Commands allowed by name (with no rules, every `run_command` is refused).
/// The rules are global — they apply to the student's own terminal `agy` too
/// — so only commands no argument can make write or run something: not `find`
/// (`-delete`, `-exec`) or `rg` (`--pre`).
const READ_ONLY_COMMANDS: [&str; 7] = ["ls", "cat", "head", "tail", "wc", "grep", "pwd"];

/// The rule shapes a student may approve (no `mcp(...)`).
pub fn is_valid_rule(rule: &str) -> bool {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"^(command|read_file|write_file|read_url)\(.+\)$").unwrap()
    })
    .is_match(rule)
}

/// The student's approvals; a row that does not parse is none.
pub async fn stored(pool: &sqlx::SqlitePool) -> Result<Vec<String>, String> {
    let raw = crate::db::store::setting(pool, SETTINGS_KEY).await?;
    Ok(raw
        .and_then(|r| serde_json::from_str::<Vec<String>>(&r).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|r| is_valid_rule(r))
        .collect())
}

pub async fn save(pool: &sqlx::SqlitePool, rules: &[String]) -> Result<(), String> {
    let value = serde_json::to_string(rules).map_err(|e| e.to_string())?;
    crate::db::store::set_setting(pool, SETTINGS_KEY, &value).await
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Rules {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

/// What Oculus wrote into the settings file last time. `approved` lets a
/// spawn with no database (the namer, `oculus agent`) rewrite the same block.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct State {
    #[serde(default)]
    pub(super) allow: Vec<String>,
    #[serde(default)]
    pub(super) deny: Vec<String>,
    #[serde(default)]
    pub(super) approved: Vec<String>,
}

pub fn settings_path() -> Result<PathBuf, String> {
    let home =
        std::env::var_os("HOME").ok_or("HOME is not set, so agy's settings cannot be found")?;
    Ok(PathBuf::from(home).join(".gemini/antigravity-cli/settings.json"))
}

/// The record of Oculus's own entries, at the library root — outside every
/// agent's writable roots, so no agent can make Oculus delete a student rule.
pub fn state_path(library: &Path) -> PathBuf {
    library.join("antigravity-rules.json")
}

/// The `oculus` binary and its `~/.local/bin` launcher. The sandbox must be
/// able to *read* the resolved binary behind the symlink, or a bare `oculus`
/// fails with `operation not permitted`.
pub struct OculusCli {
    pub bin: PathBuf,
    pub launcher: Option<PathBuf>,
}

impl OculusCli {
    pub fn discover() -> Option<OculusCli> {
        let found = crate::harness::cli::discover::oculus_cli()?;
        let bin = found.canonicalize().unwrap_or(found);
        let launcher = std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join(".local/bin/oculus"))
            .filter(|p| std::fs::symlink_metadata(p).is_ok());
        Some(OculusCli { bin, launcher })
    }
}

/// Oculus's rules for a thread over `library`, plus the student's approvals:
/// Claude's `settings_json` (`claude/settings.rs`) in agy's syntax. The workspace
/// (`agents/`) is writable under `accept-edits` already, so only the
/// database's files are granted.
pub fn rules_for(library: &Path, oculus: Option<&OculusCli>, approved: &[String]) -> Rules {
    let at = |rel: &str| library.join(rel).display().to_string();
    let mut allow: Vec<String> = crate::library::paths::db_write_paths(library)
        .iter()
        .map(|p| format!("write_file({})", p.display()))
        .collect();
    if let Some(cli) = oculus {
        if let Some(dir) = cli.bin.parent() {
            allow.push(format!("read_file({})", dir.display()));
        }
        if let Some(dir) = cli.launcher.as_deref().and_then(Path::parent) {
            allow.push(format!("read_file({})", dir.display()));
        }
        allow.push("command(oculus)".into());
        allow.push(format!("command({})", cli.bin.display()));
        if let Some(l) = &cli.launcher {
            allow.push(format!("command({})", l.display()));
        }
    }
    allow.extend(READ_ONLY_COMMANDS.iter().map(|c| format!("command({c})")));
    allow.extend(approved.iter().cloned());

    let mut deny: Vec<String> = LIBRARY_DIRS
        .iter()
        .map(|d| at(d))
        .chain(WORKSPACE_DIRS.iter().map(|d| at(&format!("agents/{d}"))))
        .map(|p| format!("write_file({p})"))
        .collect();
    // `ROOT_FILE_GLOBS` as the files they cover.
    deny.extend(
        named_root_files(library)
            .iter()
            .map(|p| format!("write_file({})", p.display())),
    );
    deny.push(format!("write_file({})", state_path(library).display()));
    // `sqlite3` on *this* database only: the rules are global, and a blanket
    // deny would ban it from the student's own sessions. A speed bump in front
    // of the CLI, not a wall (a `cd` and a relative path pass). agy ignores
    // `regex:` denies.
    for db in sqlite_paths(library) {
        deny.push(format!("command(sqlite3 {db})"));
    }

    Rules {
        allow: dedupe(allow),
        deny: dedupe(deny),
    }
}

/// `oculus.db` as a command line may spell it: as given and resolved, and a
/// path with a space also double-quoted, single-quoted and escaped.
fn sqlite_paths(library: &Path) -> Vec<String> {
    let db = crate::library::paths::db_path(library);
    let mut plain = vec![db.display().to_string()];
    if let Ok(real) = db.canonicalize().or_else(|_| {
        library
            .canonicalize()
            .map(|l| crate::library::paths::db_path(&l))
    }) {
        plain.push(real.display().to_string());
    }
    let mut out = Vec::new();
    for p in dedupe(plain) {
        if p.contains(char::is_whitespace) {
            out.push(format!("\"{p}\""));
            out.push(format!("'{p}'"));
            out.push(p.replace(' ', "\\ "));
        }
        out.push(p);
    }
    dedupe(out)
}

/// Whether one of Oculus's own denies already covers `rule`, so approving it
/// would change nothing.
pub fn denied_by(library: &Path, rule: &str) -> Option<String> {
    let deny = rules_for(library, None, &[]).deny;
    let inner = |r: &str, head: &str| {
        r.strip_prefix(head)
            .and_then(|s| s.strip_suffix(')'))
            .map(String::from)
    };
    for d in &deny {
        if d == rule {
            return Some(d.clone());
        }
        if let (Some(want), Some(closed)) = (inner(rule, "write_file("), inner(d, "write_file(")) {
            if Path::new(&want).starts_with(&closed) {
                return Some(d.clone());
            }
        }
        // Shadowed when the deny is a word prefix of it; a wider allow is not.
        if let (Some(want), Some(closed)) = (inner(rule, "command("), inner(d, "command(")) {
            let want: Vec<&str> = want.split_whitespace().collect();
            let closed: Vec<&str> = closed.split_whitespace().collect();
            if !closed.is_empty() && want.starts_with(&closed) {
                return Some(d.clone());
            }
        }
    }
    None
}

pub(super) fn dedupe(v: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    v.into_iter().filter(|s| seen.insert(s.clone())).collect()
}
