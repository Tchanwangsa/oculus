//! The agent-facing docs that live in the library.
//!
//! `agents/` in the data directory holds one `AGENTS.md` for every subject,
//! symlinked into each course folder, plus the stubs a human authors. It is
//! written from two places — `oculus docs` fills the whole layer on demand,
//! and a sync links whatever subjects it just scraped — so the rules about
//! what may be overwritten live here rather than in either caller.
//!
//! It also holds the memory layer an agent writes back into: `TASTE.md` for
//! standing preferences and `memories/` for facts, once globally and once per
//! course folder. Nothing here ever writes a memory — the shape of the layer
//! lives here because [`user_context`] reads it back for the in-app chat, and
//! a reader and a scaffolder that disagree would be the whole bug.

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
/// The index beside the memories, in both buckets. Skipped by [`user_context`]:
/// it is a table of contents for the files it sits with, so a reader that has
/// the files themselves would only be reading their titles twice.
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

// ── What the in-app chat reads back ──────────────────────────────────────────

/// Cap on the whole injected block. It rides in the system prompt of every
/// turn of every chat, so this is a per-message cost rather than a one-off —
/// and the global bucket is meant to stay small enough that it never bites.
const MAX_CONTEXT_CHARS: usize = 8_000;

/// The global memory layer, as one block for the chat agent's system prompt.
///
/// `TASTE.md` plus the *bodies* of every cross-subject memory — bodies, not
/// the index, because the in-app agent has no filesystem tool and a title it
/// cannot open is worse than no title at all. Subject memories are left out
/// on purpose: they are the other bucket, and a chat is not scoped to one
/// subject.
///
/// `None` when there is nothing to say, so a library nobody has taught
/// anything pays nothing for the feature.
pub fn user_context(data_dir: &Path) -> Option<String> {
    let dir = agents_dir(data_dir);
    let mut out = String::new();

    if let Some(taste) = authored(&dir.join(TASTE_DOC_NAME), TASTE_DOC) {
        out.push_str("### Standing preferences, in the student's own words\n\n");
        out.push_str(&taste);
        out.push_str("\n\n");
    }

    let mut files: Vec<PathBuf> = std::fs::read_dir(dir.join(MEMORIES_DIR))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "md")
                && p.file_name().is_some_and(|n| n != MEMORY_INDEX_NAME)
        })
        .collect();
    // Sorted so the same library produces the same prompt twice running, which
    // is what makes a cached prefix worth anything.
    files.sort();

    for path in files {
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        let body = strip_frontmatter(&body).trim();
        if body.is_empty() {
            continue;
        }
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        out.push_str(&format!("### {stem}\n\n{body}\n\n"));
    }

    let out = out.trim();
    if out.is_empty() {
        return None;
    }
    Some(format!(
        "## What you already know about this student\n\n\
         Written down over earlier sessions, by you and by the coding agents that \
         work in the library folder. Treat it as true unless this conversation \
         shows otherwise, and never read it aloud unprompted.\n\n{}",
        truncate(out, MAX_CONTEXT_CHARS)
    ))
}

/// A stub is a prompt to the user, not a fact about them: injecting one would
/// tell the model the student prefers nothing in particular, which is a claim
/// the empty file never made. Compared against the shipped template with the
/// stub note removed, so deleting that note does not by itself count as
/// having written something.
fn authored(path: &Path, template: &str) -> Option<String> {
    let body = std::fs::read_to_string(path).ok()?;
    let body = without_blockquotes(&body);
    (body != without_blockquotes(template)).then_some(body)
}

