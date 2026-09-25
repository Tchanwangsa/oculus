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
//! remembers the old path still writes into the one store. **Nothing here ever
//! writes a memory**: this module owns the shape of the folder, and
//! [`crate::memory`] — behind `oculus memory` — owns what goes in it, down to
//! the `MEMORY.md` index, which is stubbed here and rewritten from the files
//! there.
//!
//! **Skills are the third thing written here, and the one directory all three
//! CLIs are pointed at.** `agents/skills/<name>/SKILL.md` is a procedure an
//! agent loads by name when a request matches it, rather than prose every
//! prompt carries. Each CLI finds them a different way, and two of the three
//! ways are the same shape: Claude Code scans `<cwd>/.claude/skills` and
//! Codex scans `<cwd>/.agents/skills`, both walking up from the working
//! directory, so each gets a relative link beside the one copy. opencode
//! takes a `skills.paths` key in its config instead, which is a line in the
//! generated `opencode.json` rather than a link. Nothing here writes outside
//! the library. They describe this binary, so they are generated and
//! overwritten like `AGENTS.md`: a skill that documents a flag the CLI no
//! longer has is worse than no skill.

use std::path::{Path, PathBuf};

use crate::paths;

/// One `AGENTS.md` for every subject, so there is nothing per-course to keep
/// in sync. Anything genuinely per-subject goes in that folder's
/// `agents/INSTRUCTIONS.md`, which nothing here ever writes.
pub const AGENTS_DOC: &str = include_str!("../templates/AGENTS.template.md");
const OCULUS_DOC: &str = include_str!("../templates/OCULUS.template.md");
const TASTE_DOC: &str = include_str!("../templates/TASTE.template.md");

const MEMORY_INDEX_DOC: &str = include_str!("../templates/MEMORY.template.md");

/// The procedures an agent loads by name, generated for the same reason
/// `AGENTS.md` is. Kept deliberately short: every one of them is in the
/// index each CLI builds at startup, and all three read that index into the
/// first prompt of every thread — including the headless jobs, which run from
/// the same folder. A skill nobody loads still costs every turn.
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

/// The one copy of the skills. Everything else about them is a route to here.
pub fn skills_dir(data_dir: &Path) -> PathBuf {
    agents_dir(data_dir).join(SKILLS_DIR)
}

/// What [`ensure_library_docs`] did, so a caller can report it.
#[derive(Default)]
pub struct LibraryDocs {
    /// Rewritten every time — a stale copy would describe a layout that no
    /// longer exists.
    pub generated: Vec<&'static str>,
    /// Stubs that were missing and have just been created.
    pub created: Vec<&'static str>,
    /// Stubs whose *guidance* was brought up to date while what the user
    /// wrote in them was carried across — see [`refresh_taste`].
    pub refreshed: Vec<&'static str>,
    /// Stubs that have been edited past the point where that merge is safe,
    /// so they were left alone and said so.
    pub diverged: Vec<&'static str>,
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

    // A stub that already exists is the user's, but only the half of it they
    // wrote. The other half is instructions to an agent, and those go stale
    // the moment the thing they describe changes — `TASTE.md` in this library
    // spent a fortnight telling its reader that Oculus folded the file into
    // every prompt, which had not been true since the BYOK chat was deleted.
    // Written once and never again is the right rule for somebody's content
    // and the wrong one for a prompt.
    match refresh_taste(&dir.join(TASTE_DOC_NAME))? {
        Refresh::Rewritten => docs.refreshed.push(TASTE_DOC_NAME),
        Refresh::Diverged => docs.diverged.push(TASTE_DOC_NAME),
        Refresh::Current => {}
    }

