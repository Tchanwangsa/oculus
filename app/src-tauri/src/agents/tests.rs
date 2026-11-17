use std::path::PathBuf;

use crate::test_support::Scratch;

use super::docs::SKILLS;
use super::links::SCANNED_SKILL_DIRS;
use super::*;

/// A hand-written AGENTS.md in a course folder must survive relinking.
#[cfg(unix)]
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
        std::fs::symlink_metadata(courses.join("fresh/.oculus/agents/memories"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        courses.join("fresh/.oculus/agents/memories").is_dir(),
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
    let old = root.join("courses/INFO30006_2026_SM2/.oculus/agents/memories");
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
    #[cfg(unix)]
    assert!(std::fs::symlink_metadata(&old)
        .unwrap()
        .file_type()
        .is_symlink());

    #[cfg(windows)]
    assert!(junction::exists(&old).unwrap());

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
            "oculus-plan",
            "oculus-ui",
            "oculus-revision",
            "oculus-widgets",
            "oculus-studio"
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

/// A skill is a folder: every file it ships lands, and anything else in the
/// app-owned folder goes.
#[test]
fn every_skill_file_lands_and_stale_ones_are_removed() {
    let root = Scratch::new("agents-skill-files");
    let studio = skills_dir(&root).join("oculus-studio");
    std::fs::create_dir_all(studio.join("old")).unwrap();
    std::fs::write(studio.join("old/notes.md"), "stale").unwrap();
    std::fs::write(studio.join("retired.md"), "stale").unwrap();
    std::os::unix::fs::symlink("/nowhere", studio.join("dangling")).unwrap();

    ensure_library_docs(&root).unwrap();

    for skill in SKILLS {
        let dir = skills_dir(&root).join(skill.name);
        let mut want: Vec<&str> = skill.files.iter().map(|(f, _)| *f).collect();
        let mut have: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        want.sort();
        have.sort();
        assert_eq!(have, want, "{}", skill.name);
        for (file, body) in skill.files {
            assert_eq!(
                std::fs::read_to_string(dir.join(file)).unwrap(),
                *body,
                "{}/{file}",
                skill.name
            );
        }
    }
    let studio_files: Vec<&str> = SKILLS
        .iter()
        .find(|s| s.name == "oculus-studio")
        .unwrap()
        .files
        .iter()
        .map(|(f, _)| *f)
        .collect();
    assert_eq!(
        studio_files,
        [SKILL_DOC_NAME, "reference.md", "build.md", "studio.d.ts"]
    );
}

/// The widgets skill's gallery, `(file, source)`.
fn widget_gallery() -> Vec<(&'static str, &'static str)> {
    let skill = SKILLS.iter().find(|s| s.name == "oculus-widgets").unwrap();
    let files: Vec<&str> = skill.files.iter().map(|(f, _)| *f).collect();
    assert_eq!(
        files,
        [
            SKILL_DOC_NAME,
            "widget.d.ts",
            "bloch-sphere.tsx",
            "derivative-steps.tsx",
            "bubble-sort.tsx",
            "vector-field.tsx"
        ]
    );
    skill
        .files
        .iter()
        .filter(|(f, _)| f.ends_with(".tsx"))
        .copied()
        .collect()
}

/// A gallery example is taught as a finished widget, so it passes the
/// check the skill tells the agent to run.
#[test]
fn every_widget_gallery_example_passes_the_check() {
    for (file, source) in widget_gallery() {
        assert_eq!(crate::widgets::check::check(source), Vec::new(), "{file}");
    }
}

/// `app/tests/fixtures/widget-gallery/<name>.js` is each example's real
/// transpile output, which the bun test builds a frame document from.
/// `WIDGET_BLESS=1 cargo test widget_gallery` rewrites them.
#[test]
fn widget_gallery_fixtures_are_the_transpile_output() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/widget-gallery");
    let bless = std::env::var_os("WIDGET_BLESS").is_some();
    let mut want = Vec::new();
    for (file, source) in widget_gallery() {
        let name = file.replace(".tsx", ".js");
        let js = crate::widgets::transpile::transpile(source).unwrap().js;
        let path = dir.join(&name);
        if bless {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, &js).unwrap();
        }
        let fixture = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            fixture == js,
            "{name} is not {file}'s transpile output; run WIDGET_BLESS=1 cargo test widget_gallery"
        );
        want.push(name);
    }
    let mut have: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    want.sort();
    have.sort();
    assert_eq!(have, want, "a fixture no example makes");
}

