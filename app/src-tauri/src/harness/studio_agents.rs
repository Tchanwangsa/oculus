//! The two subagents a chat agent may start while it makes a Studio video:
//! `studio-research` reads one slice of the source, `studio-scene` builds one
//! scene. Their models come from Settings → Jobs (`Job::StudioResearch`,
//! `Job::StudioScene`), one entry per agent (`jobs::for_agent`).
//!
//! Each CLI starts a subagent its own way, so each gets the model its own
//! way: Claude and opencode through named agents written at spawn (Claude's
//! `--agents`, opencode's config), Codex through `spawn_agent`'s `model`,
//! which the skill asks for by the words [`how_to_spawn`] prints. agy's
//! `invoke_subagent` takes no model, so agy does the work itself.
//! docs/harness.md ("Subagents") has the measurements behind each.

use serde_json::{json, Map, Value};

use super::jobs::{self, Job, JobSelection};
use super::Provider;

/// One subagent: its name in every CLI, the job holding its models, and its
/// system prompt (the role's half; the skill has the rest).
pub struct Role {
    pub name: &'static str,
    pub job: Job,
    pub description: &'static str,
    pub prompt: &'static str,
}

pub const ROLES: [Role; 2] = [
    Role {
        name: "studio-research",
        job: Job::StudioResearch,
        description: "Reads one slice of a lecture for an Oculus Studio video and writes notes",
        prompt: include_str!("../../templates/STUDIO_RESEARCH.template.md"),
    },
    Role {
        name: "studio-scene",
        job: Job::StudioScene,
        description: "Builds one scene of an Oculus Studio video",
        prompt: include_str!("../../templates/STUDIO_SCENE.template.md"),
    },
];

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
            "prompt": role.prompt,
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
            "prompt": role.prompt,
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
            format!(
                "spawn_agent with model \"{}\"{effort}, fork_turns \"none\", a task_name of lowercase letters, digits and underscores, and the brief as message. At most 3 at once; collect each with wait_agent, then close_agent it, since a finished agent holds its slot until closed.",
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
        assert!(v["studio-scene"]["prompt"]
            .as_str()
            .unwrap()
            .contains("section \"Build\""));
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
}
