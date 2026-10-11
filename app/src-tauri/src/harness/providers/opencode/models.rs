//! The model catalogue: listing it, and reading one row of it.

use serde_json::Value;

use super::server::OpencodeServer;
use super::types::{ModelCost, ModelFacts, ModelInfo};

impl OpencodeServer {
    /// Every model opencode can reach, from `GET /config/providers` — what
    /// `opencode models` prints. Not `/api/model`: that lists only the
    /// providers the instance happened to instantiate, and omits signed-in
    /// ones like OpenRouter entirely (1.18.2). The response is never logged
    /// or quoted into an error: rows can carry the provider's real key.
    pub fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        parse_models(&self.get(&format!("/config/providers?{}", self.directory_query()))?)
    }

    /// One `providerID/id`'s context window, for the usage ring. Same source
    /// as [`Self::list_models`], so any model picked there is found here.
    pub(super) fn context_window(&self, model: &str) -> Option<u64> {
        let (provider, id) = split_model(model)?;
        let v = self
            .get(&format!("/config/providers?{}", self.directory_query()))
            .ok()?;
        v["providers"]
            .as_array()?
            .iter()
            .find(|p| p["id"].as_str() == Some(provider.as_str()))?["models"]
            .as_object()?
            .values()
            .find(|m| m["id"].as_str() == Some(id.as_str()))?["limit"]["context"]
            .as_u64()
    }
}

/// `providerID/id`, split on the **first** slash only: an id can itself
/// contain one (`tss-nvidia-spark/nvidia/Qwen3.6-35B-A3B-NVFP4`).
pub fn split_model(model: &str) -> Option<(String, String)> {
    let (p, id) = model.split_once('/')?;
    (!p.is_empty() && !id.is_empty()).then(|| (p.to_string(), id.to_string()))
}