/// Each CLI indexes a skill by its `SKILL.md` front matter; the name must be
/// the folder's.
#[test]
fn every_skill_opens_with_its_own_name_and_a_description() {
    for skill in SKILLS {
        let (file, doc) = skill.files[0];
        assert_eq!(file, SKILL_DOC_NAME, "{}", skill.name);
        let front = doc
            .strip_prefix("---\n")
            .and_then(|d| d.split_once("\n---\n"))
            .map(|(f, _)| f)
            .unwrap_or_else(|| panic!("{}: no front matter", skill.name));
        assert!(
            front.lines().any(|l| l == format!("name: {}", skill.name)),
            "{}",
            skill.name
        );
        assert!(
            front
                .lines()
                .any(|l| l.starts_with("description: ") && l.len() > 40),
            "{}",
            skill.name
        );
    }
}

/// opencode finds skills through `skills.paths` in its config, relative to
/// the `agents/` cwd: every skill's folder must sit under one of them.
#[test]
fn opencodes_skill_paths_reach_every_skill() {
    let root = Scratch::new("agents-skill-opencode");
    ensure_library_docs(&root).unwrap();
    let raw = include_str!("../../templates/OPENCODE.template.json");
    let v: serde_json::Value = serde_json::from_str(raw).unwrap();
    let paths: Vec<&str> = v["skills"]["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    for skill in SKILLS {
        assert!(
            paths.iter().any(|p| agents_dir(&root)
                .join(p)
                .join(skill.name)
                .join(SKILL_DOC_NAME)
                .is_file()),
            "no skills.paths entry reaches {}",
            skill.name
        );
    }
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
        #[cfg(unix)]
        assert_eq!(
            std::fs::read_link(&link).unwrap(),
            PathBuf::from("../../skills/oculus-lectures"),
            "{scanned}: relative, so the library stays movable"
        );
        assert_eq!(
            std::fs::read_to_string(link.join(SKILL_DOC_NAME)).unwrap(),
            SKILLS[0].files[0].1,
            "{scanned}: and it resolves to the one copy"
        );
    }

    // Every skill, every file, through both links.
    for scanned in SCANNED_SKILL_DIRS {
        for skill in SKILLS {
            let link = agents_dir(&root)
                .join(scanned)
                .join("skills")
                .join(skill.name);
            for (file, body) in skill.files {
                assert_eq!(
                    std::fs::read_to_string(link.join(file)).unwrap(),
                    *body,
                    "{scanned}/skills/{}/{file}",
                    skill.name
                );
            }
        }
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
        .join(".agents/skills/oculus-plan/SKILL.md")
        .is_file());
}

/// A link is ours and gets repointed; a real file is somebody's.
#[cfg(unix)]
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

#[cfg(windows)]
#[test]
fn windows_course_docs_are_managed_copies_and_memories_are_junctions() {
    let root = Scratch::new("agents-windows-link");
    let course = root.join("courses/X");
    std::fs::create_dir_all(&course).unwrap();
    ensure_library_docs(&root).unwrap();
    assert_eq!(link_course(&root, "X").unwrap(), Link::Linked);
    assert!(course.join("AGENTS.md.oculus-managed").is_file());
    assert!(junction::exists(course.join("agents/memories")).unwrap());
    assert_eq!(link_course(&root, "X").unwrap(), Link::Current);
    let memories = course.join("agents/memories");
    let wrong = root.join("wrong-bucket");
    std::fs::create_dir_all(&wrong).unwrap();
    std::fs::write(wrong.join("keep.md"), "keep").unwrap();
    junction::delete(&memories).unwrap();
    std::fs::remove_dir(&memories).unwrap();
    junction::create(&wrong, &memories).unwrap();
    assert_eq!(link_course(&root, "X").unwrap(), Link::Current);
    assert_eq!(
        std::fs::read_to_string(wrong.join("keep.md")).unwrap(),
        "keep"
    );
    assert_eq!(
        crate::runtime::platform::canonicalize(junction::get_target(&memories).unwrap()).unwrap(),
        crate::runtime::platform::canonicalize(root.join("agents/memories/X")).unwrap()
    );
    std::fs::write(course.join("AGENTS.md"), "mine").unwrap();
    assert_eq!(link_course(&root, "X").unwrap(), Link::Skipped);
    assert_eq!(
        std::fs::read_to_string(course.join("AGENTS.md")).unwrap(),
        "mine"
    );
}