fn without_blockquotes(md: &str) -> String {
    md.lines()
        .filter(|l| !l.trim_start().starts_with('>'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Drop a leading `---` YAML block. The frontmatter is addressing whoever
/// files the memory — name, description, type — and the model needs the fact.
fn strip_frontmatter(md: &str) -> &str {
    md.strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---"))
        .map_or(md, |(_, body)| body)
}

/// On a character boundary, and said out loud: a model that is silently handed
/// half a memory should be told the other half exists.
fn truncate(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        None => s.to_string(),
        Some((end, _)) => format!("{}\n\n(Older memories omitted — the store is larger than fits here.)", &s[..end]),
    }
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
    link_dir(&dir)
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
        match link_dir(&dir)? {
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

/// A relative symlink, so the whole library stays movable, and skipped rather
/// than clobbered when a real file is already sitting there.
fn link_dir(dir: &Path) -> Result<Link, String> {
    let name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
    let memories = dir.join("agents").join(MEMORIES_DIR);
    std::fs::create_dir_all(&memories)
        .map_err(|e| format!("cannot create {name}/agents: {e}"))?;

    // Named for the folder, because the one thing this index has to make
    // obvious is which bucket it is — a subject memory filed globally, or the
    // reverse, is the mistake the two-bucket split exists to prevent.
    let index = memories.join(MEMORY_INDEX_NAME);
    if !index.exists() {
        let body = format!(
            "# Memories — {name}\n\n\
             Facts about this subject alone. Anything true across subjects goes in the\n\
             library's own `agents/memories/`.\n\n\
             <!-- - [Title](file-name.md) — the hook, in a clause -->\n"
        );
        std::fs::write(&index, body).map_err(|e| format!("cannot write {name}/{}: {e}", MEMORY_INDEX_NAME))?;
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
        assert!(courses.join("fresh/agents/memories").is_dir());
        // Named for the folder, so the bucket a memory lands in is unambiguous.
        let index =
            std::fs::read_to_string(courses.join("fresh/agents/memories/MEMORY.md")).unwrap();
        assert!(index.starts_with("# Memories — fresh"));

        // Second run is a no-op, which is what lets cli:install call it blind.
        let again = link_all(&root).unwrap();
        assert!(again.linked.is_empty());
        assert_eq!(again.current, 2);
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

    /// Nothing written yet must cost the chat nothing — and a stub is a prompt
    /// to the user, not a statement that the user prefers nothing.
    #[test]
    fn a_library_nobody_has_taught_anything_contributes_no_prompt() {
        let root = scratch("context-empty");
        ensure_library_docs(&root).unwrap();
        assert!(user_context(&root).is_none());

        // Deleting the stub note is not the same as having written something.
        let taste = agents_dir(&root).join("TASTE.md");
        let body = std::fs::read_to_string(&taste).unwrap();
        let stripped: String = body
            .lines()
            .filter(|l| !l.trim_start().starts_with('>'))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&taste, stripped).unwrap();
        assert!(user_context(&root).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// What the chat actually receives: preferences and memory *bodies*, with
    /// the frontmatter and the index left behind.
    #[test]
    fn taste_and_global_memories_reach_the_prompt_without_their_bookkeeping() {
        let root = scratch("context-full");
        ensure_library_docs(&root).unwrap();
        let dir = agents_dir(&root);
        std::fs::write(
            dir.join("TASTE.md"),
            "# Preferences\n\n## Writing\n\n- Lead with the verdict.\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("memories/study-workflow.md"),
            "---\nname: study-workflow\ndescription: how he triages\n---\n\nTriages by ROI.\n",
        )
        .unwrap();
        // The index names the files that follow it; sending both would be
        // sending every title twice.
        std::fs::write(dir.join("memories/MEMORY.md"), "- [Workflow](study-workflow.md) — ROI\n")
            .unwrap();

        let context = user_context(&root).unwrap();
        assert!(context.contains("Lead with the verdict."));
        assert!(context.contains("Triages by ROI."));
        assert!(!context.contains("description: how he triages"));
        assert!(!context.contains("[Workflow]"));

        // A subject memory is the other bucket's business.
        std::fs::create_dir_all(root.join("courses/COMP30026/agents/memories")).unwrap();
        std::fs::write(
            root.join("courses/COMP30026/agents/memories/exam.md"),
            "Exam is open book.",
        )
        .unwrap();
        assert!(!user_context(&root).unwrap().contains("open book"));
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
