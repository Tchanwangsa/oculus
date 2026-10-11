//! The inline `--settings` document that contains a Claude thread.

use std::path::Path;

use crate::harness::protected::{LIBRARY_DIRS, ROOT_FILE_GLOBS, WORKSPACE_DIRS};

/// The inline `--settings` document; docs/harness.md explains each part.
/// `//` prefixes an absolute path in the CLI's rule syntax. Root files are
/// denied by suffix, because a bare `*` also denied everything under `agents/`.
/// `hosts` (`host:port`) are the network destinations a sandboxed command
/// may reach: the TTS server's, or none.
pub(super) fn settings_json(
    library: &Path,
    cwd: &Path,
    oculus: Option<&Path>,
    hosts: &[String],
) -> String {
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

    // `agents/`, the database's three files (`paths::db_write_paths`) and
    // `studio/` (`paths::agent_write_dirs`).
    let write: Vec<String> = std::iter::once(cwd.to_path_buf())
        .chain(crate::library::paths::db_write_paths(library))
        .chain(crate::library::paths::agent_write_dirs(library))
        .map(|p| p.display().to_string())
        .collect();
    // `oculus` reaches its keys only through keyd's socket; without this the
    // sandbox refuses the connect (EPERM).
    let keyd = crate::library::paths::keyd_socket_path(library)
        .display()
        .to_string();
    let mut network = serde_json::json!({ "allowLocalBinding": true, "allowUnixSockets": [keyd] });
    if !hosts.is_empty() {
        network["allowedDomains"] = serde_json::json!(hosts);
    }
    serde_json::json!({
        "permissions": { "allow": allow, "deny": deny },
        "sandbox": {
            "enabled": true,
            "failIfUnavailable": false,
            "autoAllowBashIfSandboxed": true,
            "allowUnsandboxedCommands": false,
            "network": network,
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
    fn the_settings_document_opens_the_database_and_studio_and_nothing_else() {
        let library = Path::new("/Users/x/Library/Application Support/com.tchan.oculus");
        let cwd = library.join("agents");
        let v: serde_json::Value = serde_json::from_str(&settings_json(library, &cwd, None, &[]))
            .expect("valid settings JSON");

        let write = v.pointer("/sandbox/filesystem/allowWrite").unwrap();
        assert_eq!(
            write,
            &serde_json::json!([
                "/Users/x/Library/Application Support/com.tchan.oculus/agents",
                "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db",
                "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db-wal",
                "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db-shm",
                "/Users/x/Library/Application Support/com.tchan.oculus/studio",
            ]),
            "the cwd, the database's three files and studio/ — nothing else in the library"
        );
        assert!(
            v.pointer("/sandbox/network/allowedDomains").is_none(),
            "no TTS server, no network host"
        );
        assert_eq!(v["sandbox"]["enabled"], true);
        assert_eq!(v["autoMemoryEnabled"], false);
        assert_eq!(
            v.pointer("/sandbox/network/allowUnixSockets").unwrap(),
            &serde_json::json!(["/Users/x/Library/Application Support/com.tchan.oculus/keyd.sock"]),
            "keyd's socket, and no other"
        );
        assert_eq!(
            v.pointer("/sandbox/network/allowUnixSockets/0").unwrap(),
            keyd_core::paths::socket(library).to_str().unwrap(),
            "the path keyd's endpoint is bound at, not a copy of it"
        );
        assert!(
            v.pointer("/sandbox/network/allowAllUnixSockets").is_none(),
            "one socket is granted, not all of them"
        );

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
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/*.cookie)",
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/vault.bin*)",
            "Edit(//Users/x/Library/Application Support/com.tchan.oculus/.vault.bin.*)",
            "Bash(sqlite3:*)",
        ] {
            assert!(deny.contains(&rule), "missing {rule} in {deny:?}");
        }
        let writable = v
            .pointer("/sandbox/filesystem/allowWrite")
            .unwrap()
            .to_string();
        assert!(
            !writable.contains("vault"),
            "the vault is never writable: {writable}"
        );

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
            &[],
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

    /// Every `agent_write_dirs` folder is writable, and no `Edit` deny (which
    /// the CLI merges into `denyWrite`) covers a file a video is made of.
    #[test]
    fn studio_is_writable_and_no_deny_covers_it() {
        let library = Path::new("/Users/x/Library/Application Support/com.tchan.oculus");
        let v: serde_json::Value =
            serde_json::from_str(&settings_json(library, &library.join("agents"), None, &[]))
                .unwrap();
        let write = v.pointer("/sandbox/filesystem/allowWrite").unwrap();
        for dir in crate::library::paths::agent_write_dirs(library) {
            let dir = dir.to_str().unwrap();
            assert!(
                write.as_array().unwrap().iter().any(|w| w == dir),
                "{dir} in {write}"
            );
            for file in [
                "videos/a/video.json",
                "videos/a/script.md",
                "library/components/Steps.tsx",
                "subjects/S/themes/warm.json",
                "library/.history/Steps.1.tsx",
            ] {
                let path = format!("{dir}/{file}");
                for rule in v
                    .pointer("/permissions/deny")
                    .and_then(|d| d.as_array())
                    .unwrap()
                    .iter()
                    .filter_map(|r| r.as_str())
                    .filter_map(|r| r.strip_prefix("Edit(/").and_then(|r| r.strip_suffix(')')))
                {
                    assert!(
                        !crate::harness::protected::glob_covers(rule, &path),
                        "Edit deny {rule} covers {path}"
                    );
                }
            }
        }
    }

    /// The TTS host goes in as `allowedDomains`, exactly as given.
    #[test]
    fn the_tts_host_is_the_only_network_domain() {
        let library = Path::new("/l");
        let v: serde_json::Value = serde_json::from_str(&settings_json(
            library,
            &library.join("agents"),
            None,
            &["127.0.0.1:8880".to_owned()],
        ))
        .unwrap();
        assert_eq!(
            v.pointer("/sandbox/network/allowedDomains").unwrap(),
            &serde_json::json!(["127.0.0.1:8880"])
        );
    }

    /// The sandbox grant follows the library the thread runs over, and no
    /// `Edit` deny (merged into `denyWrite`) covers the socket it grants.
    #[test]
    fn the_socket_grant_follows_the_library_and_no_deny_covers_it() {
        for lib in [
            "/Users/x/Library/Application Support/com.tchan.oculus",
            "/d",
        ] {
            let library = Path::new(lib);
            let v: serde_json::Value =
                serde_json::from_str(&settings_json(library, &library.join("agents"), None, &[]))
                    .unwrap();
            let socket = keyd_core::paths::socket(library);
            assert_eq!(
                v.pointer("/sandbox/network/allowUnixSockets").unwrap(),
                &serde_json::json!([socket.to_str().unwrap()])
            );
            for rule in v
                .pointer("/permissions/deny")
                .and_then(|d| d.as_array())
                .unwrap()
                .iter()
                .filter_map(|r| r.as_str())
                .filter_map(|r| r.strip_prefix("Edit(/").and_then(|r| r.strip_suffix(')')))
            {
                assert!(
                    !crate::harness::protected::glob_covers(rule, socket.to_str().unwrap()),
                    "Edit deny {rule} covers {socket:?}"
                );
            }
        }
    }
}
