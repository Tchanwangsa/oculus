//! The agent-facing docs that live in the library.
//!
//! `agents/` in the data directory holds one `AGENTS.md` for every subject,
//! symlinked into each course folder, the stubs a human authors, and the memory
//! layer an agent writes back into (`TASTE.md`, `memories/`, `memories/<CODE>/`).
//! It is written from `oculus docs` and from every sync, so the rules about what
//! may be overwritten live here.
//!
//! Both memory buckets live under `agents/` because an in-app thread may write
//! it and not a subject's `.oculus/`; [`link_dir`] leaves a symlink in each
//! course folder so the per-course path lands in the same store. Nothing here reads the
//! memory layer back or writes a memory — [`crate::agents::memory`] owns the contents.
//!
//! `agents/skills/` is the one copy of the skills: Claude Code and Codex find it
//! through relative links (`.claude/skills`, `.agents/skills`), opencode through
//! its config. Skills and `AGENTS.md` are regenerated every time; a stale one is
//! followed as a procedure. See docs/harness.md.

mod docs;
mod links;
pub mod memory;
mod taste;
#[cfg(test)]
#[cfg(unix)]
mod tests;

pub use docs::{ensure_library_docs, LibraryDocs, AGENTS_DOC};
pub use links::{link_all, link_course, subject_memories, Link, LinkReport};
pub use taste::{refresh_taste, Refresh};

use std::path::{Path, PathBuf};

pub const AGENTS_DOC_NAME: &str = "AGENTS.md";
pub const CLI_DOC_NAME: &str = "OCULUS-CLI.md";
const SKILLS_DIR: &str = "skills";
const SKILL_DOC_NAME: &str = "SKILL.md";
const TASTE_DOC_NAME: &str = "TASTE.md";
/// The index beside the memories, in both buckets.
const MEMORY_INDEX_NAME: &str = "MEMORY.md";
pub const MEMORIES_DIR: &str = "memories";

/// From a course folder to the one central copy; relative so the library can move.
const AGENTS_DOC_REL: &str = "../../agents/AGENTS.md";

/// The agents' workspace, at the data dir's top: every thread's cwd.
pub const AGENTS_DIR: &str = "agents";

pub fn agents_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(AGENTS_DIR)
}

/// The one copy of the skills. Everything else about them is a route to here.
pub fn skills_dir(data_dir: &Path) -> PathBuf {
    agents_dir(data_dir).join(SKILLS_DIR)
}
