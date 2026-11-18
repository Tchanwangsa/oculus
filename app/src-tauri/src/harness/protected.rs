//! What no agent may write, whichever CLI runs it. Each bridge renders these
//! in its own mechanism: Claude (`providers/claude/settings.rs`) as `Edit`
//! denies its sandbox also takes; Codex (`providers/codex/mod.rs`) as the
//! writable roots it is given; opencode's copy is static in
//! `templates/OPENCODE.template.json`, held to these by the test below; Rust
//! refuses them at every request (`requests/policy.rs`), which is
//! Antigravity's only gate.
//!
//! The library's own zone — everything in a subject beside `Course/` and
//! `.oculus/` (`layout::Zone::Own`) — is writable without a request.

use std::path::{Path, PathBuf};

use crate::library::layout;

/// Data-dir folders an agent reads but never writes, whole: a recordings
/// folder outside the subjects (none in the Drive tree) and an earlier
/// version's session folder. `courses/` is not one: its subjects' own files
/// are writable.
pub fn library_dirs() -> Vec<&'static str> {
    layout::library_roots()
        .into_iter()
        .filter(|d| *d != layout::COURSES_DIR)
        .chain(["canvas-session"])
        .collect()
}

/// The data-dir folder holding the subjects: writable, but for each
/// subject's `SUBJECT_DIRS` and anything directly inside it that is not in a
/// subject.
pub fn subjects_dir(library: &Path) -> PathBuf {
    layout::courses_dir(library)
}

/// Inside every subject, the folders no agent writes: `Course/` (what the
/// sync owns) and `.oculus/` (app state).
pub fn subject_dirs() -> Vec<&'static str> {
    layout::reserved_names()
}

/// `subject_dirs` as data-dir-relative rule patterns, `*` standing for any
/// one subject: `courses/*/Course`.
pub fn subject_dir_globs() -> Vec<String> {
    subject_dirs()
        .into_iter()
        .map(|d| format!("{}/*/{d}", layout::COURSES_DIR))
        .collect()
}

/// Whether a data-dir-relative path is in the own zone, which an agent
/// writes without asking.
pub fn own_zone(rel: &str) -> bool {
    layout::zone_of(rel) == layout::Zone::Own
}

/// App-owned folders inside the writable `agents/` folder: generated skills
/// and the links scanning CLIs find them through (`crate::agents`).
pub const WORKSPACE_DIRS: [&str; 3] = ["skills", ".claude", ".agents"];

/// Library-root files, by pattern. Never `oculus.db*`: see
/// `providers/claude/settings.rs`.
///
/// The sessions live in keyd's sealed vault, so the first two cover only the
/// files an earlier version kept, until keyd imports them. The last two are
/// the vault, its lock, and the temp file each write goes through: encrypted,
/// but an agent that deleted or truncated them would sign the student out of
/// everything.
pub const ROOT_FILE_GLOBS: [&str; 6] = [
    "*.cookie",
    "*.token",
    "*.json",
    "*.log",
    "vault.bin*",
    ".vault.bin.*",
];

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
    use std::path::Path;

    #[test]
    fn the_vault_and_the_temp_file_each_write_goes_through_are_covered() {
        use keyd_core::paths;
        let lib = Path::new("/l");
        let vault = paths::vault(lib);
        for file in [vault.clone(), paths::vault_lock(&vault)] {
            let path = crate::library::paths::slash_path(&file);
            assert!(
                ROOT_FILE_GLOBS
                    .iter()
                    .any(|g| glob_covers(&format!("/l/{g}"), &path)),
                "no root glob covers {path}"
            );
        }
        // The temp file a vault write goes through (`vault/store.rs`).
        assert!(ROOT_FILE_GLOBS
            .iter()
            .any(|g| glob_covers(&format!("/l/{g}"), "/l/.vault.bin.4242.a1b2c3d4e5f6.tmp")));
    }

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

        let mut want: Vec<String> = library_dirs()
            .iter()
            .map(|d| format!("{{{{LIBRARY}}}}/{d}/**"))
            .collect();
        want.extend(
            ROOT_FILE_GLOBS
                .iter()
                .map(|g| format!("{{{{LIBRARY}}}}/{g}")),
        );
        for glob in subject_dir_globs() {
            want.push(format!("{{{{LIBRARY}}}}/{glob}"));
            want.push(format!("{{{{LIBRARY}}}}/{glob}/*"));
        }
        // Anything in `courses/` outside a subject's own zone.
        want.push(format!("{{{{LIBRARY}}}}/{}/**", layout::COURSES_DIR));
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

    #[test]
    fn the_own_zone_is_everything_in_a_subject_but_course_and_oculus() {
        for rel in [
            "courses/X/notes.md",
            "courses/X/Exam prep/deep/scan.pdf",
            "courses/Y/board.whiteboard",
            "courses/X/.assets/p.png",
        ] {
            assert!(own_zone(rel), "{rel}");
        }
        for rel in [
            "courses",
            "courses/X",
            "courses/X/Course/Files/a.pdf",
            "courses/X/course/home.md",
            "courses/X/.oculus/agents/INSTRUCTIONS.md",
            "agents/notes.md",
            "oculus.db",
            "courses/X/../../oculus.db",
        ] {
            assert!(!own_zone(rel), "{rel}");
        }
        assert_eq!(
            subject_dir_globs(),
            ["courses/*/Course", "courses/*/.oculus"]
        );
        assert!(
            !library_dirs().contains(&layout::COURSES_DIR),
            "the subjects' folder is not protected whole"
        );
    }
}
