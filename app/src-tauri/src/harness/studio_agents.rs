//! The two subagents a chat agent may start while it makes a Studio video:
//! `studio-research` reads one slice of the source, `studio-scene` builds,
//! fixes or reviews one scene. Their models come from Settings → Jobs
//! (`Job::StudioResearch`, `Job::StudioScene`), one entry per agent.
//!
//! Each CLI starts a subagent its own way, so each gets the model its own
//! way: Claude and opencode through named agents written at spawn (Claude's
//! `--agents`, opencode's config), Codex through `spawn_agent`'s `model`,
//! which the skill asks for by the words [`how_to_spawn`] prints. agy's
//! `invoke_subagent` takes no model, so agy does the work itself. The scene
//! prompt carries the skill's `build.md` and the builder's sections of
//! `reference.md`; Codex, with no role prompt, is told to read the same by
//! [`codex_scene_opening`]. docs/harness.md ("Subagents") has the rest.

use std::sync::LazyLock;

use serde_json::{json, Map, Value};

use super::jobs::{self, Job, JobSelection};
use super::Provider;

/// One subagent: its name in every CLI, the job holding its models, and its
/// system prompt.
pub struct Role {
    pub name: &'static str,
    pub job: Job,
    pub description: &'static str,
    pub prompt: fn() -> &'static str,
}

impl Role {
    pub fn prompt(&self) -> &'static str {
        (self.prompt)()
    }
}

pub const ROLES: [Role; 2] = [
    Role {
        name: "studio-research",
        job: Job::StudioResearch,
        description: "Reads one slice of a lecture for an Oculus Studio video and writes notes",
        prompt: || include_str!("../../templates/STUDIO_RESEARCH.template.md"),
    },
    Role {
        name: "studio-scene",
        job: Job::StudioScene,
        description: "Builds, fixes or reviews one scene of an Oculus Studio video",
        prompt: scene_prompt,
    },
];

const SCENE_ROLE: &str = include_str!("../../templates/STUDIO_SCENE.template.md");
const BUILD: &str = include_str!("../../templates/skills/oculus-studio/build.md");
const REFERENCE: &str = include_str!("../../templates/skills/oculus-studio/reference.md");

/// The `reference.md` sections a scene helper works from, in the file's
/// order. The director's own (video.json, script.md, Narrate,
/// Pronunciation, Render, Libraries) stay out; Slides is in because a
/// builder makes the crop its scene shows.
pub const BUILDER_SECTIONS: [&str; 11] = [
    "Components",
    "Elements",
    "The theme",
    "Motion",
    "Maths",
    "Moving maths",
    "Word cues",
    "Transitions and magic move",
    "Lint",
    "Looking",
    "Slides",
];

/// The scene role's prompt: the role, `build.md`, then [`BUILDER_SECTIONS`].
fn scene_prompt() -> &'static str {
    static PROMPT: LazyLock<String> = LazyLock::new(|| {
        let mut out = format!(
            "{}\n\n{}\n\n# `reference.md`: the builder's sections\n",
            SCENE_ROLE.trim_end(),
            BUILD.trim_end()
        );
        for name in BUILDER_SECTIONS {
            let body = section(REFERENCE, name)
                .unwrap_or_else(|| panic!("reference.md has no `## {name}`"));
            out.push('\n');
            out.push_str(body.trim_end());
            out.push('\n');
        }
        out
    });
    &PROMPT
}

/// `doc`'s `## name` section, heading included, up to the next `## `
/// heading; headings inside code fences don't count.
fn section<'a>(doc: &'a str, name: &str) -> Option<&'a str> {
    let heading = format!("## {name}");
    let mut fenced = false;
    let mut start = None;
    let mut at = 0;
    for line in doc.split_inclusive('\n') {
        let text = line.trim_end();
        if text.starts_with("```") {
            fenced = !fenced;
        } else if !fenced && text.starts_with("## ") {
            if let Some(s) = start {
                return Some(&doc[s..at]);
            }
            if text == heading {
                start = Some(at);
            }
        }
        at += line.len();
    }
    start.map(|s| &doc[s..])
}

/// What a Codex director's scene brief opens with, since Codex gives a
/// subagent no role prompt; `SKILL.md` repeats it word for word.
pub fn codex_scene_opening() -> String {
    let quoted: Vec<String> = BUILDER_SECTIONS
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect();
    let (last, rest) = quoted.split_last().expect("sections");
    format!(
        "Read `skills/oculus-studio/build.md`, then these sections of `skills/oculus-studio/reference.md`: {} and {last}.",
        rest.join(", ")
    )
}

/// Whether the agent's subagents can be given a model at all.
pub fn takes_model(provider: Provider) -> bool {
    provider != Provider::Antigravity
}

