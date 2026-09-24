//! Oculus's permission rules, kept in `agy`'s own global settings file.
//!
//! **Why the global file.** Claude takes its whole containment inline, as
//! `--settings <json>`; `agy` 1.2.9 takes rules from exactly one place,
//! `~/.gemini/antigravity-cli/settings.json` under `permissions.allow` /
//! `permissions.deny`. Measured, every other door was tried and none loads: a
//! workspace `.agents/hooks.json`, a project file under
//! `~/.gemini/config/projects/`, environment variables, and a `HOME` override
//! (which also loses the keychain sign-in). So Oculus writes a block into the
//! student's own file — which is why this module is careful about it:
//!
//! - It remembers exactly which entries **it** wrote last time
//!   ([`state_path`]), so a stale one — an old library path, a revoked
//!   approval — can be taken out again while an entry the student added by
//!   hand is never touched, even when it is spelled the same as one of ours.
//! - Every other key, and the order of every key, is kept as it was. The file
//!   is read as an ordered list of raw values and only `permissions.allow` and
//!   `permissions.deny` are re-rendered.
//! - A file that is not valid JSON is refused, never overwritten, and the
//!   spawn that needed it fails with the reason: running `agy` without its
//!   rules would be running it unconstrained.
//! - The write is a temp file and a rename, and only happens when something
//!   changed, so a spawn with nothing new to say leaves the file's mtime alone.
//!
//! **A live `agy` does not re-read the file** (measured: a rule added
//! mid-session is ignored until the process is respawned). So the rules are
//! written before every spawn, and an approval takes effect by dropping the
//! thread's process so the next message resumes it with `--conversation`.
//!
//! The rule syntax is `agy`'s: `write_file(/abs)` is recursive and implies
//! read, `read_file(/abs)`, `command(prefix)` matched word by word,
//! `read_url(domain)`. Deny beats ask beats allow. Grants under `read_file`
//! and `write_file` also widen the *terminal* sandbox's read and write
//! allowlists, which is what makes a shell `echo > file` and a bare `oculus`
//! work at all.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;
use serde_json::Value;

/// The `settings` row holding the student's own approvals: a JSON array of
/// rule strings, added from a thread's "allow" and removed from Settings.
pub const SETTINGS_KEY: &str = "antigravity_allowed_rules";

/// Commands that only read, allowed by name, so a turn is not refused over an
/// `ls` — measured: with no rules at all *every* `run_command` is refused,
/// `ls` included, and a refusal ends the turn.
///
/// The list is short **because the rules are global**: they live in the
/// student's own settings file, so they apply in the `agy` the student runs
/// in a terminal too, where the sandbox is usually off. A command belongs here
/// only if no *argument* can make it write or run something, since the
/// matcher is a word-by-word prefix and flags are just more words. A redirect
/// is not: measured without `--sandbox`, `command(cat)` allowed did *not*
/// cover `cat <lib>/notes.md > <outside>/cat.txt`, which was refused. `find`
/// and `rg` are out for that reason — `find … -delete` or `-exec rm`, and
/// `rg --pre <cmd>`, are all still `find …` and `rg …` to a prefix rule.
const READ_ONLY_COMMANDS: [&str; 7] = ["ls", "cat", "head", "tail", "wc", "grep", "pwd"];

/// The rule shapes a student may approve. `mcp(...)` is left out on purpose:
/// nothing in a thread's refusals suggests one, and an MCP server is not a
/// thing this app configures.
pub fn is_valid_rule(rule: &str) -> bool {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^(command|read_file|write_file|read_url)\(.+\)$").unwrap())
        .is_match(rule)
}

/// The student's approvals, from the `settings` row. A row that does not
/// parse is no approvals — the cost is a refusal the student can approve
/// again, never a rule they did not give.
pub async fn stored(pool: &sqlx::SqlitePool) -> Result<Vec<String>, String> {
    let raw: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?1")
        .bind(SETTINGS_KEY)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(raw
        .and_then(|r| serde_json::from_str::<Vec<String>>(&r).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|r| is_valid_rule(r))
        .collect())
}

