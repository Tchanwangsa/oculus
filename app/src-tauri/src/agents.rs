//! The agent-facing docs that live in the library.
//!
//! `agents/` in the data directory holds one `AGENTS.md` for every subject,
//! symlinked into each course folder, plus the stubs a human authors. It is
//! written from two places — `oculus docs` fills the whole layer on demand,
//! and a sync links whatever subjects it just scraped — so the rules about
//! what may be overwritten live here rather than in either caller.
//!
//! It also holds the memory layer an agent writes back into: `TASTE.md` for
//! standing preferences and `memories/` for facts — `memories/` itself for
//! what holds across subjects, `memories/<CODE>/` for one subject. **Nothing
//! here reads that layer back.** A thread's brief names the two buckets and
//! the agent opens them with its own tools
//! (`instructions` in `crate::harness`); the app never folds them into a
//! prompt itself. It used to, for the BYOK chat that has since been deleted,
//! and a second copy of the same files in every system prompt is a cost with
//! no reader.
//!
//! **Both buckets live under the library's own `agents/`, and that is not a
//! filing preference — it is the only place an in-app thread may write.** The
//! subject bucket used to be `courses/<CODE>/agents/memories/`, which every
//! template told the agent to use and no sandbox would let it touch: Codex's
//! writable root is the thread's cwd and Claude's seatbelt and `Edit` denies
//! say the same, so a subject fact was an instruction the app itself had made
//! impossible to follow. [`link_dir`] now moves any of those files into the
//! library bucket and leaves a symlink where they were, so an agent that
//! remembers the old path still writes into the one store. Nothing here ever
//! writes a memory.

use std::path::{Path, PathBuf};

use crate::paths;

/// One `AGENTS.md` for every subject, so there is nothing per-course to keep
/// in sync. Anything genuinely per-subject goes in that folder's
/// `agents/INSTRUCTIONS.md`, which nothing here ever writes.
pub const AGENTS_DOC: &str = include_str!("../templates/AGENTS.template.md");
const OCULUS_DOC: &str = include_str!("../templates/OCULUS.template.md");
const TASTE_DOC: &str = include_str!("../templates/TASTE.template.md");

const MEMORY_INDEX_DOC: &str = include_str!("../templates/MEMORY.template.md");

pub const AGENTS_DOC_NAME: &str = "AGENTS.md";
pub const CLI_DOC_NAME: &str = "OCULUS-CLI.md";
const TASTE_DOC_NAME: &str = "TASTE.md";
/// The index beside the memories, in both buckets: a table of contents an
/// agent reads before opening the files it sits with.
const MEMORY_INDEX_NAME: &str = "MEMORY.md";
const MEMORIES_DIR: &str = "memories";

/// From a course folder to the one central copy. Relative so the library can
/// move without every link in it going dangling.
const AGENTS_DOC_REL: &str = "../../agents/AGENTS.md";

pub fn agents_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("agents")
}

/// What [`ensure_library_docs`] did, so a caller can report it.
#[derive(Default)]
pub struct LibraryDocs {
    /// Rewritten every time — a stale copy would describe a layout that no
    /// longer exists.
    pub generated: Vec<&'static str>,
    /// Stubs that were missing and have just been created.
    pub created: Vec<&'static str>,
}

/// Fill `agents/` with everything that does not need the CLI's own help tree.
///
/// Cheap and idempotent, so both `oculus docs` and every sync can call it
/// blind. `OCULUS-CLI.md` is not written here: rendering it needs clap's
/// command tree, which only the binary has — and a library that has never met
/// the binary has no use for its reference anyway.
pub fn ensure_library_docs(data_dir: &Path) -> Result<LibraryDocs, String> {
    let dir = agents_dir(data_dir);
    std::fs::create_dir_all(dir.join(MEMORIES_DIR))
        .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

    let mut docs = LibraryDocs::default();

    // Generated: overwritten every time. This is what makes "one universal
    // AGENTS.md" real rather than five copies quietly drifting apart.
    let path = dir.join(AGENTS_DOC_NAME);
    std::fs::write(&path, AGENTS_DOC).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    docs.generated.push(AGENTS_DOC_NAME);

    // Stubs: written once and then the user's. Overwriting these would throw
    // away the only thing in `agents/` a human — or an agent — actually
    // authored. The memory index is stubbed for the same reason `agents/` is
    // scaffolded at all: an empty directory does not tell anyone what goes in
    // it, and an index nobody created is an index nobody appends to.
    for (name, path, body) in [
        ("OCULUS.md", dir.join("OCULUS.md"), OCULUS_DOC),
        (TASTE_DOC_NAME, dir.join(TASTE_DOC_NAME), TASTE_DOC),
        (
            "memories/MEMORY.md",
            dir.join(MEMORIES_DIR).join(MEMORY_INDEX_NAME),
            MEMORY_INDEX_DOC,
        ),
    ] {
        if path.exists() {
            continue;
        }
        std::fs::write(&path, body).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        docs.created.push(name);
    }
    Ok(docs)
}