/// Every path the agent docs spell is the layout's: the folder tree names
/// each synced folder, the examples are what the layout module builds, and
/// no earlier layout's folder is left in them.
#[test]
fn the_agent_docs_spell_the_layout_they_describe() {
    use crate::library::layout::{
        self, derived_rel, in_course, recording_name, recording_rel, synced, synced_dir,
        synced_file, DerivedKind, SyncedFile, SyncedKind, CATEGORIES, LEGACY,
    };
    let harness = include_str!("../../templates/HARNESS.template.md");
    let skill = |name: &str| SKILLS.iter().find(|s| s.name == name).unwrap().files[0].1;
    let (library, lectures) = (skill("oculus-library"), skill("oculus-lectures"));

    for kind in [
        SyncedKind::Files,
        SyncedKind::Media,
        SyncedKind::Lessons,
        SyncedKind::Discussion,
        SyncedKind::Pages,
        SyncedKind::Assignments,
        SyncedKind::Quizzes,
        SyncedKind::Announcements,
        SyncedKind::Modules,
        SyncedKind::Images,
    ] {
        let dir = synced_dir(kind);
        let name = dir.rsplit('/').next().unwrap();
        assert!(
            AGENTS_DOC.contains(&format!("{name}/")),
            "AGENTS.md lacks {dir}"
        );
    }
    for file in [SyncedFile::Home, SyncedFile::Syllabus] {
        let rel = synced_file(file);
        assert!(AGENTS_DOC.contains(&rel), "AGENTS.md lacks {rel}");
    }
    for name in layout::reserved_names() {
        assert!(
            AGENTS_DOC.contains(&format!("{name}/")),
            "AGENTS.md lacks {name}/"
        );
    }
    let instructions = format!(
        "{}/INSTRUCTIONS.md",
        layout::agents_rel("X").strip_prefix("courses/X/").unwrap()
    );
    assert!(AGENTS_DOC.contains(&instructions), "{instructions}");

    // A PDF's text, where the library skill says it is.
    let pdf = synced(SyncedKind::Files, "13.pdf");
    let md = derived_rel(&pdf, DerivedKind::Markdown);
    assert!(library.contains(&format!("`{md}`")), "{md}");
    // A citation as the brief spells it.
    let cited = in_course("X", &synced(SyncedKind::Files, "week-3.pdf")).replace("/X/", "/<CODE>/");
    assert!(harness.contains(&cited), "{cited}");
    // A recording folder as the lectures skill spells it.
    let code = "MULT20015_2026_SM2";
    let folder = recording_rel(
        code,
        &recording_name("2026-09-08", "MULT20015_2026_SM2 MO L105", code),
    );
    assert!(
        lectures.contains(&format!("../{folder}/outline.md")),
        "{folder}"
    );
    for category in CATEGORIES {
        assert!(
            library.contains(&format!("`{category}`")),
            "the library skill lacks {category}"
        );
    }

    let docs = [
        ("AGENTS.md", AGENTS_DOC),
        ("the brief", harness),
        ("oculus-library", library),
        ("oculus-lectures", lectures),
    ];
    let legacy_own = LEGACY.own_dirs.iter().map(|(_, dir)| format!("{dir}/"));
    let legacy_synced = LEGACY.synced_dirs.iter().map(|(_, dir)| format!("/{dir}/"));
    let legacy_recordings = LEGACY.recordings_root().map(|r| format!("../{r}/"));
    for old in legacy_own.chain(legacy_synced).chain(legacy_recordings) {
        for (name, text) in docs {
            assert!(!text.contains(&old), "{name} still spells {old}");
        }
    }
}
