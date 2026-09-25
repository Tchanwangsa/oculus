//! The agent-facing docs that live in the library.
//!
//! `agents/` in the data directory holds one `AGENTS.md` for every subject,
//! symlinked into each course folder, the stubs a human authors, and the memory
//! layer an agent writes back into (`TASTE.md`, `memories/`, `memories/<CODE>/`).
//! It is written from `oculus docs` and from every sync, so the rules about what
//! may be overwritten live here.
//!
//! Both memory buckets live under `agents/` because it is the only place an
//! in-app thread may write; [`link_dir`] leaves a symlink in each course folder
//! so the old per-course path lands in the same store. Nothing here reads the
//! memory layer back or writes a memory — [`crate::memory`] owns the contents.
//!
//! `agents/skills/` is the one copy of the skills: Claude Code and Codex find it
//! through relative links (`.claude/skills`, `.agents/skills`), opencode through
//! its config. Skills and `AGENTS.md` are regenerated every time; a stale one is
//! followed as a procedure. See docs/harness.md.

use std::path::{Path, PathBuf};

use crate::paths;

/// One `AGENTS.md` for every subject. Per-subject additions go in that folder's
/// `agents/INSTRUCTIONS.md`, which nothing here writes.
pub const AGENTS_DOC: &str = include_str!("../templates/AGENTS.template.md");
const OCULUS_DOC: &str = include_str!("../templates/OCULUS.template.md");
const TASTE_DOC: &str = include_str!("../templates/TASTE.template.md");

const MEMORY_INDEX_DOC: &str = include_str!("../templates/MEMORY.template.md");

/// The procedures an agent loads by name. Kept short: every CLI reads the
/// skill index into the first prompt of every thread, headless jobs included.
const SKILLS: [(&str, &str); 3] = [
    (
        "oculus-lectures",
        include_str!("../templates/skills/oculus-lectures/SKILL.md"),
    ),
    (
        "oculus-library",
        include_str!("../templates/skills/oculus-library/SKILL.md"),
    ),
    (
        "oculus-plan",
        include_str!("../templates/skills/oculus-plan/SKILL.md"),
    ),
];

pub const AGENTS_DOC_NAME: &str = "AGENTS.md";
pub const CLI_DOC_NAME: &str = "OCULUS-CLI.md";
const SKILLS_DIR: &str = "skills";
const SKILL_DOC_NAME: &str = "SKILL.md";
const TASTE_DOC_NAME: &str = "TASTE.md";
/// The index beside the memories, in both buckets.
const MEMORY_INDEX_NAME: &str = "MEMORY.md";
const MEMORIES_DIR: &str = "memories";

/// From a course folder to the one central copy; relative so the library can move.
const AGENTS_DOC_REL: &str = "../../agents/AGENTS.md";

pub fn agents_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("agents")
}

/// The one copy of the skills. Everything else about them is a route to here.
pub fn skills_dir(data_dir: &Path) -> PathBuf {
    agents_dir(data_dir).join(SKILLS_DIR)
}

/// What [`ensure_library_docs`] did, so a caller can report it.
#[derive(Default)]
pub struct LibraryDocs {
    /// Rewritten every time.
    pub generated: Vec<&'static str>,
    /// Stubs that were missing and have just been created.
    pub created: Vec<&'static str>,
    /// Stubs whose guidance was updated with the user's text carried across — see
    /// [`refresh_taste`].
    pub refreshed: Vec<&'static str>,
    /// Stubs edited past where that merge is safe, so left alone.
    pub diverged: Vec<&'static str>,
}

/// Fill `agents/` with everything that does not need the CLI's own help tree.
///
///
/// Idempotent, so `oculus docs` and every sync call it blind. `OCULUS-CLI.md`
/// needs clap's command tree, so only the binary writes it.
pub fn ensure_library_docs(data_dir: &Path) -> Result<LibraryDocs, String> {
    let dir = agents_dir(data_dir);
    std::fs::create_dir_all(dir.join(MEMORIES_DIR))
        .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

    let mut docs = LibraryDocs::default();

    // Generated: overwritten every time.
    let path = dir.join(AGENTS_DOC_NAME);
    std::fs::write(&path, AGENTS_DOC).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    docs.generated.push(AGENTS_DOC_NAME);

    // Stubs: written once, then the user's. The memory index is stubbed so an
    // empty bucket says what goes in it.
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

    // An existing stub's guidance half is a prompt that goes stale; only the
    // user's half is kept.
    match refresh_taste(&dir.join(TASTE_DOC_NAME))? {
        Refresh::Rewritten => docs.refreshed.push(TASTE_DOC_NAME),
        Refresh::Diverged => docs.diverged.push(TASTE_DOC_NAME),
        Refresh::Current => {}
    }

    // Generated: a stale skill is followed as a procedure.
    let skills = skills_dir(data_dir);
    for (name, body) in SKILLS {
        let dir = skills.join(name);
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        let path = dir.join(SKILL_DOC_NAME);
        std::fs::write(&path, body)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        docs.generated.push(name);
    }
    link_agent_skills(data_dir)?;
    Ok(docs)
}

