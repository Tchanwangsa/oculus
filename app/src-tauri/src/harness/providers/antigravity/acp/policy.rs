//! Rust's answer to `session/request_permission`, before anyone is asked.
//! Under `agy acp` this is the only gate: the agent's own rules neither skip
//! nor block a request (stage 5 probe), so what Rust allows runs and what it
//! refuses does not. In `default` mode every write and every command asks.
//!
//! - A write naming a protected path (`protected.rs`, by way of
//!   `requests/policy.rs`) or the database is refused, unseen.
//! - A write whose every path is under the chat's write root (`agents/`,
//!   `studio/`) or in a subject's own files (`policy::in_own_zone`) is
//!   allowed, unseen, and so is a plain `oculus …` or `ls …`.
//! - Anything else is put to the student (a request), or refused where
//!   nobody can be asked.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::harness::child::str_of as s;
use crate::harness::requests::{policy, Action, Decision, NewRequest, RequestOption};

/// What Rust does with one permission request.
#[derive(Debug, PartialEq)]
pub(super) enum Verdict {
    /// Answered Allow, nobody asked.
    Allow,
    /// Answered Deny, nobody asked, for the reason given (logged).
    Deny(String),
    /// The student decides, where a student can be asked.
    Ask(NewRequest),
}

/// The tools that write files, by name, beside ACP's own write kinds.
const WRITE_TOOLS: [&str; 8] = [
    "write_file",
    "edit_file",
    "write_to_file",
    "replace_file_content",
    "multi_replace_file_content",
    "delete_file",
    "move_file",
    "sed_file",
];

/// `rawInput` keys that name a file a tool touches. Not `Target`, which is
/// `edit_file`'s text to replace.
const PATH_KEYS: [&str; 8] = [
    "Path",
    "TargetFile",
    "AbsolutePath",
    "DirectoryPath",
    "SourcePath",
    "DestinationPath",
    "FilePath",
    "File",
];

/// The commands Rust allows unasked, as the first and only command of a
/// line: `oculus` refuses its own risky verbs to an agent (`OCULUS_AGENT`),
/// and `ls` reads.
const ALLOWED_COMMANDS: [&str; 2] = ["oculus", "ls"];

/// Characters that make a line more than one plain command: separators,
/// pipes, redirects, substitutions and escapes.
const SHELL_SYNTAX: [char; 13] = [
    ';', '&', '|', '<', '>', '`', '$', '(', ')', '{', '}', '\\', '\n',
];

pub(super) fn options() -> Vec<RequestOption> {
    let option = |id: &str, label: &str, decision| RequestOption {
        id: id.into(),
        label: label.into(),
        decision,
        scope: None,
    };
    vec![
        option("accept", "Approve once", Decision::Accept),
        option("decline", "Decline", Decision::Decline),
    ]
}

/// Every path a call names: its `locations` (which agy resolves through
/// symlinks), its diffs' paths and its input's path fields.
fn paths_of(call: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |p: Option<&str>| {
        if let Some(p) = p.filter(|p| !p.trim().is_empty()) {
            if !out.iter().any(|o| o == p) {
                out.push(p.to_string());
            }
        }
    };
    for l in call["locations"].as_array().into_iter().flatten() {
        push(l["path"].as_str());
    }
    for c in call["content"].as_array().into_iter().flatten() {
        push(c["path"].as_str());
    }
    for input in [&call["rawInput"], &call["args"]] {
        for key in PATH_KEYS {
            push(input[key].as_str());
        }
    }
    out
}

/// A write's changes in the shape a file change's row reads
/// (`changes[{path, kind, diff}]`).
fn changes(call: &Value, paths: &[String], delete: bool) -> Value {
    let diffs: Vec<Value> = call["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["type"] == "diff" && c["path"].is_string())
        .map(|c| {
            let lines = |key: &str, mark: char| {
                c[key]
                    .as_str()
                    .unwrap_or("")
                    .lines()
                    .map(|l| format!("{mark}{l}\n"))
                    .collect::<String>()
            };
            let kind = match (delete, c["oldText"].is_string()) {
                (true, _) => "delete",
                (false, true) => "update",
                (false, false) => "add",
            };
            json!({
                "path": c["path"],
                "kind": kind,
                "diff": format!("{}{}", lines("oldText", '-'), lines("newText", '+')),
            })
        })
        .collect();
    if !diffs.is_empty() {
        return Value::Array(diffs);
    }
    let kind = if delete { "delete" } else { "update" };
    paths
        .iter()
        .map(|p| json!({ "path": p, "kind": kind, "diff": "" }))
        .collect()
}

/// `paths` made absolute. agy runs its commands from the library root, not
/// the chat's folder, so a relative path is read against both, and each
/// reading must pass: the stricter answer stands.
fn absolute_paths(paths: &[String], library: &Path, cwd: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for p in paths {
        match Path::new(p).is_absolute() {
            true => out.push(p.clone()),
            false => {
                for base in [cwd, library] {
                    out.push(base.join(p).display().to_string());
                }
            }
        }
    }
    out
}

/// Whether `line` is one plain `oculus …` or `ls …`.
pub(super) fn allowed_command(line: &str) -> bool {
    let line = line.trim();
    if line.contains(SHELL_SYNTAX) {
        return false;
    }
    line.split_whitespace()
        .next()
        .is_some_and(|first| ALLOWED_COMMANDS.contains(&first))
}

/// The database and its sidecars (`oculus.db-wal`, `-shm`, a journal): an
/// agent writes them only through `oculus`.
fn database(library: &Path, cwd: &Path, paths: &[String]) -> Option<String> {
    let library = policy::resolve(library);
    paths
        .iter()
        .find(|p| {
            let full = absolute(cwd, p);
            let full = policy::resolve(&full);
            full.parent() == Some(library.as_path())
                && full
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().to_lowercase().starts_with("oculus.db"))
        })
        .cloned()
}

