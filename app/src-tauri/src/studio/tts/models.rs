//! The model ids a TTS server offers: `GET {base}/v1/models`, OpenAI's
//! listing. Free on every server, so Settings -> Voice may call it; it never
//! synthesises.

use serde_json::Value;

use super::{excerpt, normalise_base, read_body};
use crate::providers::ratelimit::transport_detail;

/// The ids in the server's `/v1/models` listing, in its order, without
/// repeats. No timeout beyond ureq's connect default.
pub fn list_models(base_url: &str) -> Result<Vec<String>, String> {
    let base = normalise_base(base_url)?;
    let url = format!("{base}/v1/models");
    let agent = ureq::AgentBuilder::new().build();
    match agent.get(&url).call() {
        Ok(response) => {
            let status = response.status();
            let body = read_body(response);
            parse_models(&body).map_err(|e| {
                format!(
                    "the TTS server at {base} answered {status} to /v1/models with {e}: {}",
                    excerpt(&body)
                )
            })
        }
        Err(ureq::Error::Status(status, response)) => Err(format!(
            "the TTS server at {base} answered {status} to /v1/models, so it may not list its models; type the model id instead: {}",
            excerpt(&read_body(response))
        )),
        Err(ureq::Error::Transport(t)) => Err(format!(
            "could not reach the TTS server at {base}: {}",
            transport_detail(&t)
        )),
    }
}

/// `{ "data": [{ "id": ... }] }` to its ids; an entry without a string id
/// is skipped.
pub(super) fn parse_models(body: &[u8]) -> Result<Vec<String>, String> {
    let value: Value =
        serde_json::from_slice(body).map_err(|_| "a body that is not JSON".to_string())?;
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or("JSON without a `data` list")?;
    let mut ids: Vec<String> = Vec::new();
    for id in data
        .iter()
        .filter_map(|m| m.get("id").and_then(Value::as_str))
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        if !ids.iter().any(|seen| seen == id) {
            ids.push(id.to_owned());
        }
    }
    Ok(ids)
}
