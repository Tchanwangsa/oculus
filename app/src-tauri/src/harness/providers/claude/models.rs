//! The CLI's model catalogue, asked of a throwaway process.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use serde_json::Value;

use crate::harness::child::{str_of, ChildProc};

/// One row of the CLI's `/model` catalogue, as `initialize` reports it. Raw
/// on purpose: `claudeAsModels` in `app/src/lib/harness/models.ts` adapts it.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// An alias (`sonnet`, `opus[1m]`, `default`) or a full name.
    pub value: String,
    /// The concrete model the alias stands for today.
    pub resolved_model: String,
    pub display_name: String,
    pub description: String,
    /// Empty for a model that takes no `--effort` (Haiku).
    pub supported_effort_levels: Vec<String>,
}

/// For a CLI wedged on a login prompt or an update, not a slow one.
const MODELS_TIMEOUT: Duration = Duration::from_secs(20);

/// Ask the installed CLI which models it offers, without starting a turn or
/// making an API call: the catalogue rides the answer to the stream-json
/// `initialize` control request. A throwaway, inert process (no hooks, no
/// MCP, no session persisted), killed afterwards since it waits for input.
pub fn list_models(
    bin: &Path,
    cwd: &Path,
    env: &[(String, String)],
) -> Result<Vec<ModelInfo>, String> {
    const REQUEST_ID: &str = "oculus-models";
    let mut cmd = Command::new(bin);
    cmd.arg("-p")
        .args(["--input-format", "stream-json"])
        .args(["--output-format", "stream-json"])
        .arg("--verbose")
        .arg("--no-session-persistence")
        .arg("--strict-mcp-config")
        .args(["--settings", r#"{"disableAllHooks":true}"#])
        .current_dir(cwd)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .env("CLAUDE_CODE_ENTRYPOINT", "cli");
    // stdin stays open until the kill: an early EOF may make the CLI leave
    // unanswered.
    let (proc, stdout) = ChildProc::spawn("claude", &mut cmd, true)?;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(answer) = models_from_response(&v, REQUEST_ID) {
                let _ = tx.send(Some(answer));
                return;
            }
        }
        let _ = tx.send(None);
    });

    let written = proc.write_line(&serde_json::json!({
        "type": "control_request",
        "request_id": REQUEST_ID,
        "request": { "subtype": "initialize" },
    }));
    let answer = written.and_then(|()| {
        rx.recv_timeout(MODELS_TIMEOUT).map_err(|_| {
            format!(
                "claude did not list its models within {}s",
                MODELS_TIMEOUT.as_secs()
            )
        })
    });

    let code = proc.kill();
    match answer? {
        Some(result) => result,
        None => Err(proc.with_tail(format!(
            "claude exited (code {code:?}) before listing its models"
        ))),
    }
}

/// The catalogue out of one stdout line, or None when it is not the answer to
/// `request_id`. A missing field reads as empty; a row with no name is skipped.
fn models_from_response(v: &Value, request_id: &str) -> Option<Result<Vec<ModelInfo>, String>> {
    if v.get("type").and_then(|t| t.as_str()) != Some("control_response")
        || v.pointer("/response/request_id").and_then(|s| s.as_str()) != Some(request_id)
    {
        return None;
    }
    if v.pointer("/response/subtype").and_then(|s| s.as_str()) == Some("error") {
        let why = v
            .pointer("/response/error")
            .and_then(|e| e.as_str())
            .unwrap_or("refused");
        return Some(Err(format!("claude would not initialize: {why}")));
    }
    let Some(rows) = v
        .pointer("/response/response/models")
        .and_then(|m| m.as_array())
    else {
        return Some(Err("claude's initialize answer has no models".into()));
    };
    Some(Ok(rows
        .iter()
        .map(|m| ModelInfo {
            value: str_of(m, "value"),
            resolved_model: str_of(m, "resolvedModel"),
            display_name: str_of(m, "displayName"),
            description: str_of(m, "description"),
            supported_effort_levels: m
                .get("supportedEffortLevels")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .filter(|m| !m.value.is_empty() || !m.resolved_model.is_empty())
        .collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::event::Provider;

    /// `initialize` from CLI 2.1.281, cut to its models. Haiku declares no
    /// effort levels and must still arrive.
    #[test]
    fn the_initialize_answer_lists_the_models() {
        let line = r#"{"type":"control_response","response":{"subtype":"success","request_id":"oculus-models","response":{"models":[
            {"value":"default","resolvedModel":"claude-opus-5-5[1m]","displayName":"Default (recommended)","description":"Opus 5.5 with 1M context · Best for everyday, complex tasks","supportsEffort":true,"supportedEffortLevels":["low","medium","high","xhigh","max"]},
            {"value":"claude-fable-5-1[1m]","resolvedModel":"claude-fable-5-1","displayName":"Fable","description":"Fable 5.1 · Most capable for your hardest and longest-running tasks","supportedEffortLevels":["low","medium","high","xhigh","max"]},
            {"value":"haiku","resolvedModel":"claude-haiku-4-5-20251001","displayName":"Haiku","description":"Haiku 4.5 · Fastest for quick answers"},
            {"displayName":"nameless"}
        ]}}}"#;
        let v: Value = serde_json::from_str(line).unwrap();

        assert!(
            models_from_response(&v, "someone-else").is_none(),
            "another request's answer"
        );
        let models = models_from_response(&v, "oculus-models").unwrap().unwrap();
        assert_eq!(
            models.len(),
            3,
            "a row with neither name is dropped, not fatal"
        );
        assert_eq!(models[0].value, "default");
        assert_eq!(models[0].resolved_model, "claude-opus-5-5[1m]");
        assert_eq!(models[0].supported_effort_levels.len(), 5);
        assert_eq!(models[1].value, "claude-fable-5-1[1m]");
        assert_eq!(models[2].resolved_model, "claude-haiku-4-5-20251001");
        assert!(models[2].supported_effort_levels.is_empty());

        let refused: Value = serde_json::from_str(
            r#"{"type":"control_response","response":{"subtype":"error","request_id":"oculus-models","error":"not logged in"}}"#,
        )
        .unwrap();
        assert!(models_from_response(&refused, "oculus-models")
            .unwrap()
            .is_err());
    }

    /// Needs an installed CLI: `cargo test --lib list_models_from_the_real_cli -- --ignored`.
    #[test]
    #[ignore]
    fn list_models_from_the_real_cli() {
        let bin = crate::harness::discover::binary(Provider::Claude).expect("claude on PATH");
        let cwd = std::env::temp_dir();
        let started = std::time::Instant::now();
        let models = list_models(&bin, &cwd, &crate::harness::discover::child_env()).unwrap();
        eprintln!(
            "{} models in {:?}: {models:#?}",
            models.len(),
            started.elapsed()
        );
        assert!(!models.is_empty());
    }
}
