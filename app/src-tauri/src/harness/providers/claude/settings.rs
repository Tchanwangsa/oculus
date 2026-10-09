//! The inline `--settings` document that contains a Claude thread.

use std::path::Path;

use crate::harness::protected::{LIBRARY_DIRS, ROOT_FILE_GLOBS, WORKSPACE_DIRS};

/// The inline `--settings` document; docs/harness.md explains each part.
/// `//` prefixes an absolute path in the CLI's rule syntax. Root files are
/// denied by suffix, because a bare `*` also denied everything under `agents/`.
pub(super) fn settings_json(library: &Path, cwd: &Path, oculus: Option<&Path>) -> String {
    let root = library.display().to_string();
    let abs = root.trim_start_matches('/');
    // Never `oculus.db*`: the CLI merges `Edit(...)` denies into the
    // sandbox's `denyWrite`, which would cancel `allowWrite` below.
    let mut deny: Vec<String> = LIBRARY_DIRS
        .iter()
        .map(|d| format!("{d}/**"))
        .chain(ROOT_FILE_GLOBS.iter().map(|g| g.to_string()))
        .chain(WORKSPACE_DIRS.iter().map(|d| format!("agents/{d}/**")))
        .map(|p| format!("Edit(//{abs}/{p})"))
        .collect();
    // The database is OS-writable, so the CLI must stay the only door to it.
    deny.push("Bash(sqlite3:*)".to_string());

    // `autoAllowBashIfSandboxed` misses commands its analyser cannot vouch
    // for (multi-line, loops), and the resulting denial sticks for the session.
    let mut allow = vec!["Bash(oculus:*)".to_string()];
    // The rule matches the command name, so the full path needs its own.
    if let Some(cli) = oculus {
        allow.push(format!("Bash({}:*)", cli.display()));
    }

    // `agents/` plus the database's three files (`paths::db_write_paths`).
    let write: Vec<String> = std::iter::once(cwd.display().to_string())
        .chain(
            crate::library::paths::db_write_paths(library)
                .iter()
                .map(|p| p.display().to_string()),
        )
        .collect();
    serde_json::json!({
        "permissions": { "allow": allow, "deny": deny },
        "sandbox": {
            "enabled": true,
            "failIfUnavailable": false,
            "autoAllowBashIfSandboxed": true,
            "allowUnsandboxedCommands": false,
            "network": { "allowLocalBinding": true },
            "filesystem": { "allowWrite": write },
        },
        "autoMemoryEnabled": false,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_document_opens_the_database_and_nothing_else() {
        let library = Path::new("/Users/x/Library/Application Support/com.tchan.oculus");
        let cwd = library.join("agents");
        let v: serde_json::Value =
            serde_json::from_str(&settings_json(library, &cwd, None)).expect("valid settings JSON");

        let write = v.pointer("/sandbox/filesystem/allowWrite").unwrap();
        assert_eq!(
            write,
            &serde_json::json!([
                "/Users/x/Library/Application Support/com.tchan.oculus/agents",
                "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db",
                "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db-wal",
                "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db-shm",
            ]),
            "the cwd, then the database's three files — nothing else in the library"
        );
        assert_eq!(v["sandbox"]["enabled"], true);
        assert_eq!(v["autoMemoryEnabled"], false);

        let deny: Vec<&str> = v
            .pointer("/permissions/deny")
            .and_then(|d| d.as_array())
            .unwrap()
            .iter()
            .filter_map(|r| r.as_str())
            .collect();
        for rule in [
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/courses/**)",
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/agents/skills/**)",
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/agents/.claude/**)",
            "Bash(sqlite3:*)",
        ] {
            assert!(deny.contains(&rule), "missing {rule} in {deny:?}");
        }

        // An `Edit(...)` deny merges into `denyWrite` and cancels `allowWrite`.
        assert!(
            !deny.iter().any(|r| r.contains("oculus.db")),
            "oculus.db must stay out of deny — it cancels allowWrite: {deny:?}"
        );

        let allow: Vec<&str> = v
            .pointer("/permissions/allow")
            .and_then(|a| a.as_array())
            .unwrap()
            .iter()
            .filter_map(|r| r.as_str())
            .collect();
        assert_eq!(
            allow,
            ["Bash(oculus:*)"],
            "the board's door, and nothing else, is allowed by name"
        );

        let v2: serde_json::Value = serde_json::from_str(&settings_json(
            library,
            &cwd,
            Some(Path::new("/opt/oculus/bin/oculus")),
        ))
        .expect("valid settings JSON");
        let allow2: Vec<&str> = v2
            .pointer("/permissions/allow")
            .and_then(|a| a.as_array())
            .unwrap()
            .iter()
            .filter_map(|r| r.as_str())
            .collect();
        assert_eq!(allow2, ["Bash(oculus:*)", "Bash(/opt/oculus/bin/oculus:*)"]);
    }
}
