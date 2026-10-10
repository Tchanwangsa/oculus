use std::path::{Path, PathBuf};

use crate::library::paths;

use super::docs::SKILLS;
use super::{
    agents_dir, AGENTS_DOC, AGENTS_DOC_NAME, AGENTS_DOC_REL, MEMORIES_DIR, MEMORY_INDEX_NAME,
    SKILLS_DIR,
};

/// Claude Code walks up from its cwd for `.claude/skills` and Codex for
/// `.agents/skills`; every thread and headless job runs in `agents/`, so both
/// links sit there, relative so the library stays movable. Nothing is written
/// outside the library (not `$CODEX_HOME`).
pub(super) const SCANNED_SKILL_DIRS: [&str; 2] = [".claude", ".agents"];

pub(super) fn link_agent_skills(data_dir: &Path) -> Result<(), String> {
    for scanned in SCANNED_SKILL_DIRS {
        let into = agents_dir(data_dir).join(scanned).join(SKILLS_DIR);
        std::fs::create_dir_all(&into)
            .map_err(|e| format!("cannot create {}: {e}", into.display()))?;
        for (name, body) in SKILLS {
            // Up out of `<scanned>/skills`, back down into `skills/` beside it.
            link_skill(
                &into.join(name),
                &format!("../../{SKILLS_DIR}/{name}"),
                name,
                body,
            )?;
        }
    }
    Ok(())
}

fn link_skill(link: &Path, target: &str, name: &str, body: &str) -> Result<Link, String> {
    match std::fs::symlink_metadata(link) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if std::fs::read_link(link).is_ok_and(|t| t == Path::new(target)) {
                return Ok(Link::Current);
            }
            std::fs::remove_file(link).map_err(|e| format!("cannot relink {name}: {e}"))?;
        }
        Ok(_) => {
            eprintln!("[oculus] {} is not ours — left alone", link.display());
            return Ok(Link::Skipped);
        }
        Err(_) => {}
    }
    link_skill_dir(target, link, body).map_err(|e| format!("cannot link {name}: {e}"))?;
    Ok(Link::Linked)
}

/// What linking one course folder did.
#[derive(Debug, PartialEq, Eq)]
pub enum Link {
    Linked,
    /// Already pointing at the right place.
    Current,
    /// A real file was there; left alone rather than overwritten.
    Skipped,
    /// No such course folder; nothing is created.
    Absent,
}

/// Scaffold and link one subject's course folder, addressed by Canvas code.
///
/// The sync-side entry point, so a new subject is linked by its first scrape.
pub fn link_course(data_dir: &Path, code: &str) -> Result<Link, String> {
    let dir = data_dir.join("courses").join(paths::safe_dir(code));
    if !dir.is_dir() {
        return Ok(Link::Absent);
    }
    link_dir(data_dir, &dir)
}

/// Point every existing course folder at the one `AGENTS.md`.
///
/// The sweep behind `oculus docs`.
pub fn link_all(data_dir: &Path) -> Result<LinkReport, String> {
    let courses = data_dir.join("courses");
    let mut report = LinkReport::default();
    let entries = match std::fs::read_dir(&courses) {
        Ok(e) => e,
        // No courses yet is not a failure.
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

/// One subject's memory bucket, under the library's writable `agents/`.
pub fn subject_memories(data_dir: &Path, course_dir: &str) -> PathBuf {
    agents_dir(data_dir).join(MEMORIES_DIR).join(course_dir)
}

/// From `courses/<CODE>/agents/memories` back to that bucket (relative).
const SUBJECT_MEMORIES_REL: &str = "../../../agents/memories";

/// Move a course folder's old `agents/memories/` into the library bucket and
///
/// A terminal agent in a course folder may still write the old path, so the
/// link keeps it pointing into the one store, inside the sandbox's writable
/// root. Idempotent: a correct link is left alone, and a colliding file is left
/// in place rather than overwriting what is filed.
fn adopt_course_memories(course_agents: &Path, bucket: &Path, name: &str) -> Result<(), String> {
    let old = course_agents.join(MEMORIES_DIR);
    match std::fs::symlink_metadata(&old) {
        // Already a link; repoint it if it aims elsewhere (links are ours).
        Ok(meta) if meta.file_type().is_symlink() => {
            let want = PathBuf::from(SUBJECT_MEMORIES_REL).join(name);
            if std::fs::read_link(&old).is_ok_and(|t| t == want) {
                return Ok(());
            }
            std::fs::remove_file(&old)
                .map_err(|e| format!("cannot relink {name}/agents/memories: {e}"))?;
        }
        Ok(meta) if meta.is_dir() => {
            for entry in std::fs::read_dir(&old).into_iter().flatten().flatten() {
                let to = bucket.join(entry.file_name());
                if to.exists() {
                    continue;
                }
                std::fs::rename(entry.path(), &to).map_err(|e| {
                    format!(
                        "cannot move {name}/agents/memories/{:?}: {e}",
                        entry.file_name()
                    )
                })?;
            }
            // Only if emptied: a folder beats a link that hides unplaced files.
            if std::fs::read_dir(&old)
                .into_iter()
                .flatten()
                .flatten()
                .next()
                .is_some()
            {
                return Ok(());
            }
            std::fs::remove_dir(&old)
                .map_err(|e| format!("cannot replace {name}/agents/memories: {e}"))?;
        }
        // A real file called `memories` is somebody's.
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

/// A relative symlink, skipped rather than clobbered when a real file is there.
fn link_dir(data_dir: &Path, dir: &Path) -> Result<Link, String> {
    let name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let bucket = subject_memories(data_dir, &name);
    std::fs::create_dir_all(&bucket)
        .map_err(|e| format!("cannot create agents/memories/{name}: {e}"))?;
    let course_agents = dir.join("agents");
    std::fs::create_dir_all(&course_agents)
        .map_err(|e| format!("cannot create {name}/agents: {e}"))?;
    adopt_course_memories(&course_agents, &bucket, &name)?;

    // Named for the subject so the bucket is unmistakable. Written after the
    // migration so an index that came across is not replaced by a stub.
    let index = bucket.join(MEMORY_INDEX_NAME);
    if !index.exists() {
        let body = format!(
            "# Memories — {name}\n\n\
             Facts about this subject alone — including ones that came up while\n\
             working on something else. Anything true across subjects, or about the\n\
             student themselves, goes in the library's own `agents/memories/`.\n\n\
             The list below is written by `oculus memory` from the files beside it,\n\
             so it cannot fall behind them. Everything above the marker is yours.\n"
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

/// A relative symlink, or a plain copy where symlinks need a privilege (Windows).
#[cfg(unix)]
fn symlink_or_copy(target: &str, link: &Path, _body: &str) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn symlink_or_copy(_target: &str, link: &Path, body: &str) -> std::io::Result<()> {
    std::fs::write(link, body)
}

/// The same for a skill directory: the copy rebuilds the directory's shape.
#[cfg(unix)]
fn link_skill_dir(target: &str, link: &Path, _body: &str) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn link_skill_dir(_target: &str, link: &Path, body: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(link)?;
    std::fs::write(link.join(super::SKILL_DOC_NAME), body)
}