/// What linking one course folder did.
#[derive(Debug, PartialEq, Eq)]
pub enum Link {
    Linked,
    /// Already pointing at the right place — the common case on a re-run.
    Current,
    /// A real file was there; left alone rather than overwritten.
    Skipped,
    /// No such course folder. Nothing has been scraped into it, so there is
    /// nothing for an agent to read and no reason to invent the directory.
    Absent,
}

/// Scaffold and link one subject's course folder, addressed by Canvas code.
///
/// This is the sync-side entry point: a subject that appears mid-semester is
/// linked by the run that first scrapes it, instead of waiting for someone to
/// remember `oculus docs`.
pub fn link_course(data_dir: &Path, code: &str) -> Result<Link, String> {
    let dir = data_dir.join("courses").join(paths::safe_dir(code));
    if !dir.is_dir() {
        return Ok(Link::Absent);
    }
    link_dir(data_dir, &dir)
}

/// Point every existing course folder at the one `AGENTS.md`.
///
/// The sweep behind `oculus docs` — it covers folders no sync touched this
/// run, including any left behind by an earlier one.
pub fn link_all(data_dir: &Path) -> Result<LinkReport, String> {
    let courses = data_dir.join("courses");
    let mut report = LinkReport::default();
    let entries = match std::fs::read_dir(&courses) {
        Ok(e) => e,
        // No courses yet is not a failure: the central docs still exist,
        // and the next sync is what creates folders to link.
        Err(_) => return Ok(report),
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        match link_dir(data_dir, &dir)? {
            Link::Linked => report.linked.push(name),
            Link::Current => report.current += 1,
            Link::Skipped => report.skipped.push(name),
            Link::Absent => {}
        }
    }
    report.linked.sort();
    report.skipped.sort();
    Ok(report)
}

/// One subject's memory bucket, under the library's `agents/` — the only
/// folder a thread can write to.
pub fn subject_memories(data_dir: &Path, course_dir: &str) -> PathBuf {
    agents_dir(data_dir).join(MEMORIES_DIR).join(course_dir)
}

/// From `courses/<CODE>/agents/memories` back to that bucket. Relative for the
/// same reason `AGENTS_DOC_REL` is: the library has to stay movable.
const SUBJECT_MEMORIES_REL: &str = "../../../agents/memories";

/// Move a course folder's old `agents/memories/` into the library bucket and
/// leave a symlink pointing at it.
///
/// Two agents write this store — the in-app thread, which cannot reach a
/// course folder, and a Claude Code or Codex the student runs in one from a
/// terminal, which can — so the migration cannot simply relocate the files
/// and hope: the second agent has the old path in its own memory, and would
/// quietly rebuild a second store beside the first. The symlink is what makes
/// the old path keep working, and it resolves *into* `agents/`, so even a
/// sandboxed write through it lands inside the writable root.
///
/// Idempotent, since it runs on every sync: a link that already points at the
/// bucket is left alone, and a course folder with real files in it is emptied
/// once and then is a link like any other. Files that would collide are left
/// where they are rather than overwriting what is already filed — the same
/// rule the rest of this module follows about somebody's work.
fn adopt_course_memories(course_agents: &Path, bucket: &Path, name: &str) -> Result<(), String> {
    let old = course_agents.join(MEMORIES_DIR);
    match std::fs::symlink_metadata(&old) {
        // Already a link. Repoint it if it aims somewhere else; a link is
        // this module's, not the student's, so replacing it loses nothing.
        Ok(meta) if meta.file_type().is_symlink() => {
            let want = PathBuf::from(SUBJECT_MEMORIES_REL).join(name);
            if std::fs::read_link(&old).is_ok_and(|t| t == want) {
                return Ok(());
            }
            std::fs::remove_file(&old).map_err(|e| format!("cannot relink {name}/agents/memories: {e}"))?;
        }
        Ok(meta) if meta.is_dir() => {
            for entry in std::fs::read_dir(&old).into_iter().flatten().flatten() {
                let to = bucket.join(entry.file_name());
                if to.exists() {
                    continue;
                }
                std::fs::rename(entry.path(), &to)
                    .map_err(|e| format!("cannot move {name}/agents/memories/{:?}: {e}", entry.file_name()))?;
            }
            // Only when it came out empty: anything left is a file this could
            // not place, and a folder is better than a link that hides it.
            if std::fs::read_dir(&old).into_iter().flatten().flatten().next().is_some() {
                return Ok(());
            }
            std::fs::remove_dir(&old).map_err(|e| format!("cannot replace {name}/agents/memories: {e}"))?;
        }
        // A real file called `memories` is somebody's, and not ours to move.
        Ok(_) => return Ok(()),
        Err(_) => {}
    }
    let target = format!("{SUBJECT_MEMORIES_REL}/{name}");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &old)
        .map_err(|e| format!("cannot link {name}/agents/memories: {e}"))?;
    #[cfg(not(unix))]
    let _ = target;
    Ok(())
}