pub async fn save(pool: &sqlx::SqlitePool, rules: &[String]) -> Result<(), String> {
    let value = serde_json::to_string(rules).map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(SETTINGS_KEY)
    .bind(value)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// One set of rules.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Rules {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

/// What Oculus wrote into the settings file last time, kept in the library.
///
/// `approved` is the approvals as they stood then, so a spawn that does not
/// read the database — the thread-naming throwaway, `oculus agent` — writes
/// the same block the last thread did instead of taking the student's
/// approvals out of the file and the next thread putting them back.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct State {
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    deny: Vec<String>,
    #[serde(default)]
    approved: Vec<String>,
}

/// `~/.gemini/antigravity-cli/settings.json` — the one place `agy` reads
/// rules from.
pub fn settings_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set, so agy's settings cannot be found")?;
    Ok(PathBuf::from(home).join(".gemini/antigravity-cli/settings.json"))
}

/// Where the record of Oculus's own entries lives: the library root, beside
/// `oculus.db` and the other app-owned files. Not `agents/`, which is the one
/// folder a thread may write — an agent that could edit this list could make
/// Oculus claim, and later delete, a rule the student wrote themselves. At the
/// root it is outside every bridge's writable roots, and Claude's `*.json`
/// deny already names it.
pub fn state_path(library: &Path) -> PathBuf {
    library.join("antigravity-rules.json")
}

/// The `oculus` binary as a thread's shell will meet it: the resolved file
/// `discover::oculus_cli` puts first on PATH, and the `~/.local/bin` launcher
/// when there is one.
///
/// Both matter. Measured: `oculus --version` inside `agy`'s sandbox fails with
/// `zsh:1: operation not permitted: oculus` — not a permission rule refusing
/// but the sandbox unable to *read* the binary, because `~/.local/bin/oculus`
/// is a symlink into `target/release/`, outside every readable root. With
/// `read_file` on the resolved directory and `command` on both spellings,
/// the bare name and the absolute path both print `oculus 0.1.0`.
pub struct OculusCli {
    /// Canonical path of the binary.
    pub bin: PathBuf,
    /// `~/.local/bin/oculus`, when it exists.
    pub launcher: Option<PathBuf>,
}

impl OculusCli {
    pub fn discover() -> Option<OculusCli> {
        let found = super::discover::oculus_cli()?;
        let bin = found.canonicalize().unwrap_or(found);
        let launcher = std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join(".local/bin/oculus"))
            .filter(|p| std::fs::symlink_metadata(p).is_ok());
        Some(OculusCli { bin, launcher })
    }
}

/// Oculus's rules for a thread over `library`, plus the student's approvals.
///
/// The mirror of Claude's `settings_json` in `claude.rs`, translated:
///
/// - **Writes**: the database's three files and nothing else outside the
///   workspace. The workspace is `agents/` (the cwd), and measured with no
///   rules, `write_to_file` there already lands under `--mode accept-edits`
///   while one in the library beside it is refused — so, like Claude's
///   `allowWrite` of cwd plus `paths::db_write_paths`, only the three files
///   are named. A single file is enough for the shell too: measured, with
///   this set a thread's `echo hi >> <library>/oculus.db-wal` lands, and its
///   bare `oculus --version` prints the version.
/// - **Denies**: Claude's `Edit(//…)` list as `write_file(…)`, and its
///   `Bash(sqlite3:*)` as `command(sqlite3 <db>)` — the database is writable so the
///   CLI can reach it, and a hand-written `UPDATE` is the one way around the
///   CLI. Claude's root globs (`*.cookie`, `*.token`, `*.log`, `*.json`) are
///   named as the files they stand for, since no glob in `agy`'s syntax has
///   been measured.
/// - **The CLI**: see [`OculusCli`].
pub fn rules_for(library: &Path, oculus: Option<&OculusCli>, approved: &[String]) -> Rules {
    let at = |rel: &str| library.join(rel).display().to_string();
    let mut allow: Vec<String> = crate::paths::db_write_paths(library)
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

    let mut deny: Vec<String> = [
        "courses",
        "lectures",
        "canvas-session",
        // The app's own files inside the writable workspace: the generated
        // skills and both folders a scanning CLI discovers them through. See
        // the same list in `claude.rs`.
        "agents/skills",
        "agents/.claude",
        "agents/.agents",
    ]
    .iter()
    .map(|p| format!("write_file({})", at(p)))
    .collect();
    deny.push(format!("write_file({})", crate::paths::cookie_path(library).display()));
    deny.push(format!("write_file({})", at("ed-session.token")));
    deny.push(format!("write_file({})", crate::paths::keepalive_log_path(library).display()));
    deny.push(format!("write_file({})", state_path(library).display()));
    // `sqlite3` pointed at *this* database, not `sqlite3` at large: the rules
    // are global, and a blanket `command(sqlite3)` — re-added on every spawn —
    // would ban the tool from every one of the student's own `agy` sessions.
    // Measured without `--sandbox`: `command(sqlite3 <lib>/oculus.db)` refused
    // `sqlite3 <lib>/oculus.db 'select 1'` while `sqlite3 :memory: 'select 2'`
    // ran; a `command(regex:^sqlite3\s.*<lib>)` deny had no effect at all, so
    // no regex rules. Both spellings of the path are named when they differ
    // (`/tmp` is `/private/tmp` on macOS). The gap is a `cd` into the library
    // and a relative path, which no prefix can see — like Claude's
    // `Bash(sqlite3:*)`, this is a speed bump in front of the CLI, which is
    // the intended door, not a wall.
    for db in sqlite_paths(library) {
        deny.push(format!("command(sqlite3 {db})"));
    }

    Rules {
        allow: dedupe(allow),
        deny: dedupe(deny),
    }
}

