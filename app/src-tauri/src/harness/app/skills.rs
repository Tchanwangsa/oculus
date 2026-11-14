//! The skills the composer's `/` menu offers, and how a picked one reaches
//! the agent.
//!
//! [`harness_skills`] scans four folders: the library's own
//! (`agents/.claude/skills`, `agents/.agents/skills`, both links to
//! `agents/skills/` on Unix) and the student's (`~/.claude/skills`,
//! `~/.agents/skills`). Each skill is a folder holding a `SKILL.md` whose
//! frontmatter names it. Skills are deduplicated by resolved path, then by
//! name, the library's first. Nothing here touches the network or a model.
//!
//! A pick rides `SendOptions::skills`; [`name_picked_skills`] puts each
//! bridge's `invoke_skill` line before the prompt, never in the stored row.
//! See docs/harness.md, "`/` finds commands and skills".

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::harness::providers::{antigravity, claude, codex, opencode};
use crate::harness::Provider;

const SKILL_DOC: &str = "SKILL.md";

/// Which badge a skill wears: the library's own, or the student's.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SkillSource {
    Oculus,
    Personal,
}

/// One installed skill, as its `SKILL.md` names it.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// The `SKILL.md`, resolved through any links.
    pub path: PathBuf,
    pub source: SkillSource,
    /// False when the frontmatter says `user-invocable: false`.
    pub user_invocable: bool,
}

/// The folders scanned, in the order their skills win a name.
pub fn roots(data_dir: &Path, home: Option<&Path>) -> Vec<(PathBuf, SkillSource)> {
    let agents = crate::agents::agents_dir(data_dir);
    let mut out = vec![
        (agents.join(".claude").join("skills"), SkillSource::Oculus),
        (agents.join(".agents").join("skills"), SkillSource::Oculus),
    ];
    if let Some(home) = home {
        out.push((home.join(".claude").join("skills"), SkillSource::Personal));
        out.push((home.join(".agents").join("skills"), SkillSource::Personal));
    }
    out
}

/// Every skill under `roots`. A missing folder is no error; a skill whose
/// `SKILL.md` is unreadable or whose frontmatter does not parse is skipped
/// alone.
pub fn scan(roots: &[(PathBuf, SkillSource)]) -> Vec<Skill> {
    let mut seen_paths = HashSet::new();
    let mut seen_names = HashSet::new();
    let mut out = Vec::new();
    for (root, source) in roots {
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                elog!("[oculus] skills: {}: {e}", root.display());
                continue;
            }
        };
        let mut dirs: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        dirs.sort();
        for dir in dirs {
            // A folder without a SKILL.md is not a skill; a link resolves here.
            let Ok(doc) = std::fs::canonicalize(dir.join(SKILL_DOC)) else {
                continue;
            };
            if !seen_paths.insert(doc.clone()) {
                continue;
            }
            let Some(skill) = read_skill(&dir, doc, *source) else {
                continue;
            };
            if seen_names.insert(skill.name.clone()) {
                out.push(skill);
            }
        }
    }
    out
}

fn read_skill(dir: &Path, doc: PathBuf, source: SkillSource) -> Option<Skill> {
    let text = match std::fs::read_to_string(&doc) {
        Ok(text) => text,
        Err(e) => {
            elog!("[oculus] skills: {}: {e}", doc.display());
            return None;
        }
    };
    let Some(front) = frontmatter(&text) else {
        elog!(
            "[oculus] skills: {}: no readable frontmatter",
            doc.display()
        );
        return None;
    };
    let folder = dir.file_name()?.to_string_lossy().into_owned();
    let name = front
        .get("name")
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or(folder);
    if !is_skill_name(&name) {
        elog!("[oculus] skills: {}: unusable name {name:?}", doc.display());
        return None;
    }
    let description = front
        .get("description")
        .map(|d| d.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    let user_invocable = front
        .get("user-invocable")
        .is_none_or(|v| !v.trim().eq_ignore_ascii_case("false"));
    Some(Skill {
        name,
        description,
        path: doc,
        source,
        user_invocable,
    })
}

/// What a `/name` word in the composer can hold (`splitSkillTokens` in
/// `app/src/lib/harness/skills.ts`), so a picked chip reads back as itself.
fn is_skill_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
}