fn absolute(cwd: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    match path.is_absolute() {
        true => path.to_path_buf(),
        false => cwd.join(path),
    }
}

/// Whether `path` is inside one of `roots`, compared with case: a folder
/// spelt differently is asked about, not let through.
fn within(roots: &[PathBuf], cwd: &Path, path: &str) -> bool {
    let path = policy::resolve(&absolute(cwd, path));
    roots.iter().any(|root| {
        let root = policy::resolve(root);
        path != root && path.starts_with(&root)
    })
}

/// Rust's answer to one `session/request_permission`'s params, for a chat
/// whose library is `library` and whose folder (the write root) is `cwd`.
pub(super) fn judge(params: &Value, library: &Path, cwd: &Path) -> Verdict {
    let call = &params["toolCall"];
    let name = s(call, "name");
    let kind = s(call, "kind");
    let input = match &call["rawInput"] {
        Value::Null => call["args"].clone(),
        raw => raw.clone(),
    };
    let id = s(call, "toolCallId");
    let ask = |action: Action, mut payload: Value, paths: Vec<String>| {
        payload["tool"] = json!(id);
        Verdict::Ask(NewRequest {
            action,
            payload,
            options: options(),
            native_ref: String::new(),
            paths,
        })
    };
    let writes =
        matches!(kind.as_str(), "edit" | "delete" | "move") || WRITE_TOOLS.contains(&name.as_str());
    if writes {
        let paths = absolute_paths(&paths_of(call), library, cwd);
        if paths.is_empty() {
            return Verdict::Deny("a write that names no path cannot be checked".into());
        }
        if let Some(p) = policy::protected(library, cwd, &paths) {
            return Verdict::Deny(format!("it writes the protected path {p}"));
        }
        if let Some(p) = database(library, cwd, &paths) {
            return Verdict::Deny(format!("it writes the database ({p})"));
        }
        let mut roots = vec![cwd.to_path_buf()];
        roots.extend(crate::library::paths::agent_write_dirs(library));
        if paths
            .iter()
            .all(|p| within(&roots, cwd, p) || policy::in_own_zone(library, cwd, p))
        {
            return Verdict::Allow;
        }
        let payload = json!({
            "changes": changes(call, &paths, kind == "delete"),
            "reason": Value::Null,
        });
        return ask(Action::FileChange, payload, paths);
    }
    match (kind.as_str(), name.as_str()) {
        ("execute", _) | (_, "run_command") => {
            let line = ["CommandLine", "Command"]
                .iter()
                .find_map(|k| input[*k].as_str())
                .unwrap_or("")
                .to_string();
            let bypass = input["BypassSandbox"] == true;
            // agy's sandbox refuses database writes even through our CLI.
            // OCULUS_AGENT guards oculus itself, including a bypassed retry.
            let oculus = line.split_whitespace().next() == Some("oculus");
            if allowed_command(&line) && (!bypass || oculus) {
                return Verdict::Allow;
            }
            let payload = json!({
                "command": line,
                "cwd": input.get("Cwd").cloned().unwrap_or(Value::Null),
                "reason": input.get("toolSummary").cloned().unwrap_or(Value::Null),
                "outsideSandbox": bypass,
            });
            ask(Action::Command, payload, Vec::new())
        }
        ("read" | "search", _) => {
            let path = paths_of(call).into_iter().next();
            let pattern = ["Pattern", "Query", "SearchTerm"]
                .iter()
                .find_map(|k| input[*k].as_str());
            let payload = json!({
                "name": name, "kind": "read", "path": path, "pattern": pattern,
            });
            ask(Action::Tool, payload, Vec::new())
        }
        ("fetch", _) => {
            let url = ["Url", "URL", "url"]
                .iter()
                .find_map(|k| input[*k].as_str());
            let payload =
                json!({ "name": name, "kind": "fetch", "url": url, "prompt": Value::Null });
            ask(Action::Tool, payload, Vec::new())
        }
        _ => {
            let payload = json!({ "name": name, "kind": "other", "arguments": input });
            ask(Action::Tool, payload, Vec::new())
        }
    }
}
