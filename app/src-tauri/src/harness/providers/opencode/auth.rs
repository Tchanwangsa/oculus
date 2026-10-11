//! Provider credentials. This server is the supported door to opencode's
//! `auth.json`, and runs the OAuth loopback listener in-process. `connected`
//! comes from instance state that a credential write does not invalidate
//! (1.18.2), so a write is followed by [`OpencodeServer::refresh`].

use serde_json::{json, Value};

use super::http::urlencode;
use super::redact::redact;
use super::server::OpencodeServer;
use super::types::{AuthMethod, AuthOption, AuthPrompt, AuthWhen, Authorization, ProviderInfo};

impl OpencodeServer {
    /// Every provider opencode knows, whether it is connected, and its
    /// sign-in methods.
    pub fn list_providers(&self) -> Result<Vec<ProviderInfo>, String> {
        let v = self.get(&format!("/provider?{}", self.directory_query()))?;
        // Unreadable methods degrade every provider to the plain-key form.
        let methods = self
            .get(&format!("/provider/auth?{}", self.directory_query()))
            .unwrap_or(Value::Null);
        parse_providers(&v, &methods)
    }

    /// One method's form spec, read back from the server so what is sent is
    /// filtered against what opencode declares now, not a stale webview copy.
    pub fn auth_method(&self, provider: &str, index: usize) -> Result<AuthMethod, String> {
        let v = self.get(&format!("/provider/auth?{}", self.directory_query()))?;
        let methods = parse_methods(&v[provider]);
        methods
            .into_iter()
            .find(|m| m.index == index)
            .ok_or_else(|| format!("opencode has no sign-in method {index} for {provider}"))
    }

    /// Write an API key to opencode's store. Never held, and redacted out of
    /// any error on the way back.
    pub fn set_api_key(
        &self,
        provider: &str,
        key: &str,
        metadata: &std::collections::BTreeMap<String, String>,
    ) -> Result<(), String> {
        self.put_auth(provider, api_credential(key, metadata))
            .map_err(|e| redact(&e, key))
    }

    pub fn remove_auth(&self, provider: &str) -> Result<(), String> {
        self.delete(&format!("/auth/{}", urlencode(provider)))
            .map(|_| ())
    }

    /// Start a browser flow. `method` is [`AuthMethod::index`].
    pub fn oauth_authorize(
        &self,
        provider: &str,
        method: usize,
        inputs: &std::collections::BTreeMap<String, String>,
    ) -> Result<Authorization, String> {
        let mut body = json!({ "method": method });
        if !inputs.is_empty() {
            body["inputs"] = json!(inputs);
        }
        let v = self.post(
            &format!(
                "/provider/{}/oauth/authorize?{}",
                urlencode(provider),
                self.directory_query()
            ),
            body,
        )?;
        Ok(Authorization {
            url: v["url"].as_str().unwrap_or_default().to_string(),
            method: v["method"].as_str().unwrap_or("auto").to_string(),
            instructions: v["instructions"].as_str().unwrap_or_default().to_string(),
        })
    }

    /// Finish a `code` flow with what the student pasted. An `auto` flow
    /// completes server-side and is noticed by refreshing.
    pub fn oauth_callback(
        &self,
        provider: &str,
        method: usize,
        code: Option<&str>,
    ) -> Result<(), String> {
        let mut body = json!({ "method": method });
        if let Some(c) = code {
            body["code"] = json!(c);
        }
        let v = self.post(
            &format!(
                "/provider/{}/oauth/callback?{}",
                urlencode(provider),
                self.directory_query()
            ),
            body,
        )?;
        if v.as_bool() == Some(false) {
            return Err("opencode rejected the code. It may have expired — try again.".into());
        }
        Ok(())
    }
}

