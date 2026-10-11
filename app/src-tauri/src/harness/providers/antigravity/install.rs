//! Writing the rules into agy's settings file without disturbing the rest of
//! it: merge against the record of what Oculus wrote last time.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use serde_json::Value;

use super::json::{Node, Ordered};
use super::rules::{dedupe, rules_for, settings_path, state_path, OculusCli, Rules, State};

/// Write the rules for a spawn over `library`. `None` approvals reuses the
/// last written ([`State::approved`]).
pub fn install(library: &Path, approved: Option<Vec<String>>) -> Result<(), String> {
    // A thread and its namer can spawn together.
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

pub(super) fn install_at(
    settings: &Path,
    state_file: &Path,
    library: &Path,
    oculus: Option<&OculusCli>,
    approved: Option<Vec<String>>,
) -> Result<(), String> {
    // Missing is empty: at worst a stale entry stays, never a student's goes.
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
        write_atomic(state_file, &format!("{body}\n")).map_err(|e| {
            format!(
                "cannot record Antigravity's rules in {}: {e}",
                state_file.display()
            )
        })?;
    }
    Ok(())
}

/// Merge `rules` into the settings file at `path`, given what Oculus wrote
/// last time, and return what Oculus owns now. Student entries stay put;
/// Oculus's unwanted ones go; new ones are appended. An entry the student
/// already had stays theirs.
pub(super) fn apply(path: &Path, last: &Rules, rules: &Rules) -> Result<Rules, String> {
    // Rename onto the file, not a dotfiles symlink.
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
            entries
                .into_iter()
                .map(|(k, v)| (k, Node::Raw(v)))
                .collect()
        }
    };

    // Opened one level so its other keys (`ask`, …) pass through untouched.
    let mut perms: Vec<(String, Node)> = match top.iter().find(|(k, _)| k == "permissions") {
        Some((_, Node::Raw(raw))) => {
            let Ordered(entries) = serde_json::from_str(raw.get())
                .map_err(|_| refuse("`permissions` is not an object".into()))?;
            entries
                .into_iter()
                .map(|(k, v)| (k, Node::Raw(v)))
                .collect()
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
    Node::Map(top)
        .serialize(&mut ser)
        .map_err(|e| e.to_string())?;
    body.push(b'\n');
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    write_atomic(&path, &String::from_utf8_lossy(&body)).map_err(|e| {
        format!(
            "cannot write Antigravity's rules to {}: {e}",
            path.display()
        )
    })?;
    Ok(owned)
}

/// One list's merge; see [`apply`]. Returns the new list and Oculus's share
/// of it.
pub(super) fn merge(
    existing: &[String],
    last: &[String],
    wanted: &[String],
) -> (Vec<String>, Vec<String>) {
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
        if (theirs.contains(e.as_str()) || wanted_set.contains(e.as_str()))
            && seen.insert(e.clone())
        {
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

pub(super) fn set(entries: &mut Vec<(String, Node)>, key: &str, value: Node) {
    match entries.iter_mut().find(|(k, _)| k == key) {
        Some((_, v)) => *v = value,
        None => entries.push((key.to_string(), value)),
    }
}

/// Temp file beside the target, then a rename; permission bits carried over.
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
