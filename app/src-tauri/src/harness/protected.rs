//! What no agent may write, whichever CLI runs it. Claude (`claude.rs`) and
//! Antigravity (`antigravity_rules.rs`) render these into their own rule
//! syntax; opencode's copy is static in `templates/OPENCODE.template.json`,
//! held to these by the test below.

/// Library folders an agent reads but never writes.
pub const LIBRARY_DIRS: [&str; 3] = ["courses", "lectures", "canvas-session"];

/// App-owned folders inside the writable `agents/` folder: generated skills
/// and the links scanning CLIs find them through (`crate::agents`).
pub const WORKSPACE_DIRS: [&str; 3] = ["skills", ".claude", ".agents"];

/// Library-root files, by suffix; agy's rules name the files instead. Never
/// `oculus.db*`: see `claude.rs`.
pub const ROOT_FILE_GLOBS: [&str; 4] = ["*.cookie", "*.token", "*.json", "*.log"];

/// Whether the rule pattern `glob` (`*` within a path segment, `**` across
/// segments) covers `path`: the check that no deny list reaches keyd's socket.
#[cfg(test)]
pub(super) fn glob_covers(glob: &str, path: &str) -> bool {
    let mut re = String::from("^");
    let mut chars = glob.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                re.push_str(".*");
            }
            '*' => re.push_str("[^/]*"),
            c => re.push_str(&regex::escape(&c.to_string())),
        }
    }
    re.push('$');
    regex::Regex::new(&re).unwrap().is_match(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glob_covers_by_segment() {
        assert!(glob_covers("/l/*.json", "/l/a.json"));
        assert!(!glob_covers("/l/*.json", "/l/sub/a.json"));
        assert!(glob_covers("/l/courses/**", "/l/courses/x/y"));
        assert!(!glob_covers("/l/*.cookie", "/l/keyd.sock"));
    }

    /// opencode's denies are relative to `agents/` unless they name the library.
    #[test]
    fn the_opencode_template_denies_the_same_paths() {
        let raw = include_str!("../../templates/OPENCODE.template.json");
        let v: serde_json::Value = serde_json::from_str(raw).expect("the template is JSON");
        let edit = v
            .pointer("/permission/edit")
            .and_then(|e| e.as_object())
            .unwrap();
        let denied: Vec<&str> = edit
            .iter()
            .filter(|(_, v)| v.as_str() == Some("deny"))
            .map(|(k, _)| k.as_str())
            .collect();

        let mut want: Vec<String> = LIBRARY_DIRS
            .iter()
            .map(|d| format!("{{{{LIBRARY}}}}/{d}/**"))
            .collect();
        want.extend(
            ROOT_FILE_GLOBS
                .iter()
                .map(|g| format!("{{{{LIBRARY}}}}/{g}")),
        );
        for w in &want {
            assert!(
                denied.contains(&w.as_str()),
                "the template does not deny {w}"
            );
        }
        for d in WORKSPACE_DIRS {
            assert!(
                denied.iter().any(|k| k.starts_with(&format!("{d}/"))),
                "the template denies nothing under agents/{d}"
            );
        }
        // opencode has no OS sandbox, so it alone may deny the database.
        for k in denied.iter().filter(|k| k.starts_with("{{LIBRARY}}/")) {
            assert!(
                want.iter().any(|w| w == k) || *k == "{{LIBRARY}}/oculus.db*",
                "the template denies {k}, which the Rust list does not"
            );
        }
    }
}