/// Each role with the model `provider`'s subagents run it on; `None` is off
/// for that agent. `chain` reads a job's list (`jobs::chain_blocking`, or a
/// fixed one in tests).
pub fn assigned(
    provider: Provider,
    chain: impl Fn(Job) -> Vec<JobSelection>,
) -> Vec<(&'static Role, Option<JobSelection>)> {
    ROLES
        .iter()
        .map(|role| {
            let sel = takes_model(provider)
                .then(|| jobs::for_agent(&chain(role.job), provider).cloned())
                .flatten();
            (role, sel)
        })
        .collect()
}

/// [`assigned`] from the stored jobs.
pub fn assigned_now(provider: Provider) -> Vec<(&'static Role, Option<JobSelection>)> {
    assigned(provider, jobs::chain_blocking)
}

/// Claude's `--agents` value: one definition per role that has a model, or
/// `None` when neither has. A definition takes a full model id and an effort;
/// subagents inherit the thread's permission mode and sandbox.
pub fn claude_agents_json(roles: &[(&'static Role, Option<JobSelection>)]) -> Option<String> {
    let mut out = Map::new();
    for (role, sel) in roles {
        let Some(sel) = sel else { continue };
        let mut def = json!({
            "description": role.description,
            "prompt": role.prompt(),
            "model": sel.model,
        });
        if let Some(e) = sel.effort() {
            def["effort"] = json!(e);
        }
        out.insert(role.name.to_string(), def);
    }
    (!out.is_empty()).then(|| Value::Object(out).to_string())
}

/// opencode's agent entries, as `"name": {…},` lines for the config's
/// `agent` object. No `permission` key: a subagent session takes its
/// parent's, and opencode denies `task` inside it unless the agent allows it.
pub fn opencode_agents_fragment(roles: &[(&'static Role, Option<JobSelection>)]) -> String {
    let mut out = String::new();
    for (role, sel) in roles {
        let Some(sel) = sel else { continue };
        let mut def = json!({
            "mode": "subagent",
            "description": role.description,
            "prompt": role.prompt(),
            "model": sel.model,
        });
        if let Some(e) = sel.effort() {
            def["variant"] = json!(e);
        }
        out.push_str(&format!("{}: {},\n    ", json!(role.name), def));
    }
    out
}

/// What the skill tells `provider`'s agent to call to start `role` on `sel`.
pub fn how_to_spawn(provider: Provider, role: &Role, sel: Option<&JobSelection>) -> String {
    let Some(sel) = sel else {
        return if takes_model(provider) {
            format!(
                "Off for this agent (no model in Settings → Jobs → {}): do these items yourself, one after another.",
                role.job.label()
            )
        } else {
            "agy's invoke_subagent takes no model, so Studio starts no subagents on agy: do these items yourself, one after another.".to_string()
        };
    };
    match provider {
        Provider::Claude => format!(
            "The Agent tool with subagent_type \"{}\" and run_in_background false. Several Agent calls in one message run at once. Its model and effort were fixed when this chat started.",
            role.name
        ),
        Provider::Opencode => format!(
            "The task tool with subagent_type \"{}\". Several task calls in one message run at once. Its model was fixed when opencode's server started.",
            role.name
        ),
        Provider::Codex => {
            let effort = sel
                .effort()
                .map(|e| format!(", reasoning_effort \"{e}\""))
                .unwrap_or_default();
            let opening = match role.job {
                Job::StudioScene => format!(
                    " Codex gives it no role prompt, so begin each brief, a builder's or a reviewer's, with this sentence: {}",
                    codex_scene_opening()
                ),
                _ => String::new(),
            };
            format!(
                "spawn_agent with model \"{}\"{effort}, fork_turns \"none\", a task_name of lowercase letters, digits and underscores, and the brief as message. At most 3 at once; collect each with wait_agent, then close_agent it, since a finished agent holds its slot until closed.{opening}",
                sel.model
            )
        }
        Provider::Antigravity => unreachable!("agy takes no model"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(provider: Provider, model: &str, effort: Option<&str>) -> JobSelection {
        JobSelection {
            provider,
            model: model.into(),
            reasoning_effort: effort.map(Into::into),
        }
    }

    fn chain(job: Job) -> Vec<JobSelection> {
        match job {
            Job::StudioResearch => vec![
                sel(Provider::Opencode, "openrouter/small", None),
                sel(Provider::Claude, "claude-haiku-4-5-20251001", None),
            ],
            Job::StudioScene => vec![
                sel(Provider::Claude, "claude-opus-5-5", Some("high")),
                sel(Provider::Codex, "gpt-6.1-sol", Some("high")),
            ],
            _ => Vec::new(),
        }
    }

    #[test]
    fn each_agent_takes_its_own_entry_and_agy_none() {
        let names = |p| -> Vec<Option<String>> {
            assigned(p, chain)
                .into_iter()
                .map(|(_, s)| s.map(|s| s.model))
                .collect()
        };
        assert_eq!(
            names(Provider::Claude),
            [
                Some("claude-haiku-4-5-20251001".into()),
                Some("claude-opus-5-5".into())
            ]
        );
        assert_eq!(
            names(Provider::Opencode),
            [Some("openrouter/small".into()), None]
        );
        assert_eq!(names(Provider::Codex), [None, Some("gpt-6.1-sol".into())]);
        assert_eq!(names(Provider::Antigravity), [None, None]);
    }

    #[test]
    fn claudes_agents_carry_the_exact_model_and_effort() {
        let json = claude_agents_json(&assigned(Provider::Claude, chain)).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["studio-research"]["model"], "claude-haiku-4-5-20251001");
        assert!(v["studio-research"].get("effort").is_none());
        assert_eq!(v["studio-scene"]["model"], "claude-opus-5-5");
        assert_eq!(v["studio-scene"]["effort"], "high");
        assert_eq!(v["studio-scene"]["prompt"], scene_prompt());
        // Nothing to define: no flag at all.
        assert!(claude_agents_json(&assigned(Provider::Claude, |_| Vec::new())).is_none());
    }

    #[test]
    fn opencodes_fragment_is_subagents_with_no_wider_permission() {
        let frag = opencode_agents_fragment(&assigned(Provider::Opencode, chain));
        let v: Value = serde_json::from_str(&format!("{{{}\"end\": null}}", frag)).unwrap();
        assert_eq!(v["studio-research"]["mode"], "subagent");
        assert_eq!(v["studio-research"]["model"], "openrouter/small");
        assert!(v["studio-research"].get("permission").is_none());
        assert!(v.get("studio-scene").is_none(), "no model, no agent");
    }

    #[test]
    fn the_wording_names_each_clis_own_call() {
        let roles = assigned(Provider::Codex, chain);
        let codex = how_to_spawn(Provider::Codex, roles[1].0, roles[1].1.as_ref());
        assert!(codex.contains("model \"gpt-6.1-sol\", reasoning_effort \"high\""));
        assert!(codex.contains("fork_turns \"none\""));
        let off = how_to_spawn(Provider::Codex, roles[0].0, None);
        assert!(off.contains("Studio research"), "{off}");
        let claude = how_to_spawn(Provider::Claude, roles[1].0, roles[1].1.as_ref());
        assert!(claude.contains("subagent_type \"studio-scene\""));
        assert!(claude.contains("run_in_background false"));
        let agy = how_to_spawn(Provider::Antigravity, roles[0].0, None);
        assert!(agy.contains("yourself"));
    }

    /// The scene prompt is the role, then build.md whole, then each builder
    /// section of reference.md from its heading on, and no director section.
    #[test]
    fn the_scene_prompt_carries_build_md_and_the_builder_sections() {
        let prompt = scene_prompt();
        assert!(prompt.starts_with(SCENE_ROLE.trim_end()));
        assert!(prompt.contains(BUILD.trim_end()));
        let mut last = 0;
        for name in BUILDER_SECTIONS {
            let heading = format!("\n## {name}\n");
            assert_eq!(REFERENCE.matches(&heading).count(), 1, "{name}");
            let first = section(REFERENCE, name)
                .unwrap()
                .lines()
                .skip(1)
                .find(|l| !l.trim().is_empty())
                .unwrap();
            let at = prompt.find(&heading).unwrap_or_else(|| panic!("{name}"));
            assert!(at > last, "{name} out of order");
            last = at;
            assert!(prompt[at..].contains(first), "{name}: {first}");
        }
        assert!(prompt.contains("### Recipes"));
        for director in [
            "video.json",
            "script.md",
            "Narrate",
            "Pronunciation",
            "Render",
            "Libraries",
        ] {
            assert!(
                !prompt.contains(&format!("\n## {director}\n")),
                "{director}"
            );
        }
        // A `## ` line inside a code fence is not a heading.
        assert!(section(REFERENCE, "idea").is_none());
        let template = SCENE_ROLE.split_whitespace().collect::<Vec<_>>().join(" ");
        for name in BUILDER_SECTIONS {
            assert!(template.contains(name), "the role names {name}");
        }
    }

    /// Codex gets no role prompt, so its director opens each scene brief with
    /// the files to read, in the same words in SKILL.md and `studio agents`.
    #[test]
    fn codex_scene_briefs_open_with_the_same_files() {
        let opening = codex_scene_opening();
        let skill = include_str!("../../templates/skills/oculus-studio/SKILL.md");
        let skill = skill.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(skill.contains(&opening), "SKILL.md lacks: {opening}");
        let roles = assigned(Provider::Codex, chain);
        let scene = how_to_spawn(Provider::Codex, roles[1].0, roles[1].1.as_ref());
        assert!(scene.ends_with(&opening), "{scene}");
        let research = how_to_spawn(
            Provider::Codex,
            roles[0].0,
            Some(&sel(Provider::Codex, "gpt-6.1-sol", None)),
        );
        assert!(!research.contains("build.md"));
    }
}
