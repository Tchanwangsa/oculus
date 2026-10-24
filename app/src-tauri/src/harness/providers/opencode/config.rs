//! The `agents/opencode.json` document: permissions and system prompts,
//! rendered from the template.

use std::path::{Path, PathBuf};

use super::CONFIG_NAME;

const CONFIG_TEMPLATE: &str = include_str!("../../../../templates/OPENCODE.template.json");

/// The hidden agents' prompts, one per [`super::NAMING_AGENT`], [`super::WRITER_AGENT`] and
/// [`super::LECTURE_END_AGENT`].
pub struct OneOffPrompts<'a> {
    pub naming: &'a str,
    pub writer: &'a str,
    pub lecture_end: &'a str,
}

/// Write `agents/opencode.json`: the permissions, the system prompts and the
/// Studio subagents (`studio_agents::opencode_agents_fragment`). Rewritten on
/// every server start, since all three carry the library's paths or settings.
pub fn write_config(
    directory: &Path,
    library: &Path,
    prompt: &str,
    one_off: &OneOffPrompts,
    subagents: &str,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(directory)
        .map_err(|e| format!("cannot create {}: {e}", directory.display()))?;
    let path = directory.join(CONFIG_NAME);
    std::fs::write(&path, render_config(library, prompt, one_off, subagents))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

/// Text substitution, never a parse/re-serialize: the ruleset is ordered
/// (last rule wins) and `serde_json`'s map sorts keys, which would silently
/// invert the containment. Values are JSON-escaped.
fn render_config(library: &Path, prompt: &str, one_off: &OneOffPrompts, subagents: &str) -> String {
    CONFIG_TEMPLATE
        .replace("\"{{SUBAGENTS}}\": null,", subagents)
        .replace(
            "{{LIBRARY}}",
            &json_fragment(&crate::library::paths::slash_path(library)),
        )
        .replace("\"{{PROMPT}}\"", &json_string(prompt))
        .replace(
            "\"{{REVISION_PROMPT}}\"",
            &json_string(crate::harness::app::revision::INSTRUCTIONS),
        )
        .replace("\"{{NAMING_PROMPT}}\"", &json_string(one_off.naming))
        .replace("\"{{WRITER_PROMPT}}\"", &json_string(one_off.writer))
        .replace(
            "\"{{DICTATION_PROMPT}}\"",
            &json_string(crate::harness::app::dictation::INSTRUCTIONS),
        )
        .replace(
            "\"{{LECTURE_END_PROMPT}}\"",
            &json_string(one_off.lecture_end),
        )
}

fn json_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// [`json_string`] without the quotes.
fn json_fragment(s: &str) -> String {
    let q = json_string(s);
    q[1..q.len() - 1].to_string()
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::super::{
        AGENT, DICTATION_AGENT, LECTURE_END_AGENT, NAMING_AGENT, REVISION_AGENT, WRITER_AGENT,
    };
    use super::*;

    /// opencode has no OS sandbox, so `oculus` reaches keyd's socket with no
    /// grant: the config sets none, denies nothing that covers the socket,
    /// and lets the CLI run.
    #[test]
    fn the_opencode_config_adds_no_sandbox_and_denies_nothing_over_keyds_socket() {
        let lib = "/Users/x/Library/Application Support/com.tchan.oculus";
        let rendered = render_config(
            Path::new(lib),
            "p",
            &OneOffPrompts {
                naming: "n",
                writer: "w",
                lecture_end: "l",
            },
            "",
        );
        let v: Value = serde_json::from_str(&rendered).expect("the template renders valid JSON");
        assert!(v.get("sandbox").is_none());
        assert_eq!(
            v.pointer("/permission/bash/oculus *"),
            Some(&json!("allow"))
        );
        let socket = keyd_core::paths::socket(std::path::Path::new(lib));
        for (pattern, rule) in v
            .pointer("/permission/edit")
            .and_then(Value::as_object)
            .unwrap()
        {
            if rule == "deny" && pattern.starts_with('/') {
                assert!(
                    !crate::harness::protected::glob_covers(pattern, socket.to_str().unwrap()),
                    "{pattern} covers {socket:?}"
                );
            }
        }
    }

    /// Every `agent_write_dirs` folder is allowed by a rule after every
    /// library deny: opencode's `*` crosses `/` and the last matching rule
    /// wins, so `{{LIBRARY}}/*.json` alone would deny a video's video.json.
    #[test]
    fn studio_is_allowed_after_every_library_deny() {
        let lib = Path::new("/Users/x/Library/Application Support/com.tchan.oculus");
        let rendered = render_config(
            lib,
            "p",
            &OneOffPrompts {
                naming: "n",
                writer: "w",
                lecture_end: "l",
            },
            "",
        );
        let rules: Vec<(String, String)> = rendered
            .lines()
            .map(str::trim)
            .filter_map(|l| {
                let (k, v) = l.trim_end_matches(',').split_once("\": ")?;
                Some((
                    k.trim_start_matches('"').to_owned(),
                    v.trim_matches('"').to_owned(),
                ))
            })
            .filter(|(k, _)| k.starts_with(lib.to_str().unwrap()))
            .collect();
        let last_deny = rules.iter().rposition(|(_, v)| v == "deny").unwrap();
        for dir in crate::library::paths::agent_write_dirs(lib) {
            let key = format!("{}/**", crate::library::paths::slash_path(&dir));
            let at = rules
                .iter()
                .position(|(k, v)| *k == key && v == "allow")
                .unwrap_or_else(|| panic!("no allow for {key} in {rules:?}"));
            assert!(at > last_deny, "{key} must come after every deny");
        }
    }

    /// The Studio subagents land in `agent` beside the primary ones, as
    /// subagents on their job's model with no permission of their own, so
    /// they run under the parent session's.
    #[test]
    fn the_studio_subagents_are_rendered_from_the_jobs() {
        use crate::harness::jobs::{Job, JobSelection};
        use crate::harness::studio_agents::{assigned, opencode_agents_fragment};
        let roles = assigned(crate::harness::Provider::Opencode, |job| match job {
            Job::StudioScene => vec![JobSelection {
                provider: crate::harness::Provider::Opencode,
                model: "openrouter/strong".into(),
                reasoning_effort: Some("high".into()),
            }],
            _ => Vec::new(),
        });
        let rendered = render_config(
            Path::new("/Users/x/Library/Application Support/oculus"),
            "p",
            &OneOffPrompts {
                naming: "n",
                writer: "w",
                lecture_end: "l",
            },
            &opencode_agents_fragment(&roles),
        );
        let v: Value = serde_json::from_str(&rendered).expect("the template renders valid JSON");
        let scene = &v["agent"]["studio-scene"];
        assert_eq!(scene["mode"], "subagent");
        assert_eq!(scene["model"], "openrouter/strong");
        assert_eq!(scene["variant"], "high");
        assert!(scene.get("permission").is_none());
        assert!(
            v["agent"].get("studio-research").is_none(),
            "off: not defined"
        );
        assert_eq!(v["agent"][AGENT]["mode"], "primary");
    }

    /// Pins the rule shapes that work: siblings of `agents/` denied one by one,
    /// the bash map ending on an allow, and in-directory paths relative.
    #[test]
    fn the_rendered_config_denies_the_right_things() {
        let rendered = render_config(
            Path::new("/Users/x/Library/Application Support/oculus"),
            "# Working inside Oculus\n\n\"quoted\" \\ backslash",
            &OneOffPrompts {
                naming: "You name conversations.",
                writer: "You complete \"notes\".",
                lecture_end: "You find where lectures end.",
            },
            "",
        );
        let v: Value = serde_json::from_str(&rendered).expect("the template renders valid JSON");

        let edit = v["permission"]["edit"].as_object().unwrap();
        assert_eq!(
            edit["*"], "allow",
            "the root stays allowed; the siblings are named"
        );
        for sib in [
            "courses/**",
            "lectures/**",
            "canvas-session/**",
            "oculus.db*",
            "vault.bin*",
            ".vault.bin.*",
        ] {
            let key = format!("/Users/x/Library/Application Support/oculus/{sib}");
            assert_eq!(edit[&key], "deny", "{sib}");
        }
        for inside in ["opencode.json", "skills/**", ".opencode/**"] {
            assert_eq!(
                edit[inside], "deny",
                "{inside} is relative, being inside the cwd"
            );
        }
        assert_eq!(v["skills"]["paths"], json!(["skills"]));

        let bash: Vec<(&String, &Value)> = v["permission"]["bash"]
            .as_object()
            .unwrap()
            .iter()
            .collect();
        assert_eq!(bash.first().unwrap().1, "deny", "the default is no");
        assert_eq!(
            bash.last().unwrap().1,
            "allow",
            "or bash is removed from the tool list"
        );

        // `deny` removes the tool, so nothing waits on an answerer.
        assert_eq!(v["permission"]["question"], "deny");
        // Subagents are on, one level deep: a subagent's own session is
        // denied `task` by opencode unless its agent allows it.
        assert_eq!(v["permission"]["task"], "allow");
        assert_eq!(v["subagent_depth"], 1);
        assert!(
            v["agent"].get("{{SUBAGENTS}}").is_none(),
            "the placeholder is always replaced"
        );
        // Parity with Claude and Codex.
        for key in ["webfetch", "websearch"] {
            assert_eq!(v["permission"][key], "allow", "{key}");
        }
        assert_eq!(
            v["permission"]["read"], "allow",
            "the library stays readable"
        );

        assert!(v["agent"][AGENT]["prompt"]
            .as_str()
            .unwrap()
            .contains("\"quoted\" \\ backslash"));
        // The one-off agents are hidden, tool-less (a trailing `*` deny is the
        // last rule for every tool) and carry their own prompt.
        for (agent, prompt) in [
            (
                DICTATION_AGENT,
                crate::harness::app::dictation::INSTRUCTIONS,
            ),
            (REVISION_AGENT, crate::harness::app::revision::INSTRUCTIONS),
            (NAMING_AGENT, "You name conversations."),
            (WRITER_AGENT, "You complete \"notes\"."),
            (LECTURE_END_AGENT, "You find where lectures end."),
        ] {
            assert_eq!(v["agent"][agent]["hidden"], true, "{agent}");
            assert_eq!(v["agent"][agent]["permission"]["*"], "deny", "{agent}");
            assert_eq!(v["agent"][agent]["prompt"], prompt, "{agent}");
        }
    }
}
