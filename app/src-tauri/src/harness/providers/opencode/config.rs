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

/// Write `agents/opencode.json`: the permissions and the system prompts.
/// Rewritten on every server start, since both carry the library's paths.
pub fn write_config(
    directory: &Path,
    library: &Path,
    prompt: &str,
    one_off: &OneOffPrompts,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(directory)
        .map_err(|e| format!("cannot create {}: {e}", directory.display()))?;
    let path = directory.join(CONFIG_NAME);
    std::fs::write(&path, render_config(library, prompt, one_off))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

/// Text substitution, never a parse/re-serialize: the ruleset is ordered
/// (last rule wins) and `serde_json`'s map sorts keys, which would silently
/// invert the containment. Values are JSON-escaped.
fn render_config(library: &Path, prompt: &str, one_off: &OneOffPrompts) -> String {
    CONFIG_TEMPLATE
        .replace(
            "{{LIBRARY}}",
            &json_fragment(&library.display().to_string()),
        )
        .replace("\"{{PROMPT}}\"", &json_string(prompt))
        .replace("\"{{NAMING_PROMPT}}\"", &json_string(one_off.naming))
        .replace("\"{{WRITER_PROMPT}}\"", &json_string(one_off.writer))
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

    use super::super::{AGENT, LECTURE_END_AGENT, NAMING_AGENT, WRITER_AGENT};
    use super::*;

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
        for key in ["question", "task"] {
            assert_eq!(v["permission"][key], "deny", "{key}");
        }
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