/// `oculus.db` as a command line may spell it: as given, and resolved.
///
/// The real library is under `~/Library/Application Support`, and a path with
/// a space reaches a command line quoted or escaped. How `agy` splits a rule
/// into words is not measured for quotes, so a path with whitespace is also
/// named in the three ways a shell would write it — each is just one more
/// deny, and a spelling that never matches costs nothing.
fn sqlite_paths(library: &Path) -> Vec<String> {
    let db = crate::paths::db_path(library);
    let mut plain = vec![db.display().to_string()];
    if let Ok(real) = db.canonicalize().or_else(|_| {
        library.canonicalize().map(|l| crate::paths::db_path(&l))
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
/// would change nothing — deny beats allow in `agy`. Said up front rather
/// than stored as an approval that silently does not work.
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
        // A command rule is a word-by-word prefix, so an allow is shadowed
        // when the deny is a prefix of it — `command(sqlite3 /lib/oculus.db
        // .dump)` under `command(sqlite3 /lib/oculus.db)` — but not when it is
        // wider than the deny: `command(sqlite3)` still allows `:memory:`.
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

/// Write the rules for a spawn over `library`.
///
/// `approved` is the student's approvals as read from the database, or `None`
/// from a caller that has no database to hand — which then gets the ones the
/// last write used ([`State::approved`]).
pub fn install(library: &Path, approved: Option<Vec<String>>) -> Result<(), String> {
    // One writer at a time inside this process: a thread and its namer can
    // spawn together, and both read-modify-write the same file.
    static LOCK: Mutex<()> = Mutex::new(());
    let _held = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    install_at(
        &settings_path()?,
        &state_path(library),
        library,
        OculusCli::discover().as_ref(),
        approved,
    )
}

fn install_at(
    settings: &Path,
    state_file: &Path,
    library: &Path,
    oculus: Option<&OculusCli>,
    approved: Option<Vec<String>>,
) -> Result<(), String> {
    // A missing or unreadable record is an empty one: the worst that costs is
    // one stale entry left behind, never a student's entry taken out.
    let last: State = std::fs::read_to_string(state_file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let approved = approved.unwrap_or_else(|| last.approved.clone());
    let rules = rules_for(library, oculus, &approved);
    let managed = apply(
        settings,
        &Rules {
            allow: last.allow.clone(),
            deny: last.deny.clone(),
        },
        &rules,
    )?;
    let next = State {
        allow: managed.allow,
        deny: managed.deny,
        approved,
    };
    if next != last {
        let body = serde_json::to_string_pretty(&next).map_err(|e| e.to_string())?;
        write_atomic(state_file, &format!("{body}\n"))
            .map_err(|e| format!("cannot record Antigravity's rules in {}: {e}", state_file.display()))?;
    }
    Ok(())
}

/// Merge `rules` into the settings file at `path`, given what Oculus wrote
/// last time, and return what Oculus owns now.
///
/// Per list: an entry the student wrote (present, and not in `last`) stays
/// where it is; an entry of Oculus's that is still wanted stays where it is;
/// one that is no longer wanted goes; a new one is appended. Oculus owns only
/// what it added — an entry the student already had is theirs, so it survives
/// the day Oculus stops asking for it.
fn apply(path: &Path, last: &Rules, rules: &Rules) -> Result<Rules, String> {
    // Through a symlink (a dotfiles repo) to the file itself, so the rename
    // replaces the file rather than the link.
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let refuse = |why: String| {
        format!(
            "{} is not valid JSON ({why}), so Oculus will not write Antigravity's permission \
             rules into it — fix or remove the file and send again",
            path.display()
        )
    };
    let mut top: Vec<(String, Node)> = match text.as_deref() {
        None => Vec::new(),
        Some(t) if t.trim().is_empty() => Vec::new(),
        Some(t) => {
            let Ordered(entries) = serde_json::from_str(t).map_err(|e| refuse(e.to_string()))?;
            entries.into_iter().map(|(k, v)| (k, Node::Raw(v))).collect()
        }
    };

    // `permissions`, opened one level so its other keys (`ask`, anything a
    // later `agy` adds) ride through untouched.
    let mut perms: Vec<(String, Node)> = match top.iter().find(|(k, _)| k == "permissions") {
        Some((_, Node::Raw(raw))) => {
            let Ordered(entries) = serde_json::from_str(raw.get())
                .map_err(|_| refuse("`permissions` is not an object".into()))?;
            entries.into_iter().map(|(k, v)| (k, Node::Raw(v))).collect()
        }
        _ => Vec::new(),
    };
    let list = |perms: &[(String, Node)], key: &str| -> Result<Vec<String>, String> {
        match perms.iter().find(|(k, _)| k == key) {
            Some((_, Node::Raw(raw))) => serde_json::from_str::<Vec<String>>(raw.get())
                .map_err(|_| refuse(format!("`permissions.{key}` is not a list of strings"))),
            _ => Ok(Vec::new()),
        }
    };
    let (had_allow, had_deny) = (list(&perms, "allow")?, list(&perms, "deny")?);
    let (allow, own_allow) = merge(&had_allow, &last.allow, &rules.allow);
    let (deny, own_deny) = merge(&had_deny, &last.deny, &rules.deny);
    let owned = Rules {
        allow: own_allow,
        deny: own_deny,
    };

    let present = |perms: &[(String, Node)], key: &str| perms.iter().any(|(k, _)| k == key);
    if text.is_some()
        && allow == had_allow
        && deny == had_deny
        && present(&perms, "allow")
        && present(&perms, "deny")
    {
        return Ok(owned);
    }

    set(&mut perms, "allow", Node::Value(Value::from(allow)));
    set(&mut perms, "deny", Node::Value(Value::from(deny)));
    set(&mut top, "permissions", Node::Map(perms));
    let mut body = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(
        &mut body,
        serde_json::ser::PrettyFormatter::with_indent(b"  "),
    );
    Node::Map(top).serialize(&mut ser).map_err(|e| e.to_string())?;
    body.push(b'\n');
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    write_atomic(&path, &String::from_utf8_lossy(&body))
        .map_err(|e| format!("cannot write Antigravity's rules to {}: {e}", path.display()))?;
    Ok(owned)
}

/// One list's merge; see [`apply`]. Returns the new list and Oculus's share
/// of it.
fn merge(existing: &[String], last: &[String], wanted: &[String]) -> (Vec<String>, Vec<String>) {
    let last: HashSet<&str> = last.iter().map(String::as_str).collect();
    let wanted_set: HashSet<&str> = wanted.iter().map(String::as_str).collect();
    let theirs: HashSet<&str> = existing
        .iter()
        .map(String::as_str)
        .filter(|e| !last.contains(e))
        .collect();
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for e in existing {
        if (theirs.contains(e.as_str()) || wanted_set.contains(e.as_str())) && seen.insert(e.clone()) {
            out.push(e.clone());
        }
    }
    for w in wanted {
        if seen.insert(w.clone()) {
            out.push(w.clone());
        }
    }
    let ours = wanted
        .iter()
        .filter(|w| !theirs.contains(w.as_str()))
        .cloned()
        .collect();
    (out, dedupe(ours))
}

fn dedupe(v: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    v.into_iter().filter(|s| seen.insert(s.clone())).collect()
}

fn set(entries: &mut Vec<(String, Node)>, key: &str, value: Node) {
    match entries.iter_mut().find(|(k, _)| k == key) {
        Some((_, v)) => *v = value,
        None => entries.push((key.to_string(), value)),
    }
}

/// Temp file beside the target, then a rename: `agy` reading the file while
/// it is written sees the old one or the new one, never half of either. The
/// original's permission bits are carried over.
fn write_atomic(path: &Path, body: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".oculus-{}.tmp", std::process::id()));
    let tmp = PathBuf::from(tmp);
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(body.as_bytes())?;
        f.sync_all()?;
    }
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&tmp, meta.permissions());
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

// ── Order-keeping JSON ───────────────────────────────────────────────────────
//
// `serde_json::Map` sorts its keys (this crate does not enable
// `preserve_order`, and turning it on would reorder every other JSON the app
// writes), so the student's file is read as an ordered list of raw values and
// written back in the same order, with only the two lists re-rendered.

/// A JSON object read as its entries, in file order, values untouched.
struct Ordered(Vec<(String, Box<RawValue>)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Ordered;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, Box<RawValue>>()? {
                    out.push((k, v));
                }
                Ok(Ordered(out))
            }
        }
        d.deserialize_map(V)
    }
}

