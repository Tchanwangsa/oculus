use std::path::PathBuf;

use crate::test_support::Scratch;

use super::docs::SKILLS;
use super::links::SCANNED_SKILL_DIRS;
use super::*;

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
    assert!(
        std::fs::symlink_metadata(courses.join("fresh/agents/memories"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        courses.join("fresh/agents/memories").is_dir(),
        "the link resolves"
    );
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
    std::fs::write(
        old.join("MEMORY.md"),
        "# Memories — INFO30006\n\n- [MST](info30006-mst.md)",
    )
    .unwrap();

    link_all(&root).unwrap();

    let bucket = root.join("agents/memories/INFO30006_2026_SM2");
    assert_eq!(
        std::fs::read_to_string(bucket.join("info30006-mst.md")).unwrap(),
        "the MST is week 7"
    );
    // The course's own index came across, so the stub never overwrote it.
    assert!(std::fs::read_to_string(bucket.join("MEMORY.md"))
        .unwrap()
        .contains("[MST]"));
    // And the old path resolves to the same file.
    assert!(old.join("info30006-mst.md").is_file());
    assert!(std::fs::symlink_metadata(&old)
        .unwrap()
        .file_type()
        .is_symlink());

    // Idempotent: a second sync neither re-moves nor re-links.
    link_all(&root).unwrap();
    assert!(old.join("info30006-mst.md").is_file());
}

/// A newly scraped subject, and one that produced no folder.
#[test]
fn a_sync_links_scraped_subjects_and_invents_no_folders() {
    let root = Scratch::new("agents-course");
    std::fs::create_dir_all(root.join("courses/COMP30026_2026_SM2")).unwrap();

    assert_eq!(
        link_course(&root, "COMP30026_2026_SM2").unwrap(),
        Link::Linked
    );
    assert_eq!(
        link_course(&root, "COMP30026_2026_SM2").unwrap(),
        Link::Current
    );

    // A scrape that wrote nothing must not leave an empty course folder.
    assert_eq!(
        link_course(&root, "NEW10001_2026_SM2").unwrap(),
        Link::Absent
    );
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
    assert_eq!(
        first.created,
        ["OCULUS.md", "TASTE.md", "memories/MEMORY.md"]
    );

    std::fs::write(agents_dir(&root).join("TASTE.md"), "my notes").unwrap();
    let second = ensure_library_docs(&root).unwrap();
    assert!(second.created.is_empty());
    assert_eq!(
        second.generated,
        [
            AGENTS_DOC_NAME,
            "oculus-lectures",
            "oculus-library",
            "oculus-plan"
        ]
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
    assert!(std::fs::read_to_string(&plan)
        .unwrap()
        .contains("name: oculus-plan"));

    std::fs::write(&plan, "do whatever you like").unwrap();
    ensure_library_docs(&root).unwrap();
    assert!(std::fs::read_to_string(&plan)
        .unwrap()
        .contains("name: oculus-plan"));
}

/// Three CLIs, three discovery paths, one directory.
#[test]
fn all_three_clis_reach_the_one_skills_directory() {
    let root = Scratch::new("agents-skill-links");
    ensure_library_docs(&root).unwrap();

    // Both scan up from the cwd, which is `agents/` for every thread and job.
    for scanned in SCANNED_SKILL_DIRS {
        let link = agents_dir(&root)
            .join(scanned)
            .join("skills/oculus-lectures");
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

    // opencode takes a config key instead (`harness/providers/opencode/config.rs`).
    assert!(skills_dir(&root)
        .join("oculus-lectures")
        .join(SKILL_DOC_NAME)
        .is_file());

    // Idempotent.
    ensure_library_docs(&root).unwrap();
    for scanned in SCANNED_SKILL_DIRS {
        let link = agents_dir(&root)
            .join(scanned)
            .join("skills/oculus-lectures");
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
    assert!(agents_dir(&root)
        .join(".agents/skills/oculus-plan")
        .is_symlink());
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

    assert_eq!(
        std::fs::read_to_string(into.join("oculus-plan")).unwrap(),
        "mine"
    );
    assert_eq!(
        std::fs::read_link(into.join("oculus-lectures")).unwrap(),
        PathBuf::from("../../skills/oculus-lectures")
    );
}
