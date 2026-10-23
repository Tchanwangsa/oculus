//! The environment a provider child process gets.

use std::path::PathBuf;

use super::locate::well_known_dirs;
use super::{binary, oculus_cli, PROVIDERS};

/// The environment a provider child gets. API keys are stripped so a key in the
/// user's shell cannot move a subscription onto per-token billing, or grow
/// opencode a provider nobody chose. The `oculus` binary's dir heads PATH,
/// because `AGENTS.md` tells the agent to run it.
pub fn child_env() -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| {
            !matches!(
                k.as_str(),
                "ANTHROPIC_API_KEY"
                    | "OPENAI_API_KEY"
                    | "CLAUDE_AGENT_SDK_CLIENT_APP"
                    // Set inside a Claude Code session; the CLI refuses to nest.
                    | "CLAUDECODE"
                    | "CLAUDE_CODE_ENTRYPOINT"
            )
        })
        .collect();

    let mut path_parts: Vec<PathBuf> = Vec::new();
    if let Some(cli) = oculus_cli() {
        if let Some(d) = cli.parent() {
            path_parts.push(d.to_path_buf());
        }
    }
    for p in PROVIDERS {
        if let Ok(b) = binary(p) {
            if let Some(d) = b.parent() {
                path_parts.push(d.to_path_buf());
            }
        }
    }
    if let Some(cur) = std::env::var_os("PATH") {
        path_parts.extend(std::env::split_paths(&cur));
    } else {
        path_parts.extend(well_known_dirs());
        path_parts.push(PathBuf::from("/usr/bin"));
        path_parts.push(PathBuf::from("/bin"));
    }
    let mut seen = std::collections::HashSet::new();
    path_parts.retain(|p| seen.insert(p.clone()));
    if let Ok(joined) = std::env::join_paths(&path_parts) {
        env.retain(|(k, _)| k != "PATH");
        env.push(("PATH".into(), joined.to_string_lossy().into_owned()));
    }
    env.retain(|(key, _)| key != "OCULUS_AGENT");
    env.push(("OCULUS_AGENT".into(), "1".into()));
    env
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_provider_child_has_the_confirmation_gate_and_no_api_billing_keys() {
        let env = super::child_env();
        let agent: Vec<_> = env
            .iter()
            .filter(|(key, _)| key == "OCULUS_AGENT")
            .collect();
        assert_eq!(agent.len(), 1);
        assert_eq!(agent[0].1, "1");
        assert!(!env.iter().any(|(key, _)| matches!(
            key.as_str(),
            "ANTHROPIC_API_KEY" | "OPENAI_API_KEY" | "CLAUDECODE" | "CLAUDE_CODE_ENTRYPOINT"
        )));
    }
}