/// A value on its way back out: verbatim, re-rendered, or an object of
/// either.
enum Node {
    Raw(Box<RawValue>),
    Value(Value),
    Map(Vec<(String, Node)>),
}

impl Serialize for Node {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Node::Raw(r) => r.serialize(s),
            Node::Value(v) => v.serialize(s),
            Node::Map(entries) => {
                let mut m = s.serialize_map(Some(entries.len()))?;
                for (k, v) in entries {
                    m.serialize_entry(k, v)?;
                }
                m.end()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory under the system temp dir. Never `~/.gemini`: every
    /// test here writes only below this.
    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("oculus-agy-rules-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn rules(allow: &[&str], deny: &[&str]) -> Rules {
        Rules {
            allow: allow.iter().map(|s| s.to_string()).collect(),
            deny: deny.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn read(p: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    }

    #[test]
    fn a_missing_file_is_created_with_only_permissions() {
        let d = scratch("fresh");
        let path = d.join("nested/settings.json");
        let owned = apply(&path, &Rules::default(), &rules(&["command(ls)"], &["command(sqlite3)"])).unwrap();
        assert_eq!(owned, rules(&["command(ls)"], &["command(sqlite3)"]));
        assert_eq!(
            read(&path),
            serde_json::json!({"permissions": {"allow": ["command(ls)"], "deny": ["command(sqlite3)"]}})
        );
        // Two-space indentation, as asked.
        assert!(std::fs::read_to_string(&path).unwrap().contains("\n  \"permissions\""));
    }

    /// The student's keys keep their values and their order, and their own
    /// rules stay where they were — including one spelled the same as ours,
    /// which stays theirs.
    #[test]
    fn the_students_keys_and_rules_are_kept() {
        let d = scratch("merge");
        let path = d.join("settings.json");
        std::fs::write(
            &path,
            r#"{
  "trustedWorkspaces": ["/w"],
  "colorScheme": "light",
  "permissions": { "ask": ["command(git)"], "allow": ["command(make)", "command(ls)"] }
}"#,
        )
        .unwrap();
        let owned = apply(&path, &Rules::default(), &rules(&["command(ls)", "command(oculus)"], &["command(sqlite3)"])).unwrap();
        // `command(ls)` was the student's before Oculus asked for it.
        assert_eq!(owned.allow, ["command(oculus)"]);
        let text = std::fs::read_to_string(&path).unwrap();
        let t = text.find("trustedWorkspaces").unwrap();
        let c = text.find("colorScheme").unwrap();
        let p = text.find("permissions").unwrap();
        assert!(t < c && c < p, "key order kept: {text}");
        let v = read(&path);
        assert_eq!(v["colorScheme"], "light");
        assert_eq!(v["trustedWorkspaces"], serde_json::json!(["/w"]));
        assert_eq!(v["permissions"]["ask"], serde_json::json!(["command(git)"]));
        assert_eq!(
            v["permissions"]["allow"],
            serde_json::json!(["command(make)", "command(ls)", "command(oculus)"])
        );
        assert_eq!(v["permissions"]["deny"], serde_json::json!(["command(sqlite3)"]));
    }

    #[test]
    fn a_stale_entry_of_ours_is_taken_out_and_theirs_is_not() {
        let d = scratch("stale");
        let path = d.join("settings.json");
        std::fs::write(
            &path,
            r#"{"permissions":{"allow":["write_file(/old/lib/oculus.db)","command(make)","command(oculus)"]}}"#,
        )
        .unwrap();
        let last = rules(&["write_file(/old/lib/oculus.db)", "command(oculus)"], &[]);
        let owned = apply(&path, &last, &rules(&["write_file(/new/lib/oculus.db)", "command(oculus)"], &[])).unwrap();
        assert_eq!(
            read(&path)["permissions"]["allow"],
            serde_json::json!(["command(make)", "command(oculus)", "write_file(/new/lib/oculus.db)"])
        );
        assert_eq!(owned.allow, ["write_file(/new/lib/oculus.db)", "command(oculus)"]);
    }

    #[test]
    fn invalid_json_is_refused_and_left_alone() {
        let d = scratch("invalid");
        let path = d.join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        let err = apply(&path, &Rules::default(), &rules(&["command(ls)"], &[])).unwrap_err();
        assert!(err.contains("not valid JSON"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
        // And a `permissions` of the wrong shape is the same refusal.
        std::fs::write(&path, r#"{"permissions":{"allow":"command(ls)"}}"#).unwrap();
        assert!(apply(&path, &Rules::default(), &rules(&["command(ls)"], &[])).is_err());
    }

    #[test]
    fn a_second_identical_write_changes_nothing() {
        let d = scratch("idempotent");
        let path = d.join("settings.json");
        std::fs::write(&path, r#"{"colorScheme":"dark"}"#).unwrap();
        let want = rules(&["command(oculus)"], &["command(sqlite3)"]);
        let owned = apply(&path, &Rules::default(), &want).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let again = apply(&path, &owned, &want).unwrap();
        assert_eq!(again, owned);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), mtime, "not rewritten");
    }

    /// The whole round trip through the state file: a revoked approval leaves
    /// the settings file, and a spawn that passes `None` keeps the last ones.
    #[test]
    fn approvals_come_and_go_through_the_record() {
        let d = scratch("install");
        let (settings, state, lib) = (d.join("settings.json"), d.join("state.json"), d.join("lib"));
        install_at(&settings, &state, &lib, None, Some(vec!["command(python3)".into()])).unwrap();
        let allow = |p: &Path| read(p)["permissions"]["allow"].clone();
        assert!(allow(&settings).as_array().unwrap().contains(&"command(python3)".into()));
        // The namer: no database, same block.
        install_at(&settings, &state, &lib, None, None).unwrap();
        assert!(allow(&settings).as_array().unwrap().contains(&"command(python3)".into()));
        // Revoked.
        install_at(&settings, &state, &lib, None, Some(vec![])).unwrap();
        assert!(!allow(&settings).as_array().unwrap().contains(&"command(python3)".into()));
    }

    #[test]
    fn the_rule_set_mirrors_claudes() {
        let lib = Path::new("/lib");
        let cli = OculusCli {
            bin: PathBuf::from("/repo/target/release/oculus"),
            launcher: Some(PathBuf::from("/home/u/.local/bin/oculus")),
        };
        let r = rules_for(lib, Some(&cli), &["command(python3)".into()]);
        for want in [
            "write_file(/lib/oculus.db)",
            "write_file(/lib/oculus.db-wal)",
            "write_file(/lib/oculus.db-shm)",
            "read_file(/repo/target/release)",
            "read_file(/home/u/.local/bin)",
            "command(oculus)",
            "command(/repo/target/release/oculus)",
            "command(/home/u/.local/bin/oculus)",
            "command(ls)",
            "command(python3)",
        ] {
            assert!(r.allow.iter().any(|a| a == want), "allow {want}: {:?}", r.allow);
        }
        // Nothing grants the library or the workspace wholesale, and nothing
        // whose arguments can write or run something (the rules are global).
        assert!(!r.allow.iter().any(|a| a == "write_file(/lib)" || a == "write_file(/lib/agents)"));
        assert!(!r.allow.iter().any(|a| a == "command(find)" || a == "command(rg)"));
        // `sqlite3` is shut on this database only, never in the student's
        // own sessions at large.
        assert!(!r.deny.iter().any(|d| d == "command(sqlite3)"));
        for want in [
            "write_file(/lib/courses)",
            "write_file(/lib/agents/skills)",
            "write_file(/lib/canvas-session.cookie)",
            "write_file(/lib/antigravity-rules.json)",
            "command(sqlite3 /lib/oculus.db)",
        ] {
            assert!(r.deny.iter().any(|a| a == want), "deny {want}: {:?}", r.deny);
        }
    }

    /// A library whose path resolves elsewhere (`/var` → `/private/var` on
    /// macOS) has its database denied under both spellings.
    #[test]
    fn sqlite3_is_denied_on_both_spellings_of_the_database() {
        let lib = scratch("spellings");
        let real = lib.canonicalize().unwrap();
        let r = rules_for(&lib, None, &[]);
        let want = |l: &Path| format!("command(sqlite3 {})", l.join("oculus.db").display());
        assert!(r.deny.contains(&want(&lib)), "{:?}", r.deny);
        assert!(r.deny.contains(&want(&real)), "{:?}", r.deny);
    }

    /// The real library has a space in its path; the deny names the quoted
    /// and escaped spellings beside the bare one.
    #[test]
    fn a_database_path_with_a_space_is_denied_however_it_is_quoted() {
        let lib = Path::new("/Users/s/Library/Application Support/com.tchan.oculus");
        let deny = rules_for(lib, None, &[]).deny;
        for want in [
            "command(sqlite3 /Users/s/Library/Application Support/com.tchan.oculus/oculus.db)",
            "command(sqlite3 \"/Users/s/Library/Application Support/com.tchan.oculus/oculus.db\")",
            "command(sqlite3 '/Users/s/Library/Application Support/com.tchan.oculus/oculus.db')",
            "command(sqlite3 /Users/s/Library/Application\\ Support/com.tchan.oculus/oculus.db)",
        ] {
            assert!(deny.iter().any(|d| d == want), "{want}: {deny:?}");
        }
    }

    #[test]
    fn rules_are_validated_and_denies_win() {
        for ok in ["command(python3)", "write_file(/a/b)", "read_file(/a)", "read_url(example.com)"] {
            assert!(is_valid_rule(ok), "{ok}");
        }
        for bad in ["command()", "mcp(x/y)", "Bash(ls)", "command(ls)\ncommand(rm)", "command(ls) "] {
            assert!(!is_valid_rule(bad), "{bad:?}");
        }
        let lib = Path::new("/lib");
        assert!(denied_by(lib, "write_file(/lib/courses/COMP30026)").is_some());
        assert!(denied_by(lib, "command(sqlite3 /lib/oculus.db)").is_some());
        assert!(denied_by(lib, "command(sqlite3 /lib/oculus.db .dump)").is_some());
        // Wider than the deny: still allows `sqlite3 :memory:`, so not refused.
        assert!(denied_by(lib, "command(sqlite3)").is_none());
        assert!(denied_by(lib, "write_file(/lib/agents/notes)").is_none());
        assert!(denied_by(lib, "command(python3)").is_none());
    }
}