/// The `key: value` pairs between a leading `---` and the next `---`. A
/// value may continue on indented lines (joined by a space) and may be
/// quoted, or a `>`/`|` block. None when there is no closed block or a line
/// is neither a pair, a continuation, a comment nor blank.
fn frontmatter(text: &str) -> Option<HashMap<String, String>> {
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    let mut out: HashMap<String, String> = HashMap::new();
    let mut current: Option<String> = None;
    for line in lines {
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed == "..." {
            return Some(out);
        }
        if trimmed.trim_start().is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if line.starts_with([' ', '\t']) {
            let key = current.as_ref()?;
            let value = out.entry(key.clone()).or_default();
            if !value.is_empty() {
                value.push(' ');
            }
            value.push_str(trimmed.trim());
            continue;
        }
        let (key, value) = trimmed.split_once(':')?;
        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return None;
        }
        let value = value.trim();
        let value = match value {
            ">" | ">-" | "|" | "|-" | ">+" | "|+" => "",
            _ => unquote(value),
        };
        out.insert(key.to_string(), value.to_string());
        current = Some(key.to_string());
    }
    None
}

fn unquote(value: &str) -> &str {
    for q in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(q) && value.ends_with(q) {
            return &value[1..value.len() - 1];
        }
    }
    value
}

/// Every skill the composer may offer, from the library at `data_dir()` and
/// the student's home.
pub fn installed() -> Vec<Skill> {
    let data = crate::library::paths::data_dir();
    scan(&roots(&data, dirs::home_dir().as_deref()))
}

#[tauri::command]
pub async fn harness_skills() -> Result<Vec<Skill>, String> {
    crate::runtime::blocking::run(|| Ok(installed())).await
}

/// The sentence every bridge understands: "Use the {name} skill.", with the
/// `SKILL.md` path, so a bridge that does not index a Personal skill can
/// still read it. Each bridge's `invoke_skill` returns this until a recorded
/// turn shows a native form that bridge acts on.
pub fn fallback_sentence(name: &str, path: &Path) -> String {
    format!(
        "Use the {name} skill. Its instructions are in `{}`.",
        path.display()
    )
}

/// One provider's line for a picked skill.
fn invoke_skill(provider: Provider, name: &str, path: &Path) -> String {
    match provider {
        Provider::Claude => claude::invoke_skill(name, path),
        Provider::Codex => codex::invoke_skill(name, path),
        Provider::Opencode => opencode::invoke_skill(name, path),
        Provider::Antigravity => antigravity::invoke_skill(name, path),
    }
}

/// `prompt` with a line per picked skill before it: each name the student
/// picked as a chip, still standing as a `/name` word in the prompt, and in
/// `catalogue`. Nothing picked leaves the prompt as it is, so a typed `/word`
/// arrives as text.
pub fn with_picked_skills(
    provider: Provider,
    picked: &[String],
    catalogue: &[Skill],
    prompt: String,
) -> String {
    let mut lines = Vec::new();
    let mut named = HashSet::new();
    for name in picked {
        if !named.insert(name.as_str()) || !has_word(&prompt, &format!("/{name}")) {
            continue;
        }
        if let Some(skill) = catalogue.iter().find(|s| &s.name == name) {
            lines.push(invoke_skill(provider, &skill.name, &skill.path));
        }
    }
    if lines.is_empty() {
        return prompt;
    }
    format!("{}\n\n{prompt}", lines.join("\n"))
}

/// The send path's one call: [`with_picked_skills`] against the installed
/// catalogue, read only when something was picked.
pub(in crate::harness) fn name_picked_skills(
    provider: Provider,
    picked: &[String],
    prompt: String,
) -> String {
    if picked.is_empty() {
        return prompt;
    }
    with_picked_skills(provider, picked, &installed(), prompt)
}