/// `GET /provider` and `GET /provider/auth`, merged into Settings' rows.
/// `/provider` echoes a connected provider's real key in `key`, which
/// [`super::redact::scrub`] does not catch: that field is never read, and the response is
/// never quoted into an error.
fn parse_providers(all: &Value, methods: &Value) -> Result<Vec<ProviderInfo>, String> {
    let list = all["all"]
        .as_array()
        .ok_or("opencode /provider: no providers")?;
    let connected: std::collections::HashSet<&str> = all["connected"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let mut out: Vec<ProviderInfo> = list
        .iter()
        .filter_map(|p| {
            let id = p["id"].as_str()?.to_string();
            Some(ProviderInfo {
                name: p["name"].as_str().unwrap_or(&id).to_string(),
                source: p["source"].as_str().unwrap_or("custom").to_string(),
                env: p["env"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default(),
                model_count: p["models"].as_object().map_or(0, serde_json::Map::len),
                connected: connected.contains(id.as_str()),
                methods: parse_methods(&methods[&id]),
                id,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// The body of `PUT /auth/{id}`. A method's extra fields are `metadata`,
/// omitted when empty to match what opencode's own store writes.
fn api_credential(key: &str, metadata: &std::collections::BTreeMap<String, String>) -> Value {
    let mut body = json!({ "type": "api", "key": key });
    if !metadata.is_empty() {
        body["metadata"] = json!(metadata);
    }
    body
}

/// What a provider with no `/provider/auth` entry takes: a plain API key,
/// which opencode accepts for any provider.
pub(super) fn default_method() -> AuthMethod {
    AuthMethod {
        index: 0,
        kind: "api".into(),
        label: "API key".into(),
        prompts: Vec::new(),
    }
}

/// `/provider/auth`'s array for one provider, never dropped or reordered
/// ([`AuthMethod::index`]).
pub(super) fn parse_methods(v: &Value) -> Vec<AuthMethod> {
    let Some(arr) = v.as_array() else {
        return vec![default_method()];
    };
    let out: Vec<AuthMethod> = arr
        .iter()
        .enumerate()
        .filter_map(|(index, m)| {
            let kind = m["type"].as_str()?;
            Some(AuthMethod {
                index,
                kind: kind.to_string(),
                label: m["label"].as_str().unwrap_or(kind).to_string(),
                prompts: m["prompts"]
                    .as_array()
                    .map(|a| a.iter().filter_map(parse_prompt).collect())
                    .unwrap_or_default(),
            })
        })
        .collect();
    if out.len() == arr.len() && !out.is_empty() {
        out
    } else {
        // An unreadable method would shift every index after it.
        vec![default_method()]
    }
}

fn parse_prompt(p: &Value) -> Option<AuthPrompt> {
    let kind = p["type"].as_str()?;
    if kind != "text" && kind != "select" {
        return None;
    }
    Some(AuthPrompt {
        kind: kind.to_string(),
        key: p["key"].as_str()?.to_string(),
        message: p["message"].as_str().unwrap_or_default().to_string(),
        placeholder: p["placeholder"].as_str().map(String::from),
        options: p["options"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|o| {
                        Some(AuthOption {
                            label: o["label"].as_str()?.to_string(),
                            value: o["value"].as_str()?.to_string(),
                            hint: o["hint"].as_str().map(String::from),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        when: p["when"].as_object().and_then(|w| {
            Some(AuthWhen {
                key: w.get("key")?.as_str()?.to_string(),
                op: w.get("op")?.as_str()?.to_string(),
                value: w.get("value")?.as_str()?.to_string(),
            })
        }),
    })
}

/// Whether a prompt is on screen. An unanswered dependency reads as "".
fn prompt_visible(p: &AuthPrompt, answers: &std::collections::BTreeMap<String, String>) -> bool {
    let Some(w) = &p.when else { return true };
    let actual = answers.get(&w.key).map(String::as_str).unwrap_or("");
    match w.op.as_str() {
        "eq" => actual == w.value,
        "neq" => actual != w.value,
        _ => true,
    }
}

/// The answers that belong to a method's visible fields, applied on the way
/// out: a field hidden again still holds its value in the webview's state.
pub fn visible_answers(
    method: &AuthMethod,
    answers: &std::collections::BTreeMap<String, String>,
) -> std::collections::BTreeMap<String, String> {
    method
        .prompts
        .iter()
        .filter(|p| prompt_visible(p, answers))
        .filter_map(|p| {
            let v = answers.get(&p.key)?;
            (!v.is_empty()).then(|| (p.key.clone(), v.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::redact::redact;
    use super::*;

    /// `openai`'s three ways in, as `/provider/auth` declares them.
    const OPENAI_METHODS: &str = r#"[
      { "type": "oauth", "label": "ChatGPT Pro/Plus (browser)" },
      { "type": "oauth", "label": "ChatGPT Pro/Plus (headless)" },
      { "type": "api",   "label": "Manually enter API Key" }
    ]"#;

    /// `github-copilot`: a select, and a text field shown for one answer.
    const COPILOT_METHODS: &str = r#"[
      { "type": "oauth", "label": "Login with GitHub Copilot", "prompts": [
        { "type": "select", "key": "deploymentType", "message": "Select GitHub deployment type",
          "options": [
            { "label": "GitHub.com", "value": "github.com", "hint": "Public" },
            { "label": "GitHub Enterprise", "value": "enterprise", "hint": "Data residency or self-hosted" }
          ] },
        { "type": "text", "key": "enterpriseUrl", "message": "Enter your GitHub Enterprise URL or domain",
          "placeholder": "company.ghe.com", "when": { "key": "deploymentType", "op": "eq", "value": "enterprise" } }
      ] }
    ]"#;

    fn method(json: &str, index: usize) -> AuthMethod {
        parse_methods(&serde_json::from_str::<Value>(json).unwrap())
            .into_iter()
            .find(|m| m.index == index)
            .expect("method")
    }

    fn answers(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn method_indices_are_positions_in_the_array() {
        let ms = parse_methods(&serde_json::from_str::<Value>(OPENAI_METHODS).unwrap());
        assert_eq!(ms.len(), 3);
        assert_eq!(
            ms.iter()
                .map(|m| (m.index, m.kind.as_str()))
                .collect::<Vec<_>>(),
            vec![(0, "oauth"), (1, "oauth"), (2, "api")],
        );
        assert_eq!(ms[2].label, "Manually enter API Key");
        assert!(
            ms[0].prompts.is_empty(),
            "the browser flow asks nothing up front"
        );
    }

    #[test]
    fn an_unreadable_method_gives_up_the_list_rather_than_renumbering() {
        let ms = parse_methods(
            &serde_json::from_str::<Value>(
                r#"[{ "label": "no type here" }, { "type": "api", "label": "Key" }]"#,
            )
            .unwrap(),
        );
        assert_eq!(ms, vec![default_method()]);
    }

    #[test]
    fn a_provider_with_no_declared_method_takes_a_plain_key() {
        for v in [Value::Null, json!({}), json!([])] {
            let ms = parse_methods(&v["anthropic"]);
            assert_eq!(ms.len(), 1);
            assert_eq!(ms[0].kind, "api");
            assert!(ms[0].prompts.is_empty());
        }
    }

    #[test]
    fn a_select_and_its_dependent_field_survive_parsing() {
        let m = method(COPILOT_METHODS, 0);
        assert_eq!(m.prompts.len(), 2);
        assert_eq!(m.prompts[0].kind, "select");
        assert_eq!(m.prompts[0].options.len(), 2);
        assert_eq!(
            m.prompts[0].options[1].hint.as_deref(),
            Some("Data residency or self-hosted")
        );
        assert_eq!(m.prompts[1].kind, "text");
        assert_eq!(m.prompts[1].placeholder.as_deref(), Some("company.ghe.com"));
        let when = m.prompts[1].when.clone().expect("a condition");
        assert_eq!(
            (when.key.as_str(), when.op.as_str(), when.value.as_str()),
            ("deploymentType", "eq", "enterprise")
        );
    }

    #[test]
    fn hidden_answers_are_dropped_on_the_way_out() {
        let m = method(COPILOT_METHODS, 0);

        let enterprise = answers(&[
            ("deploymentType", "enterprise"),
            ("enterpriseUrl", "acme.ghe.com"),
        ]);
        assert_eq!(visible_answers(&m, &enterprise), enterprise);

        let switched_back = answers(&[
            ("deploymentType", "github.com"),
            ("enterpriseUrl", "acme.ghe.com"),
        ]);
        assert_eq!(
            visible_answers(&m, &switched_back),
            answers(&[("deploymentType", "github.com")])
        );

        // Unanswered reads as empty, so `eq` hides and the field waits.
        assert!(visible_answers(&m, &answers(&[])).is_empty());
    }

    #[test]
    fn answers_the_method_never_asked_for_do_not_travel() {
        let m = method(COPILOT_METHODS, 0);
        let padded = answers(&[
            ("deploymentType", "github.com"),
            ("key", "sk-something"),
            ("blank", ""),
        ]);
        assert_eq!(
            visible_answers(&m, &padded),
            answers(&[("deploymentType", "github.com")])
        );
    }

    #[test]
    fn the_credential_body_is_the_key_and_nothing_else() {
        assert_eq!(
            api_credential("sk-live", &answers(&[])),
            json!({ "type": "api", "key": "sk-live" }),
        );
        assert_eq!(
            api_credential("cf-token", &answers(&[("accountId", "abc123")])),
            json!({ "type": "api", "key": "cf-token", "metadata": { "accountId": "abc123" } }),
        );
    }

    #[test]
    fn an_error_cannot_carry_the_key_back_out() {
        let msg = redact(
            "opencode /auth/openai: HTTP 400 invalid key sk-proj-abcdef123456",
            "sk-proj-abcdef123456",
        );
        assert!(!msg.contains("sk-proj"), "{msg}");
        assert!(msg.contains("[redacted]"));
        assert_eq!(
            redact("cannot reach opencode", "abc"),
            "cannot reach opencode"
        );
    }

    #[test]
    fn a_connected_providers_key_never_leaves_rust() {
        let all = json!({
            "connected": ["xai"],
            "default": {},
            "all": [{ "id": "xai", "name": "xAI", "source": "api",
                      "env": ["XAI_API_KEY"], "key": "xai-SECRETVALUE12345",
                      "options": {}, "models": { "grok": {} } }]
        });
        let rows = parse_providers(&all, &Value::Null).unwrap();
        assert!(rows[0].connected);
        assert_eq!(rows[0].source, "api");
        let wire = serde_json::to_string(&rows).unwrap();
        assert!(!wire.contains("SECRETVALUE"), "{wire}");
    }

    #[test]
    fn providers_merge_their_connected_state_and_their_forms() {
        let all = json!({
            "connected": ["opencode", "tss-nvidia-spark"],
            "default": {},
            "all": [
                { "id": "openai", "name": "OpenAI", "source": "custom",
                  "env": ["OPENAI_API_KEY"], "options": {}, "models": { "a": {}, "b": {} } },
                { "id": "tss-nvidia-spark", "name": "TSS NVIDIA Spark", "source": "config",
                  "env": [], "options": {}, "models": { "a": {} } },
                { "id": "anthropic", "name": "Anthropic", "source": "custom",
                  "env": ["ANTHROPIC_API_KEY"], "options": {}, "models": {} }
            ]
        });
        let methods: Value =
            serde_json::from_str(&format!("{{\"openai\": {OPENAI_METHODS}}}")).unwrap();
        let rows = parse_providers(&all, &methods).unwrap();

        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["anthropic", "openai", "tss-nvidia-spark"]
        );

        let openai = &rows[1];
        assert!(!openai.connected);
        assert_eq!(openai.model_count, 2);
        assert_eq!(openai.methods.len(), 3);

        let anthropic = &rows[0];
        assert_eq!(
            anthropic.methods,
            vec![default_method()],
            "no entry means a plain key"
        );

        let spark = &rows[2];
        assert!(spark.connected);
        assert_eq!(spark.source, "config", "declared, not signed in to");
    }
}
