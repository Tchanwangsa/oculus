use std::path::Path;

use super::links::link_agent_skills;
use super::{
    agents_dir, refresh_taste, skills_dir, Refresh, AGENTS_DOC_NAME, MEMORIES_DIR,
    MEMORY_INDEX_NAME, SKILL_DOC_NAME, TASTE_DOC_NAME,
};

/// One `AGENTS.md` for every subject. Per-subject additions go in that folder's
/// `agents/INSTRUCTIONS.md`, which nothing here writes.
pub const AGENTS_DOC: &str = include_str!("../../templates/AGENTS.template.md");
const OCULUS_DOC: &str = include_str!("../../templates/OCULUS.template.md");
pub(super) const TASTE_DOC: &str = include_str!("../../templates/TASTE.template.md");

const MEMORY_INDEX_DOC: &str = include_str!("../../templates/MEMORY.template.md");

/// One skill: its folder under `agents/skills/` and every file in it,
/// `SKILL.md` first.
pub(super) struct Skill {
    pub name: &'static str,
    pub files: &'static [(&'static str, &'static str)],
}

/// The procedures an agent loads by name. Kept short: every CLI reads the
/// skill index into the first prompt of every thread, headless jobs included.
pub(super) const SKILLS: &[Skill] = &[
    Skill {
        name: "oculus-lectures",
        files: &[(
            SKILL_DOC_NAME,
            include_str!("../../templates/skills/oculus-lectures/SKILL.md"),
        )],
    },
    Skill {
        name: "oculus-library",
        files: &[(
            SKILL_DOC_NAME,
            include_str!("../../templates/skills/oculus-library/SKILL.md"),
        )],
    },
    Skill {
        name: "oculus-plan",
        files: &[(
            SKILL_DOC_NAME,
            include_str!("../../templates/skills/oculus-plan/SKILL.md"),
        )],
    },
    Skill {
        name: "oculus-ui",
        files: &[(
            SKILL_DOC_NAME,
            include_str!("../../templates/skills/oculus-ui/SKILL.md"),
        )],
    },
    Skill {
        name: "oculus-studio",
        files: &[
            (
                SKILL_DOC_NAME,
                include_str!("../../templates/skills/oculus-studio/SKILL.md"),
            ),
            (
                "reference.md",
                include_str!("../../templates/skills/oculus-studio/reference.md"),
            ),
            // Also the scene helper's role prompt (`harness::studio_agents`).
            (
                "build.md",
                include_str!("../../templates/skills/oculus-studio/build.md"),
            ),
            // Beside the prelude it types, whose test keeps the two equal.
            (
                "studio.d.ts",
                include_str!("../../../studio-core/studio.d.ts"),
            ),
        ],
    },
];

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
    std::fs::write(&path, AGENTS_DOC)
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
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
    for skill in SKILLS {
        write_skill(&skills.join(skill.name), skill)?;
        docs.generated.push(skill.name);
    }
    link_agent_skills(data_dir)?;
    Ok(docs)
}

/// Writes every file of `skill` into `dir` and removes anything else there:
/// the folder is app-owned (agents may not write it), so a file a skill no
/// longer ships would only be read as a stale instruction.
pub(super) fn write_skill(dir: &Path, skill: &Skill) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    for (file, body) in skill.files {
        let path = dir.join(file);
        std::fs::write(&path, body).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if skill.files.iter().any(|(file, _)| name == *file) {
            continue;
        }
        let path = entry.path();
        let is_dir = std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir());
        let removed = if is_dir {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        removed.map_err(|e| format!("cannot remove stale {}: {e}", path.display()))?;
    }
    Ok(())
}