/// Whether `word` stands in `text` with whitespace or an edge on both sides.
fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + word.len()..].chars().next();
        before.is_none_or(char::is_whitespace) && after.is_none_or(char::is_whitespace)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("harness-skills-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_skill(root: &Path, folder: &str, doc: &str) {
        let dir = root.join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(SKILL_DOC), doc).unwrap();
    }

    fn doc(name: &str, description: &str) -> String {
        format!("---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n")
    }

    #[cfg(unix)]
    #[test]
    fn the_library_and_home_folders_scan_into_one_entry_per_skill() {
        let root = scratch("scan");
        let data = root.join("data");
        let home = root.join("home");
        let skills = crate::agents::agents_dir(&data).join("skills");
        write_skill(
            &skills,
            "oculus-plan",
            &doc("oculus-plan", "Proposes tasks."),
        );
        write_skill(&skills, "notes", &doc("notes", "Ours."));
        // Both CLIs' folders are links to the one copy, as `agents::docs` makes them.
        for cli in [".claude", ".agents"] {
            let dir = crate::agents::agents_dir(&data).join(cli);
            std::fs::create_dir_all(&dir).unwrap();
            std::os::unix::fs::symlink("../skills", dir.join("skills")).unwrap();
        }
        let personal = home.join(".claude").join("skills");
        write_skill(&personal, "docx", &doc("docx", "Word files."));
        write_skill(&personal, "notes", &doc("notes", "Mine, shadowed by ours."));
        write_skill(
            &home.join(".agents").join("skills"),
            "quiet",
            "---\nname: quiet\ndescription: >-\n  Folded\n  over lines.\nuser-invocable: false\n---\n",
        );

        let found = scan(&roots(&data, Some(&home)));
        let summary: Vec<_> = found
            .iter()
            .map(|s| {
                (
                    s.name.as_str(),
                    s.source,
                    s.description.as_str(),
                    s.user_invocable,
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("notes", SkillSource::Oculus, "Ours.", true),
                ("oculus-plan", SkillSource::Oculus, "Proposes tasks.", true),
                ("docx", SkillSource::Personal, "Word files.", true),
                ("quiet", SkillSource::Personal, "Folded over lines.", false),
            ]
        );
        let plan = &found[1];
        assert_eq!(
            plan.path,
            std::fs::canonicalize(skills.join("oculus-plan").join(SKILL_DOC)).unwrap()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_home_folder_is_no_error() {
        let root = scratch("missing");
        let data = root.join("data");
        write_skill(
            &crate::agents::agents_dir(&data)
                .join(".claude")
                .join("skills"),
            "only",
            &doc("only", "The one."),
        );
        let found = scan(&roots(&data, Some(&root.join("no-such-home"))));
        assert_eq!(
            found.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["only"]
        );
        assert!(scan(&roots(&root.join("no-such-library"), None)).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_bad_frontmatter_skips_only_that_skill() {
        let root = scratch("bad");
        let home = root.join("home");
        let dir = home.join(".claude").join("skills");
        write_skill(&dir, "a-good", &doc("a-good", "Fine."));
        write_skill(
            &dir,
            "b-unclosed",
            "---\nname: b-unclosed\ndescription: never closed\n",
        );
        write_skill(&dir, "c-none", "# No frontmatter at all\n");
        write_skill(
            &dir,
            "d-junk",
            "---\nname: d-junk\nthis line is not a pair\n---\n",
        );
        write_skill(&dir, "e-spaced", "---\nname: has a space\n---\n");
        write_skill(
            &dir,
            "f-unnamed",
            "---\ndescription: \"Named by its folder.\"\n---\n",
        );
        std::fs::create_dir_all(dir.join("g-no-doc")).unwrap();
        let found = scan(&roots(&root.join("data"), Some(&home)));
        let names: Vec<_> = found
            .iter()
            .map(|s| (s.name.as_str(), s.description.as_str()))
            .collect();
        assert_eq!(
            names,
            vec![("a-good", "Fine."), ("f-unnamed", "Named by its folder.")]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    fn catalogue() -> Vec<Skill> {
        vec![Skill {
            name: "oculus-plan".into(),
            description: String::new(),
            path: PathBuf::from("/lib/agents/skills/oculus-plan/SKILL.md"),
            source: SkillSource::Oculus,
            user_invocable: true,
        }]
    }

    const PROVIDERS: [Provider; 4] = [
        Provider::Claude,
        Provider::Codex,
        Provider::Opencode,
        Provider::Antigravity,
    ];

    #[test]
    fn every_bridge_names_a_picked_skill_before_the_prompt() {
        let sentence = "Use the oculus-plan skill. Its instructions are in `/lib/agents/skills/oculus-plan/SKILL.md`.";
        for provider in PROVIDERS {
            let prompt = with_picked_skills(
                provider,
                &["oculus-plan".into()],
                &catalogue(),
                "/oculus-plan week 3".into(),
            );
            assert_eq!(
                prompt,
                format!("{sentence}\n\n/oculus-plan week 3"),
                "{provider:?}"
            );
        }
    }

    #[test]
    fn a_typed_slash_word_arrives_unchanged_on_every_bridge() {
        for provider in PROVIDERS {
            let typed = "/oculus-plan is what I typed, not picked";
            assert_eq!(
                with_picked_skills(provider, &[], &catalogue(), typed.into()),
                typed
            );
            assert_eq!(name_picked_skills(provider, &[], typed.into()), typed);
        }
    }

    #[test]
    fn a_pick_no_longer_in_the_text_or_the_catalogue_names_nothing() {
        let picked = [
            "oculus-plan".to_string(),
            "gone".to_string(),
            "oculus-plan".to_string(),
        ];
        let edited = "I removed it from /oculus-planning";
        assert_eq!(
            with_picked_skills(Provider::Codex, &picked, &catalogue(), edited.into()),
            edited
        );
        let kept = with_picked_skills(
            Provider::Codex,
            &picked,
            &catalogue(),
            "/oculus-plan /gone".into(),
        );
        assert_eq!(kept.matches("Use the").count(), 1);
    }
}
