//! Asking an address what is answering there.

use serde_json::Value;

use super::{CONNECT_TIMEOUT, HEALTH_PATH, HEALTH_TIMEOUT, V1_HEALTH_PATH};

/// What is answering at an address. `WrongApi` is spelled out because
/// "unreachable" would be a lie about a running MinerU 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalHealth {
    Ready,
    /// Answered `/health` and said it is not serving work.
    NotServing,
    /// No `/health`, but `/v1/health` answers: MinerU 4's V1 service, which
    /// dropped `/file_parse` for an upload/job/download cycle.
    WrongApi,
    Unreachable,
}

/// Ask an address what it is. Takes a URL, not a client, so the settings
/// page can test the endpoint field before it is saved.
pub fn probe(base_url: &str) -> LocalHealth {
    let base = base_url.trim().trim_end_matches('/');
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout(HEALTH_TIMEOUT)
        .build();
    match agent.get(&format!("{base}{HEALTH_PATH}")).call() {
        Ok(response) => {
            let body = response
                .into_string()
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .unwrap_or(Value::Null);
            match body.get("status").and_then(Value::as_str) {
                Some("healthy") => LocalHealth::Ready,
                _ => LocalHealth::NotServing,
            }
        }
        Err(ureq::Error::Status(404, _)) => {
            match agent.get(&format!("{base}{V1_HEALTH_PATH}")).call() {
                Ok(_) => LocalHealth::WrongApi,
                Err(_) => LocalHealth::Unreachable,
            }
        }
        // MinerU's 503 is "task manager not up yet": present, not absent.
        Err(ureq::Error::Status(_, _)) => LocalHealth::NotServing,
        Err(ureq::Error::Transport(_)) => LocalHealth::Unreachable,
    }
}