/// What [`refresh_taste`] decided to do.
pub enum Refresh {
    /// The guidance was behind; it has been replaced and the bullets kept.
    Rewritten,
    /// Already the current text, or empty of anything to keep.
    Current,
    /// Edited past what a merge can be sure about, so left alone.
    Diverged,
}

/// Bring `TASTE.md`'s instructions up to date without touching what the user
/// put in it.
///
///
/// The template is re-rendered and the user's bullets are carried under their
/// headings. It refuses rather than guesses: prose under a heading, or no
/// known heading left, is reported and left alone.
pub fn refresh_taste(path: &Path) -> Result<Refresh, String> {
    let Ok(current) = std::fs::read_to_string(path) else {
        return Ok(Refresh::Current);
    };
    if current == TASTE_DOC {
        return Ok(Refresh::Current);
    }

    let mut kept: Vec<(String, Vec<String>)> = TASTE_SECTIONS
        .iter()
        .map(|h| ((*h).to_string(), Vec::new()))
        .collect();
    let mut section: Option<usize> = None;
    let mut seen = 0;
    for line in current.lines() {
        if let Some(name) = line.strip_prefix("## ") {
            section = kept
                .iter()
                .position(|(h, _)| h.eq_ignore_ascii_case(name.trim()));
            if section.is_some() {
                seen += 1;
            }
            continue;
        }
        let Some(idx) = section else { continue };
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if !t.starts_with("- ") && !t.starts_with("* ") {
            // Prose under a heading: not a shape the merge can take apart.
            return Ok(Refresh::Diverged);
        }
        kept[idx]
            .1
            .push(format!("- {}", t.trim_start_matches(['-', '*']).trim()));
    }
    if seen == 0 {
        return Ok(Refresh::Diverged);
    }

    let mut out = TASTE_DOC.to_string();
    for (heading, bullets) in &kept {
        if bullets.is_empty() {
            continue;
        }
        let marker = format!("## {heading}\n");
        let Some(at) = out.find(&marker) else {
            continue;
        };
        let at = at + marker.len();
        out.insert_str(at, &format!("\n{}\n", bullets.join("\n")));
    }
    if out == current {
        return Ok(Refresh::Current);
    }
    std::fs::write(path, out).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(Refresh::Rewritten)
}

/// The headings `TASTE.md` ships with; the only ones a bullet carries under.
const TASTE_SECTIONS: [&str; 3] = ["Writing", "Working", "Study"];

/// Claude Code walks up from its cwd for `.claude/skills` and Codex for
/// `.agents/skills`; every thread and headless job runs in `agents/`, so both
/// links sit there, relative so the library stays movable. Nothing is written
/// outside the library (not `$CODEX_HOME`).
const SCANNED_SKILL_DIRS: [&str; 2] = [".claude", ".agents"];

