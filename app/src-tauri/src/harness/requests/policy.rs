//! What Rust checks before a request reaches the student. A request that
//! names paths (a file change, extra file access) is refused when one of
//! them is protected (`protected.rs`): the student never sees it, and the
//! CLI is told no. A subject's own files are not protected; a CLI whose
//! sandbox could not grant them asks, and the student decides. A shell
//! command cannot be checked this way, since nothing can tell from a
//! command line what it will touch; its row says so instead, and the name
//! denies and `OCULUS_AGENT` keep refusing inside an approved command.

use std::path::{Component, Path, PathBuf};

use crate::harness::protected::{library_dirs, own_zone, ROOT_FILE_GLOBS, WORKSPACE_DIRS};
use crate::library::layout::COURSES_DIR;

/// The first of `paths` that no agent may write in the library at
/// `library`. Relative paths are read against `cwd`, the agent's folder.
pub fn protected(library: &Path, cwd: &Path, paths: &[String]) -> Option<String> {
    let library = resolve(library);
    paths
        .iter()
        .find(|p| covers(&library, &resolve(&absolute(cwd, p))))
        .cloned()
}

/// Whether `path` (absolute, or relative to `cwd`) is in the library's own
/// zone, which an agent writes without asking.
pub fn in_own_zone(library: &Path, cwd: &Path, path: &str) -> bool {
    let library = resolve(library);
    library_rel(&library, &resolve(&absolute(cwd, path))).is_some_and(|rel| own_zone(&rel))
}

fn absolute(cwd: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    match path.is_absolute() {
        true => path.to_path_buf(),
        false => cwd.join(path),
    }
}

/// `path` relative to `library`, `/`-separated in its own spelling, when it
/// is inside it. The prefix is compared without case: the student's disk
/// ignores it by default, so `Courses/` is `courses/`, existing or not.
fn library_rel(library: &Path, path: &Path) -> Option<String> {
    let lib: Vec<String> = parts(library);
    let all: Vec<String> = parts(path);
    let inside = all.len() >= lib.len()
        && lib
            .iter()
            .zip(&all)
            .all(|(a, b)| a.to_lowercase() == b.to_lowercase());
    inside.then(|| all[lib.len()..].join("/"))
}

fn parts(path: &Path) -> Vec<String> {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect()
}

fn covers(library: &Path, path: &Path) -> bool {
    // A folder that holds the library: a grant on it reaches every
    // protected folder inside.
    if library_rel(path, library).is_some() {
        return true;
    }
    let Some(rel) = library_rel(library, path) else {
        return false;
    };
    let lower = rel.to_lowercase();
    let parts: Vec<&str> = lower.split('/').filter(|p| !p.is_empty()).collect();
    match parts.as_slice() {
        // The library itself: a grant on it reaches every protected folder.
        [] => true,
        [first, ..] if library_dirs().contains(first) => true,
        // A subject's own files are writable; `Course/`, `.oculus/`, the
        // subjects' folder itself and a subject's folder are not.
        [first, ..] if *first == COURSES_DIR => !own_zone(&rel),
        [agents, dir, ..] if *agents == "agents" && WORKSPACE_DIRS.contains(dir) => true,
        // An `agents/` grant reaches the workspace folders inside it.
        [agents] if *agents == "agents" => true,
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
pub(in crate::harness) fn resolve(path: &Path) -> PathBuf {
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
            "/lib/oculus/courses",
            "/lib/oculus/courses/maths",
            "/lib/oculus/courses/loose.pdf",
            "/lib/oculus/courses/maths/Course/Files/a.pdf",
            "/lib/oculus/courses/maths/Course",
            "/lib/oculus/courses/maths/Course/Recordings/x/transcript.vtt",
            "/lib/oculus/courses/maths/.oculus/agents/INSTRUCTIONS.md",
            "/lib/oculus/courses/maths/.oculus",
            "/lib/oculus/canvas-session/x",
            "/lib/oculus/vault.bin",
            "/lib/oculus/.vault.bin.tmp",
            "/lib/oculus/settings.json",
            "/lib/oculus",
            "/lib/oculus/agents",
            "/lib/oculus/agents/skills/a/SKILL.md",
            "/lib/oculus/agents/.claude/settings.json",
            "../courses/maths/Course/home.md",
            "skills/x",
            "/lib/oculus/agents/../courses/y/Course/z",
            "/lib",
            "/",
            "/lib/oculus/Courses/x/COURSE/home.md",
            "/lib/oculus/courses/x/.OCULUS/derived/a.pages.json",
            "/LIB/Oculus/COURSES",
            "/lib/oculus/Settings.JSON",
            "/lib/oculus/agents/Skills/x",
        ] {
            assert!(check(path), "{path} is protected");
        }
        for path in [
            "/lib/oculus/courses/maths/notes.md",
            "/lib/oculus/courses/maths/Exam prep/deep/scan.pdf",
            "/lib/oculus/courses/other/summary.md",
            "/lib/oculus/Courses/maths/Notes/a.md",
            "/lib/oculus/courses/maths/settings.json",
            "../courses/maths/notes.md",
            "/lib/oculus/agents/../courses/maths/a.md",
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
    fn the_own_zone_is_read_against_the_chats_folder() {
        let library = Path::new("/lib/oculus");
        let cwd = library.join("agents");
        for path in [
            "../courses/maths/notes.md",
            "/lib/oculus/courses/maths/Exam prep/a.md",
            "/LIB/oculus/Courses/maths/a.md",
        ] {
            assert!(in_own_zone(library, &cwd, path), "{path}");
        }
        for path in [
            "notes.md",
            "../courses/maths/Course/home.md",
            "../courses/maths/.oculus/x",
            "../courses/maths",
            "/elsewhere/courses/maths/a.md",
        ] {
            assert!(!in_own_zone(library, &cwd, path), "{path}");
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