/// A relative symlink, so the whole library stays movable, and skipped rather
/// than clobbered when a real file is already sitting there.
fn link_dir(data_dir: &Path, dir: &Path) -> Result<Link, String> {
    let name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
    let bucket = subject_memories(data_dir, &name);
    std::fs::create_dir_all(&bucket)
        .map_err(|e| format!("cannot create agents/memories/{name}: {e}"))?;
    let course_agents = dir.join("agents");
    std::fs::create_dir_all(&course_agents)
        .map_err(|e| format!("cannot create {name}/agents: {e}"))?;
    adopt_course_memories(&course_agents, &bucket, &name)?;

    // Named for the subject, because the one thing this index has to make
    // obvious is which bucket it is — a subject memory filed globally, or the
    // reverse, is the mistake the two-bucket split exists to prevent. Written
    // after the migration above, so a real index that came across from a
    // course folder is not replaced by a stub.
    let index = bucket.join(MEMORY_INDEX_NAME);
    if !index.exists() {
        let body = format!(
            "# Memories — {name}\n\n\
             Facts about this subject alone. Anything true across subjects goes in the\n\
             library's own `agents/memories/`.\n\n\
             <!-- - [Title](file-name.md) — the hook, in a clause -->\n"
        );
        std::fs::write(&index, body)
            .map_err(|e| format!("cannot write agents/memories/{name}/{MEMORY_INDEX_NAME}: {e}"))?;
    }

    let link = dir.join(AGENTS_DOC_NAME);
    match std::fs::symlink_metadata(&link) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if std::fs::read_link(&link).is_ok_and(|t| t == PathBuf::from(AGENTS_DOC_REL)) {
                return Ok(Link::Current);
            }
            std::fs::remove_file(&link).map_err(|e| format!("cannot relink {name}: {e}"))?;
        }
        Ok(_) => return Ok(Link::Skipped),
        Err(_) => {}
    }
    symlink_or_copy(AGENTS_DOC_REL, &link, AGENTS_DOC)
        .map_err(|e| format!("cannot link {name}: {e}"))?;
    Ok(Link::Linked)
}

#[derive(Default)]
pub struct LinkReport {
    pub linked: Vec<String>,
    pub current: usize,
    pub skipped: Vec<String>,
}