/// The rows of a `GET /config/providers`, minus disabled and deprecated
/// models, sorted by id.
fn parse_models(v: &Value) -> Result<Vec<ModelInfo>, String> {
    let list = v["providers"]
        .as_array()
        .ok_or("opencode /config/providers: no providers")?;
    let mut out = Vec::new();
    for p in list {
        let provider = p["id"].as_str().unwrap_or("");
        // A map keyed by id; the entry's own `id` is what is read.
        let Some(models) = p["models"].as_object() else {
            continue;
        };
        for m in models.values() {
            let id = m["id"].as_str().unwrap_or("");
            if id.is_empty() || provider.is_empty() {
                continue;
            }
            if m["enabled"].as_bool() == Some(false) {
                continue;
            }
            if m["status"].as_str() == Some("deprecated") {
                continue;
            }
            let variants = variant_ids(&m["variants"]);
            let name = m["name"].as_str().unwrap_or(id);
            let caps = &m["capabilities"];
            // Absent reads as capable, or a provider whose rows say nothing
            // would have its whole catalogue gated out.
            let flag = |v: &Value| v.as_bool() != Some(false);
            out.push(ModelInfo {
                id: format!("{provider}/{id}"),
                display_name: name.to_string(),
                description: describe(m),
                default_variant: default_variant(&variants),
                variants,
                is_default: false,
                tool_call: flag(&caps["toolcall"]),
                text_input: flag(&caps["input"]["text"]),
                text_output: flag(&caps["output"]["text"]),
                facts: facts(m),
            });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// The table's columns off one catalogue row. Absent stays `None`, never 0.
pub(super) fn facts(m: &Value) -> ModelFacts {
    let caps = &m["capabilities"];
    let cost = m["cost"].as_object().map(|c| ModelCost {
        input: c.get("input").and_then(Value::as_f64),
        output: c.get("output").and_then(Value::as_f64),
        cache_read: m["cost"]["cache"]["read"].as_f64(),
        cache_write: m["cost"]["cache"]["write"].as_f64(),
    });
    let text = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(String::from);
    ModelFacts {
        cost,
        context: m["limit"]["context"].as_u64(),
        max_output: m["limit"]["output"].as_u64(),
        reasoning: caps["reasoning"].as_bool() == Some(true),
        attachment: caps["attachment"].as_bool() == Some(true),
        inputs: ["image", "pdf", "audio", "video"]
            .into_iter()
            .filter(|k| caps["input"][*k].as_bool() == Some(true))
            .map(String::from)
            .collect(),
        release_date: text(&m["release_date"]),
        family: text(&m["family"]),
    }
}

/// A model's reasoning levels: an object keyed by id, or an array of `{id}`.
fn variant_ids(v: &Value) -> Vec<String> {
    if let Some(a) = v.as_array() {
        return a
            .iter()
            .filter_map(|x| x["id"].as_str().map(String::from))
            .collect();
    }
    match v.as_object() {
        Some(o) => o.keys().cloned().collect(),
        None => Vec::new(),
    }
}

/// Where a fresh pick of this model lands: `high`, else the nearest level
/// below it, by name — the list's order is a JSON map's alphabetical one.
pub(super) fn default_variant(variants: &[String]) -> Option<String> {
    ["high", "medium", "low", "minimal", "none"]
        .iter()
        .find_map(|p| variants.iter().find(|v| v.as_str() == *p).cloned())
        .or_else(|| variants.first().cloned())
}

fn describe(m: &Value) -> String {
    let mut bits: Vec<String> = Vec::new();
    if let Some(f) = m["family"].as_str() {
        bits.push(f.to_string());
    }
    if let Some(w) = m["limit"]["context"].as_u64() {
        bits.push(format!("{}K context", w / 1000));
    }
    if m["status"].as_str() == Some("beta") || m["status"].as_str() == Some("alpha") {
        bits.push(m["status"].as_str().unwrap_or("").to_string());
    }
    bits.join(" · ")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_model_s_capabilities_come_off_the_row_and_default_to_capable() {
        let v = json!({
            "providers": [{ "id": "openrouter", "models": {
                "a": { "id": "a", "name": "A", "capabilities": {
                    "toolcall": false,
                    "input": { "text": true }, "output": { "text": true } } },
                "b": { "id": "b", "name": "B", "capabilities": {
                    "toolcall": true,
                    "input": { "text": true }, "output": { "text": false } } },
                "c": { "id": "c", "name": "C" }
            }}]
        });
        let models = parse_models(&v).unwrap();
        assert_eq!(models.len(), 3);
        assert!(!models[0].tool_call, "a: toolcall false is read");
        assert!(models[0].text_input && models[0].text_output);
        assert!(models[1].tool_call);
        assert!(
            !models[1].text_output,
            "b: an output it cannot write in text"
        );
        assert!(
            models[2].tool_call && models[2].text_input && models[2].text_output,
            "c: no capabilities block at all reads as capable, never as refused"
        );
    }

    #[test]
    fn a_model_s_facts_come_off_the_row_and_an_unpriced_row_is_unknown_not_free() {
        let v = json!({
            "providers": [{ "id": "openrouter", "models": {
                "a": { "id": "a", "name": "A", "family": "qwen", "release_date": "2026-05-21",
                    "cost": { "input": 1.475, "output": 4.425, "cache": { "read": 0.1 } },
                    "limit": { "context": 1000000, "output": 131072 },
                    "capabilities": { "reasoning": true, "attachment": true,
                        "input": { "text": true, "image": true, "pdf": true, "audio": false } } },
                "b": { "id": "b", "name": "B", "cost": { "input": 0, "output": 0 } },
                "c": { "id": "c", "name": "C", "family": "" }
            }}]
        });
        let models = parse_models(&v).unwrap();
        let a = &models[0].facts;
        let cost = a.cost.as_ref().unwrap();
        assert_eq!((cost.input, cost.output), (Some(1.475), Some(4.425)));
        assert_eq!((cost.cache_read, cost.cache_write), (Some(0.1), None));
        assert_eq!((a.context, a.max_output), (Some(1_000_000), Some(131_072)));
        assert!(a.reasoning && a.attachment);
        assert_eq!(a.inputs, ["image", "pdf"]);
        assert_eq!(a.release_date.as_deref(), Some("2026-05-21"));
        assert_eq!(a.family.as_deref(), Some("qwen"));
        assert_eq!(
            models[1].facts.cost.as_ref().unwrap().input,
            Some(0.0),
            "b: a stated zero is zero"
        );
        assert_eq!(
            models[2].facts,
            ModelFacts::default(),
            "c: nothing stated is all unknown"
        );
    }

    /// `ModelFacts` in `app/src/lib/harness/models.ts` reads these names.
    #[test]
    fn a_model_s_facts_are_spelled_the_way_the_table_reads_them() {
        let json = serde_json::to_value(ModelFacts {
            cost: Some(ModelCost {
                input: None,
                output: None,
                cache_read: None,
                cache_write: None,
            }),
            ..ModelFacts::default()
        })
        .unwrap();
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "attachment",
                "context",
                "cost",
                "family",
                "inputs",
                "maxOutput",
                "reasoning",
                "releaseDate"
            ]
        );
        let mut cost: Vec<&str> = json["cost"]
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        cost.sort_unstable();
        assert_eq!(cost, ["cacheRead", "cacheWrite", "input", "output"]);
    }

    /// `OpencodeModel` in `app/src/lib/harness/models.ts` reads these names; a rename
    /// is a silently empty model list, not a type error.
    #[test]
    fn a_model_row_is_spelled_the_way_the_picker_reads_it() {
        let json = serde_json::to_value(ModelInfo {
            id: "anthropic/claude-opus-4-5".into(),
            display_name: "Claude Opus 4.5 (latest)".into(),
            description: "claude-opus · 200K context".into(),
            variants: vec![],
            default_variant: None,
            is_default: false,
            tool_call: true,
            text_input: true,
            text_output: true,
            facts: ModelFacts::default(),
        })
        .unwrap();
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "defaultVariant",
                "description",
                "displayName",
                "facts",
                "id",
                "isDefault",
                "textInput",
                "textOutput",
                "toolCall",
                "variants"
            ]
        );
    }

    #[test]
    fn a_model_is_split_on_its_first_slash_only() {
        assert_eq!(
            split_model("tss-nvidia-spark/nvidia/Qwen3.6-35B-A3B-NVFP4"),
            Some((
                "tss-nvidia-spark".into(),
                "nvidia/Qwen3.6-35B-A3B-NVFP4".into()
            ))
        );
        assert_eq!(
            split_model("anthropic/claude-opus-4-5"),
            Some(("anthropic".into(), "claude-opus-4-5".into()))
        );
        assert_eq!(split_model("no-slash"), None);
    }

    /// `/config/providers` as 1.18.2 answers: providers, each with a map of
    /// models keyed by id.
    #[test]
    fn the_model_list_is_every_configured_provider_s_models() {
        let v = json!({
            "default": {},
            "providers": [
                { "id": "openrouter", "models": {
                    "aion-labs/aion-2.0": {
                        "id": "aion-labs/aion-2.0", "name": "Aion-2.0",
                        "status": "active", "limit": { "context": 131072 }, "variants": {} },
                    "old/thing": {
                        "id": "old/thing", "name": "Old", "status": "deprecated",
                        "limit": { "context": 8192 }, "variants": {} },
                }},
                { "id": "tss-nvidia-spark", "models": {
                    "nvidia/Qwen3.6-35B-A3B-NVFP4": {
                        "id": "nvidia/Qwen3.6-35B-A3B-NVFP4", "name": "Qwen3.6 35B",
                        "limit": { "context": 262144 },
                        "variants": { "high": {}, "low": {} } },
                }},
                // No models at all: skipped, not an error.
                { "id": "anthropic", "models": {} },
            ]
        });

        let models = parse_models(&v).unwrap();

        assert_eq!(
            models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec![
                "openrouter/aion-labs/aion-2.0",
                "tss-nvidia-spark/nvidia/Qwen3.6-35B-A3B-NVFP4",
            ],
            "deprecated is dropped, and an id keeps every slash it came with",
        );
        assert_eq!(models[0].display_name, "Aion-2.0");
        assert_eq!(models[0].description, "131K context");
        assert!(
            models[0].variants.is_empty(),
            "an empty object is no levels"
        );

        assert_eq!(
            models[1].variants,
            vec!["high".to_string(), "low".to_string()]
        );
        assert_eq!(models[1].default_variant.as_deref(), Some("high"));
    }

    #[test]
    fn the_default_level_is_high_or_the_nearest_below_it() {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        assert_eq!(
            default_variant(&v(&["low", "medium", "high"])).as_deref(),
            Some("high")
        );
        assert_eq!(
            default_variant(&v(&["low", "medium", "xhigh"])).as_deref(),
            Some("medium")
        );
        assert_eq!(
            default_variant(&v(&["max", "xhigh"])).as_deref(),
            Some("max")
        );
        assert_eq!(default_variant(&[]), None);
    }
}
