//! Tauri injects its IPC bridge into every child webview, remote pages
//! included, so only the capability ACL keeps a dock page from the app's
//! commands. This fails the build on any capability that could reach one:
//! a `remote` section, a `webviews` entry that could match `dock-*`, or a
//! `windows` entry that could match `main` (window and webview scopes match
//! by OR, and dock pages are child webviews of `main`).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::label;
use super::services::SERVICES;
use crate::shell::browser::MAIN;

const GLOB_CHARS: [char; 4] = ['*', '?', '[', '{'];

/// Tauri's scope patterns are globs. A `[...]` class is read as any one
/// character and `{a,b}` as anything, which can only over-match.
fn glob_matches(pattern: &str, text: &str) -> bool {
    fn go(p: &[char], t: &[char]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some('*') => (0..=t.len()).any(|i| go(&p[1..], &t[i..])),
            Some('{') => true,
            Some('?') => !t.is_empty() && go(&p[1..], &t[1..]),
            Some('[') => {
                let close = p.iter().position(|c| *c == ']').unwrap_or(p.len() - 1);
                !t.is_empty() && go(&p[close + 1..], &t[1..])
            }
            Some(c) => t.first() == Some(c) && go(&p[1..], &t[1..]),
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    go(&p, &t)
}

/// True when `entry` could name a dock page: it matches a real label, it is
/// itself a `dock-` label, or it is a glob whose literal prefix is compatible
/// with `dock-` (`*`, `d*`, `dock-w?`, …).
fn could_match_dock(entry: &str) -> bool {
    if SERVICES.iter().any(|s| glob_matches(entry, &label(s.id))) || entry.starts_with("dock-") {
        return true;
    }
    match entry.find(GLOB_CHARS) {
        Some(at) => {
            let prefix = &entry[..at];
            "dock-".starts_with(prefix) || prefix.starts_with("dock-")
        }
        None => false,
    }
}

fn strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| v.to_string())
            })
            .collect(),
        _ => vec![],
    }
}

/// Every reason `capability` could hand a dock page an app command.
fn violations(capability: &Value) -> Vec<String> {
    let mut found = vec![];
    if capability.get("remote").is_some() {
        found.push("has a `remote` section".to_string());
    }
    for entry in strings(capability.get("webviews")) {
        if could_match_dock(&entry) {
            found.push(format!("webviews entry {entry:?} can match a dock page"));
        }
    }
    for entry in strings(capability.get("windows")) {
        if glob_matches(&entry, MAIN) {
            found.push(format!(
                "windows entry {entry:?} matches `main`, which holds the dock pages"
            ));
        }
    }
    found
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn capability_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read capabilities/")
        .map(|e| e.expect("capabilities/ entry").path())
        .collect();
    files.sort();
    files
}

#[test]
fn no_capability_reaches_a_dock_page() {
    let files = capability_files(&manifest_dir().join("capabilities"));
    assert!(!files.is_empty(), "no capability files found");
    let mut failures = vec![];
    for path in files {
        let name = path.display().to_string();
        // Tauri also reads .json5 and .toml here; this test only parses JSON,
        // so anything else would go unchecked.
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            failures.push(format!(
                "{name}: not a .json file, so this test cannot check it"
            ));
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read capability");
        let capability: Value = serde_json::from_str(&text).expect("capability is JSON");
        for reason in violations(&capability) {
            failures.push(format!("{name}: {reason}"));
        }
    }
    // Capabilities can also be declared inline in tauri.conf.json.
    let conf: Value = serde_json::from_str(
        &std::fs::read_to_string(manifest_dir().join("tauri.conf.json")).expect("tauri.conf.json"),
    )
    .expect("tauri.conf.json is JSON");
    if let Some(Value::Array(inline)) = conf.pointer("/app/security/capabilities") {
        for capability in inline.iter().filter(|c| c.is_object()) {
            for reason in violations(capability) {
                failures.push(format!("tauri.conf.json inline capability: {reason}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_checks_catch_what_they_should() {
    use serde_json::json;

    for bad in [
        json!({ "remote": { "urls": ["https://web.whatsapp.com"] }, "webviews": ["main"] }),
        json!({ "webviews": ["dock-*"] }),
        json!({ "webviews": ["dock-whatsapp"] }),
        json!({ "webviews": ["*"] }),
        json!({ "webviews": ["d*"] }),
        json!({ "webviews": ["*-discord"] }),
        json!({ "webviews": ["dock-[a-z]*"] }),
        json!({ "windows": ["main"] }),
        json!({ "windows": ["*"] }),
        json!({ "windows": ["ma?n"] }),
        json!({ "windows": ["m*"] }),
    ] {
        assert!(!violations(&bad).is_empty(), "missed {bad}");
    }
    for good in [
        json!({ "webviews": ["main"] }),
        json!({ "windows": ["canvas-auth", "canvas-worker"] }),
        json!({ "webviews": ["settings", "canvas-*"] }),
    ] {
        assert!(violations(&good).is_empty(), "flagged {good}");
    }
}