fn link_agent_skills(data_dir: &Path) -> Result<(), String> {
    for scanned in SCANNED_SKILL_DIRS {
        let into = agents_dir(data_dir).join(scanned).join(SKILLS_DIR);
        std::fs::create_dir_all(&into)
            .map_err(|e| format!("cannot create {}: {e}", into.display()))?;
        for (name, body) in SKILLS {
            // Up out of `<scanned>/skills`, back down into `skills/` beside it.
            link_skill(&into.join(name), &format!("../../{SKILLS_DIR}/{name}"), name, body)?;
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
            // Only if emptied: a folder beats a link that hides unplaced files.
            if std::fs::read_dir(&old).into_iter().flatten().flatten().next().is_some() {
                return Ok(());
            }
            std::fs::remove_dir(&old).map_err(|e| format!("cannot replace {name}/agents/memories: {e}"))?;
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
    let name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
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
    std::fs::write(link.join(SKILL_DOC_NAME), body)
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    /// A hand-written AGENTS.md in a course folder must survive relinking.
    #[test]
    fn linking_replaces_stale_links_but_never_real_files() {
        let root = Scratch::new("agents-link");
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
        // The bucket is under `agents/`; the course folder gets a link to it.
        assert!(root.join("agents/memories/fresh").is_dir());
        assert!(std::fs::symlink_metadata(courses.join("fresh/agents/memories"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(courses.join("fresh/agents/memories").is_dir(), "the link resolves");
        // Named for the subject, so the bucket a memory lands in is unambiguous.
        let index = std::fs::read_to_string(root.join("agents/memories/fresh/MEMORY.md")).unwrap();
        assert!(index.starts_with("# Memories — fresh"));

        // Second run is a no-op.
        let again = link_all(&root).unwrap();
        assert!(again.linked.is_empty());
        assert_eq!(again.current, 2);
    }

    /// Memories already filed in a course folder move into the library bucket.
    #[test]
    fn memories_filed_in_a_course_folder_are_adopted() {
        let root = Scratch::new("agents-adopt");
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
        // And the old path resolves to the same file.
        assert!(old.join("info30006-mst.md").is_file());
        assert!(std::fs::symlink_metadata(&old).unwrap().file_type().is_symlink());

        // Idempotent: a second sync neither re-moves nor re-links.
        link_all(&root).unwrap();
        assert!(old.join("info30006-mst.md").is_file());
    }

    /// A newly scraped subject, and one that produced no folder.
    #[test]
    fn a_sync_links_scraped_subjects_and_invents_no_folders() {
        let root = Scratch::new("agents-course");
        std::fs::create_dir_all(root.join("courses/COMP30026_2026_SM2")).unwrap();

        assert_eq!(link_course(&root, "COMP30026_2026_SM2").unwrap(), Link::Linked);
        assert_eq!(link_course(&root, "COMP30026_2026_SM2").unwrap(), Link::Current);

        // A scrape that wrote nothing must not leave an empty course folder.
        assert_eq!(link_course(&root, "NEW10001_2026_SM2").unwrap(), Link::Absent);
        assert!(!root.join("courses/NEW10001_2026_SM2").exists());
    }

    /// A sync writes the central copy before linking to it.
    #[test]
    fn sync_side_links_are_never_dangling() {
        let root = Scratch::new("agents-central");
        std::fs::create_dir_all(root.join("courses/MULT20015_2026_SM2")).unwrap();

        ensure_library_docs(&root).unwrap();
        link_course(&root, "MULT20015_2026_SM2").unwrap();

        let through_link =
            std::fs::read_to_string(root.join("courses/MULT20015_2026_SM2/AGENTS.md")).unwrap();
        assert_eq!(through_link, AGENTS_DOC);
    }

    /// `TASTE.md`'s guidance moves; the user's bullets do not.
    #[test]
    fn taste_keeps_its_bullets_and_takes_the_new_guidance() {
        let root = Scratch::new("agents-taste");
        std::fs::create_dir_all(&root).unwrap();
        ensure_library_docs(&root).unwrap();
        let path = agents_dir(&root).join(TASTE_DOC_NAME);

        // A stub from an older build, with two preferences written into it.
        std::fs::write(
            &path,
            "# Preferences\n\nOculus reads this file into the chat's prompt on every message.\n\n## Writing\n\n- Lead with the verdict.\n\n## Working\n\n- Use bun, never npm.\n\n## Study\n",
        )
        .unwrap();

        let docs = ensure_library_docs(&root).unwrap();
        assert_eq!(docs.refreshed, vec![TASTE_DOC_NAME]);
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(
            after.contains("- Lead with the verdict."),
            "the bullets came across"
        );
        assert!(
            after.contains("- Use bun, never npm."),
            "under their own headings"
        );
        assert!(
            after.find("- Lead with the verdict.").unwrap() < after.find("## Working").unwrap(),
            "and in the right sections"
        );
        assert!(
            !after.contains("into the chat's prompt on every message"),
            "the stale line went"
        );

        // Idempotent: a second pass has nothing to do.
        let again = ensure_library_docs(&root).unwrap();
        assert!(
            again.refreshed.is_empty(),
            "a current file is not rewritten"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), after);
    }

    /// Prose under a heading is refused, not guessed at.
    #[test]
    fn a_taste_file_written_another_way_is_left_alone() {
        let root = Scratch::new("agents-taste-diverged");
        std::fs::create_dir_all(&root).unwrap();
        ensure_library_docs(&root).unwrap();
        let path = agents_dir(&root).join(TASTE_DOC_NAME);

        let mine = "# Preferences\n\n## Writing\n\nI want short answers, and I will explain \
                    why below.\n\n## Working\n\n## Study\n";
        std::fs::write(&path, mine).unwrap();
        let docs = ensure_library_docs(&root).unwrap();
        assert_eq!(docs.diverged, vec![TASTE_DOC_NAME]);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), mine, "untouched");

        // So is a file whose headings have gone entirely.
        std::fs::write(&path, "# Preferences\n\nJust a paragraph.\n").unwrap();
        let docs = ensure_library_docs(&root).unwrap();
        assert_eq!(docs.diverged, vec![TASTE_DOC_NAME]);
    }

    #[test]
    fn stubs_are_written_once_and_then_left_alone() {
        let root = Scratch::new("agents-stubs");
        let first = ensure_library_docs(&root).unwrap();
        assert_eq!(first.created, ["OCULUS.md", "TASTE.md", "memories/MEMORY.md"]);

        std::fs::write(agents_dir(&root).join("TASTE.md"), "my notes").unwrap();
        let second = ensure_library_docs(&root).unwrap();
        assert!(second.created.is_empty());
        assert_eq!(
            second.generated,
            [AGENTS_DOC_NAME, "oculus-lectures", "oculus-library", "oculus-plan"]
        );
        assert_eq!(
            std::fs::read_to_string(agents_dir(&root).join("TASTE.md")).unwrap(),
            "my notes"
        );
    }

    /// Skills are always overwritten: an agent-edited copy would be followed.
    #[test]
    fn skills_are_rewritten_over_whatever_is_there() {
        let root = Scratch::new("agents-skills");
        ensure_library_docs(&root).unwrap();

        let plan = skills_dir(&root).join("oculus-plan/SKILL.md");
        assert!(plan.is_file());
        assert!(std::fs::read_to_string(&plan).unwrap().contains("name: oculus-plan"));

        std::fs::write(&plan, "do whatever you like").unwrap();
        ensure_library_docs(&root).unwrap();
        assert!(std::fs::read_to_string(&plan).unwrap().contains("name: oculus-plan"));
    }

    /// Three CLIs, three discovery paths, one directory.
    #[test]
    fn all_three_clis_reach_the_one_skills_directory() {
        let root = Scratch::new("agents-skill-links");
        ensure_library_docs(&root).unwrap();

        // Both scan up from the cwd, which is `agents/` for every thread and job.
        for scanned in SCANNED_SKILL_DIRS {
            let link = agents_dir(&root).join(scanned).join("skills/oculus-lectures");
            assert_eq!(
                std::fs::read_link(&link).unwrap(),
                PathBuf::from("../../skills/oculus-lectures"),
                "{scanned}: relative, so the library stays movable"
            );
            assert_eq!(
                std::fs::read_to_string(link.join(SKILL_DOC_NAME)).unwrap(),
                SKILLS[0].1,
                "{scanned}: and it resolves to the one copy"
            );
        }

        // opencode takes a config key instead (`opencode.rs`).
        assert!(skills_dir(&root).join("oculus-lectures").join(SKILL_DOC_NAME).is_file());

        // Idempotent.
        ensure_library_docs(&root).unwrap();
        for scanned in SCANNED_SKILL_DIRS {
            let link = agents_dir(&root).join(scanned).join("skills/oculus-lectures");
            assert!(link.join(SKILL_DOC_NAME).is_file(), "{scanned}");
        }
    }

    /// Nothing this module writes leaves the library.
    #[test]
    fn nothing_is_written_outside_the_library() {
        let root = Scratch::new("agents-skill-contained");
        let home = root.join("home");
        std::fs::create_dir_all(home.join(".codex")).unwrap();

        ensure_library_docs(&root).unwrap();

        assert!(
            !home.join(".codex/skills").exists(),
            "reached into {}",
            home.join(".codex/skills").display()
        );
        assert!(agents_dir(&root).join(".agents/skills/oculus-plan").is_symlink());
    }

    /// A link is ours and gets repointed; a real file is somebody's.
    #[test]
    fn a_real_skill_on_the_link_path_is_left_alone() {
        let root = Scratch::new("agents-skill-mine");
        let into = agents_dir(&root).join(".agents/skills");
        std::fs::create_dir_all(&into).unwrap();
        std::fs::write(into.join("oculus-plan"), "mine").unwrap();
        // And a stale link, which is ours and must be repointed.
        std::os::unix::fs::symlink("/nowhere", into.join("oculus-lectures")).unwrap();

        ensure_library_docs(&root).unwrap();

        assert_eq!(std::fs::read_to_string(into.join("oculus-plan")).unwrap(), "mine");
        assert_eq!(
            std::fs::read_link(into.join("oculus-lectures")).unwrap(),
            PathBuf::from("../../skills/oculus-lectures")
        );
    }
}
