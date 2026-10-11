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

/// The procedures an agent loads by name. Kept short: every CLI reads the
/// skill index into the first prompt of every thread, headless jobs included.
pub(super) const SKILLS: [(&str, &str); 3] = [
    (
        "oculus-lectures",
        include_str!("../../templates/skills/oculus-lectures/SKILL.md"),
    ),
    (
        "oculus-library",
        include_str!("../../templates/skills/oculus-library/SKILL.md"),
    ),
    (
        "oculus-plan",
        include_str!("../../templates/skills/oculus-plan/SKILL.md"),
    ),
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
    for (name, body) in SKILLS {
        let dir = skills.join(name);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        let path = dir.join(SKILL_DOC_NAME);
        std::fs::write(&path, body).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        docs.generated.push(name);
    }
    link_agent_skills(data_dir)?;
    Ok(docs)
}