    // Generated too, and for a sharper version of the same reason: a skill is
    // read as a procedure rather than as background, so one describing a flag
    // this binary no longer has is followed anyway.
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
/// The file is two things at once — a standing brief to the agent about when
/// something earns a line, and the lines themselves — and only the second half
/// is anybody's work. So the template is re-rendered and the bullets are
/// carried into it under the headings they were under.
///
/// **It refuses rather than guesses.** A heading carrying anything that is not
/// a bullet, or a file with none of the three headings left in it, has been
/// written in a way this cannot take apart, and overwriting it would be the
/// one unrecoverable thing this module does. Those are reported and left.
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
            // Prose under a heading: somebody is using this file in a way the
            // merge was not written for.
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

/// The headings `TASTE.md` ships with, and the only ones a bullet can be
/// carried across under.
const TASTE_SECTIONS: [&str; 3] = ["Writing", "Working", "Study"];

/// Both scanning CLIs get the same link, because they are the same problem:
/// Claude Code walks up from the working directory looking for
/// `.claude/skills`, Codex walks up looking for `.agents/skills`, and the
/// working directory of every thread and every headless job is `agents/`. So
/// each directory sits inside the folder it points into, and the links are
/// relative for the same reason the course folders' `AGENTS.md` is: the
/// library has to stay movable.
///
/// Codex also reads `$CODEX_HOME/skills`, and an earlier version of this put
/// the links there. That was wrong twice over — it wrote outside the library,
/// so two Oculus skills turned up in every Codex session on the machine, and
/// it treated Codex as the one needing special handling when it had the
/// project-level directory all along. Nothing here leaves `agents/` now.
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

/// The same trade for a skill, which is a *directory* with one file in it
/// rather than a file: where there are no symlinks the copy has to rebuild
/// that shape, not write the body at the link's own path.
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
    /// The half of `TASTE.md` that is a prompt has to be able to move, and the
    /// half that is the user's must not. This is the one file in `agents/`
    /// where those two live together.
    #[test]
    fn taste_keeps_its_bullets_and_takes_the_new_guidance() {
        let root = scratch("taste");
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

        let _ = std::fs::remove_dir_all(&root);
    }

    /// The refusal that makes the merge safe to run unattended. Prose under a
    /// heading is somebody using the file in a way this cannot take apart, and
    /// guessing at it would be the one unrecoverable thing here.
    #[test]
    fn a_taste_file_written_another_way_is_left_alone() {
        let root = scratch("taste-diverged");
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

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stubs_are_written_once_and_then_left_alone() {
        let root = scratch("stubs");
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
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The skills are the other half of "generated, always overwritten", and
    /// the half where it matters most: they are read as a procedure, so a
    /// copy an agent edited to suit itself would be followed rather than
    /// weighed. There is exactly one copy, and every run rewrites it.
    #[test]
    fn skills_are_rewritten_over_whatever_is_there() {
        let root = scratch("skills");
        ensure_library_docs(&root).unwrap();

        let plan = skills_dir(&root).join("oculus-plan/SKILL.md");
        assert!(plan.is_file());
        assert!(std::fs::read_to_string(&plan).unwrap().contains("name: oculus-plan"));

        std::fs::write(&plan, "do whatever you like").unwrap();
        ensure_library_docs(&root).unwrap();
        assert!(std::fs::read_to_string(&plan).unwrap().contains("name: oculus-plan"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Three CLIs, three discovery paths, one directory — and two of the
    /// three paths are the same shape, so they are built the same way.
    #[test]
    fn all_three_clis_reach_the_one_skills_directory() {
        let root = scratch("skill-links");
        ensure_library_docs(&root).unwrap();

        // Claude Code scans `<cwd>/.claude/skills` and Codex scans
        // `<cwd>/.agents/skills`, both walking up from the working directory
        // — and the cwd of a thread, and of a headless job, is `agents/`.
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

        // opencode takes a config key instead of a link; `opencode.rs` owns
        // that, and points it at this same directory.
        assert!(skills_dir(&root).join("oculus-lectures").join(SKILL_DOC_NAME).is_file());

        // Idempotent, because a sync calls this on every run.
        ensure_library_docs(&root).unwrap();
        for scanned in SCANNED_SKILL_DIRS {
            let link = agents_dir(&root).join(scanned).join("skills/oculus-lectures");
            assert!(link.join(SKILL_DOC_NAME).is_file(), "{scanned}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Nothing this module writes leaves the library. An earlier version put
    /// Codex's links in `$CODEX_HOME/skills`, which meant a Canvas sync
    /// reached into a directory shared with every other project on the
    /// machine — two Oculus skills in every Codex session, course library or
    /// not. Codex has a project-level directory; it gets that instead.
    #[test]
    fn nothing_is_written_outside_the_library() {
        let root = scratch("skill-contained");
        let home = root.join("home");
        std::fs::create_dir_all(home.join(".codex")).unwrap();

        ensure_library_docs(&root).unwrap();

        assert!(
            !home.join(".codex/skills").exists(),
            "reached into {}",
            home.join(".codex/skills").display()
        );
        assert!(agents_dir(&root).join(".agents/skills/oculus-plan").is_symlink());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The same rule the course folders' `AGENTS.md` follows: a link is this
    /// module's and gets repointed, a real file is somebody's and does not.
    /// A student who wrote their own `oculus-plan` keeps it.
    #[test]
    fn a_real_skill_on_the_link_path_is_left_alone() {
        let root = scratch("skill-mine");
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
        let _ = std::fs::remove_dir_all(&root);
    }
}
