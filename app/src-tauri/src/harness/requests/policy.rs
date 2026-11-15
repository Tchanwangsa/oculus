//! What Rust checks before a request reaches the student. A request that
//! names paths (a file change, extra file access) is refused when one of
//! them is protected (`protected.rs`): the student never sees it, and the
//! CLI is told no. A shell command cannot be checked this way, since
//! nothing can tell from a command line what it will touch; its row says
//! so instead, and the name denies and `OCULUS_AGENT` keep refusing inside
//! an approved command.

use std::path::{Component, Path, PathBuf};

use crate::harness::protected::{LIBRARY_DIRS, ROOT_FILE_GLOBS, WORKSPACE_DIRS};

/// The first of `paths` that no agent may write in the library at
/// `library`. Relative paths are read against `cwd`, the agent's folder.
pub fn protected(library: &Path, cwd: &Path, paths: &[String]) -> Option<String> {
    let library = resolve(library);
    paths
        .iter()
        .find(|p| {
            let path = Path::new(p.as_str());
            let full = match path.is_absolute() {
                true => path.to_path_buf(),
                false => cwd.join(path),
            };
            covers(&library, &resolve(&full))
        })
        .cloned()
}

fn covers(library: &Path, path: &Path) -> bool {
    // A folder that holds the library: a grant on it reaches every
    // protected folder inside.
    if library.starts_with(path) {
        return true;
    }
    let Ok(rest) = path.strip_prefix(library) else {
        return false;
    };
    let parts: Vec<String> = rest
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    match parts.as_slice() {
        // The library itself: a grant on it reaches every protected folder.
        [] => true,
        [first, ..] if LIBRARY_DIRS.contains(&first.as_str()) => true,
        [agents, dir, ..] if agents == "agents" && WORKSPACE_DIRS.contains(&dir.as_str()) => true,
        // An `agents/` grant reaches the workspace folders inside it.
        [agents] if agents == "agents" => true,
        [file] => ROOT_FILE_GLOBS.iter().any(|glob| glob_matches(glob, file)),
        _ => false,
    }
}

/// `*` within one name, as `ROOT_FILE_GLOBS` uses it.
fn glob_matches(glob: &str, name: &str) -> bool {
    let pieces: Vec<&str> = glob.split('*').collect();
    let (first, last) = (pieces[0], pieces[pieces.len() - 1]);
    if pieces.len() == 1 {
        return name == glob;
    }
    if !name.starts_with(first) || !name[first.len()..].ends_with(last) {
        return false;
    }
    let mut rest = &name[first.len()..name.len() - last.len()];
    for piece in &pieces[1..pieces.len() - 1] {
        match rest.find(piece) {
            Some(at) => rest = &rest[at + piece.len()..],
            None => return false,
        }
    }
    true
}

/// `path` with `.` and `..` worked out, and its deepest existing folder
/// through its symlinks, so `/tmp` and `/private/tmp` compare equal.
fn resolve(path: &Path) -> PathBuf {
    let mut lexical = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                lexical.pop();
            }
            Component::CurDir => {}
            other => lexical.push(other.as_os_str()),
        }
    }
    let mut existing = lexical.clone();
    let mut tail = Vec::new();
    while !existing.exists() {
        match existing.file_name() {
            Some(name) => tail.push(name.to_os_string()),
            None => return lexical,
        }
        if !existing.pop() {
            return lexical;
        }
    }
    let mut out = existing.canonicalize().unwrap_or(existing);
    for name in tail.into_iter().rev() {
        out.push(name);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(path: &str) -> bool {
        let library = Path::new("/lib/oculus");
        let cwd = library.join("agents");
        protected(library, &cwd, &[path.to_string()]).is_some()
    }

    #[test]
    fn protected_folders_and_root_files_are_refused_and_the_rest_is_not() {
        for path in [
            "/lib/oculus/courses/maths/notes.md",
            "/lib/oculus/lectures",
            "/lib/oculus/canvas-session/x",
            "/lib/oculus/vault.bin",
            "/lib/oculus/.vault.bin.tmp",
            "/lib/oculus/settings.json",
            "/lib/oculus",
            "/lib/oculus/agents",
            "/lib/oculus/agents/skills/a/SKILL.md",
            "/lib/oculus/agents/.claude/settings.json",
            "../courses/x.md",
            "skills/x",
            "/lib/oculus/agents/../courses/y",
            "/lib",
            "/",
        ] {
            assert!(check(path), "{path} is protected");
        }
        for path in [
            "/lib/oculus/agents/notes.md",
            "/lib/oculus/agents/attachments/a.png",
            "/lib/oculus/studio/video/video.json",
            "/elsewhere/courses/x",
            "/lib/oculus/oculus.db",
            "scratch/out.txt",
        ] {
            assert!(!check(path), "{path} is not protected");
        }
    }

    #[test]
    fn a_glob_matches_within_one_name() {
        assert!(glob_matches("*.json", "a.json"));
        assert!(glob_matches(".vault.bin.*", ".vault.bin.123"));
        assert!(glob_matches("vault.bin*", "vault.bin"));
        assert!(!glob_matches("*.json", "a.jsonl"));
        assert!(!glob_matches("vault.bin*", "my.vault.bin"));
    }
}