/// A relative symlink where the platform has them, a plain copy where it does
/// not. Windows needs a privilege for symlinks that a CLI should not demand.
#[cfg(unix)]
fn symlink_or_copy(target: &str, link: &Path, _body: &str) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn symlink_or_copy(_target: &str, link: &Path, body: &str) -> std::io::Result<()> {
    std::fs::write(link, body)
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("oculus-agents-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    /// The one branch that must never regress: a hand-written AGENTS.md in a
    /// course folder is somebody's work, and relinking must not eat it.
    #[test]
    fn linking_replaces_stale_links_but_never_real_files() {
        let root = scratch("link");
        let courses = root.join("courses");
        for name in ["fresh", "stale", "handwritten"] {
            std::fs::create_dir_all(courses.join(name)).unwrap();
        }
        std::os::unix::fs::symlink("../../elsewhere.md", courses.join("stale/AGENTS.md")).unwrap();
        std::fs::write(courses.join("handwritten/AGENTS.md"), "mine").unwrap();

        let report = link_all(&root).unwrap();
        assert_eq!(report.linked, ["fresh", "stale"]);
        assert_eq!(report.skipped, ["handwritten"]);
        assert_eq!(
            std::fs::read_link(courses.join("stale/AGENTS.md")).unwrap(),
            PathBuf::from(AGENTS_DOC_REL)
        );
        assert_eq!(
            std::fs::read_to_string(courses.join("handwritten/AGENTS.md")).unwrap(),
            "mine"
        );
        // The bucket itself is under the library's `agents/` — the only
        // folder a thread can write to — and the course folder gets a link to
        // it, so the path an agent may already have in its memory still works.
        assert!(root.join("agents/memories/fresh").is_dir());
        assert!(std::fs::symlink_metadata(courses.join("fresh/agents/memories"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(courses.join("fresh/agents/memories").is_dir(), "the link resolves");
        // Named for the subject, so the bucket a memory lands in is unambiguous.
        let index = std::fs::read_to_string(root.join("agents/memories/fresh/MEMORY.md")).unwrap();
        assert!(index.starts_with("# Memories — fresh"));

        // Second run is a no-op, which is what lets cli:install call it blind.
        let again = link_all(&root).unwrap();
        assert!(again.linked.is_empty());
        assert_eq!(again.current, 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The memories a terminal agent already filed in a course folder — the
    /// path every template used to name — are moved into the library bucket
    /// rather than stranded behind a link that hides them.
    #[test]
    fn memories_filed_in_a_course_folder_are_adopted() {
        let root = scratch("adopt");
        let old = root.join("courses/INFO30006_2026_SM2/agents/memories");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("info30006-mst.md"), "the MST is week 7").unwrap();
        std::fs::write(old.join("MEMORY.md"), "# Memories — INFO30006\n\n- [MST](info30006-mst.md)").unwrap();

        link_all(&root).unwrap();

        let bucket = root.join("agents/memories/INFO30006_2026_SM2");
        assert_eq!(
            std::fs::read_to_string(bucket.join("info30006-mst.md")).unwrap(),
            "the MST is week 7"
        );
        // The course's own index came across, so the stub never overwrote it.
        assert!(std::fs::read_to_string(bucket.join("MEMORY.md")).unwrap().contains("[MST]"));
        // And the old path now resolves to the same file, for whoever still
        // writes there.
        assert!(old.join("info30006-mst.md").is_file());
        assert!(std::fs::symlink_metadata(&old).unwrap().file_type().is_symlink());

        // Idempotent: a second sync neither re-moves nor re-links.
        link_all(&root).unwrap();
        assert!(old.join("info30006-mst.md").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// What a sync does with a subject it has just scraped for the first time,
    /// and with one that produced no folder at all.
    #[test]
    fn a_sync_links_scraped_subjects_and_invents_no_folders() {
        let root = scratch("course");
        std::fs::create_dir_all(root.join("courses/COMP30026_2026_SM2")).unwrap();

        assert_eq!(link_course(&root, "COMP30026_2026_SM2").unwrap(), Link::Linked);
        assert_eq!(link_course(&root, "COMP30026_2026_SM2").unwrap(), Link::Current);

        // A subject whose scrape wrote nothing has nothing to annotate, and
        // must not leave an empty course folder behind as a side effect.
        assert_eq!(link_course(&root, "NEW10001_2026_SM2").unwrap(), Link::Absent);
        assert!(!root.join("courses/NEW10001_2026_SM2").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A link is only useful if it resolves, so a sync has to write the
    /// central copy before pointing anything at it.
    #[test]
    fn sync_side_links_are_never_dangling() {
        let root = scratch("central");
        std::fs::create_dir_all(root.join("courses/MULT20015_2026_SM2")).unwrap();

        ensure_library_docs(&root).unwrap();
        link_course(&root, "MULT20015_2026_SM2").unwrap();

        let through_link =
            std::fs::read_to_string(root.join("courses/MULT20015_2026_SM2/AGENTS.md")).unwrap();
        assert_eq!(through_link, AGENTS_DOC);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The stubs are the only thing in `agents/` a human authors; a sync runs
    /// far more often than `oculus docs` and must never touch them.
    #[test]
    fn stubs_are_written_once_and_then_left_alone() {
        let root = scratch("stubs");
        let first = ensure_library_docs(&root).unwrap();
        assert_eq!(first.created, ["OCULUS.md", "TASTE.md", "memories/MEMORY.md"]);

        std::fs::write(agents_dir(&root).join("TASTE.md"), "my notes").unwrap();
        let second = ensure_library_docs(&root).unwrap();
        assert!(second.created.is_empty());
        assert_eq!(second.generated, [AGENTS_DOC_NAME]);
        assert_eq!(
            std::fs::read_to_string(agents_dir(&root).join("TASTE.md")).unwrap(),
            "my notes"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
