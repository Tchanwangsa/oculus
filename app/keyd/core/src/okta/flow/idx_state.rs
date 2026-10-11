//! Reading Okta's remediations: which factor is offered or challenged, the
//! payload that selects one, and the messages that fail an attempt.

use crate::okta::LoginError;

pub(super) fn remediations(state: &serde_json::Value) -> Vec<&serde_json::Value> {
    state["remediation"]["value"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default()
}

pub(super) fn remediation<'a>(
    state: &'a serde_json::Value,
    name: &str,
) -> Option<&'a serde_json::Value> {
    remediations(state)
        .into_iter()
        .find(|r| r["name"].as_str() == Some(name))
}

pub(super) fn remediation_names(state: &serde_json::Value) -> Vec<String> {
    remediations(state)
        .iter()
        .filter_map(|r| r["name"].as_str().map(str::to_string))
        .collect()
}

/// The `id` + `methodType` an authenticator option is selected by, read out of
/// the nested form Okta describes each option with.
pub(super) fn option_fields(option: &serde_json::Value) -> (Option<String>, Option<String>) {
    let mut id = None;
    let mut method = None;
    if let Some(fields) = option["value"]["form"]["value"].as_array() {
        for f in fields {
            match f["name"].as_str() {
                Some("id") => id = f["value"].as_str().map(str::to_string),
                Some("methodType") => method = f["value"].as_str().map(str::to_string),
                _ => {}
            }
        }
    }
    (id, method)
}

pub(super) fn authenticator_options(rem: &serde_json::Value) -> Vec<&serde_json::Value> {
    rem["value"]
        .as_array()
        .and_then(|fields| {
            fields
                .iter()
                .find(|f| f["name"].as_str() == Some("authenticator"))
        })
        .and_then(|f| f["options"].as_array())
        .map(|o| o.iter().collect())
        .unwrap_or_default()
}

pub(super) fn option_labels(rem: &serde_json::Value) -> Vec<String> {
    authenticator_options(rem)
        .iter()
        .filter_map(|o| o["label"].as_str().map(str::to_string))
        .collect()
}

/// Which factor we are looking for at this point in the flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Factor {
    Password,
    Totp,
}

/// Build the `authenticator` payload that selects `want`. TOTP is matched by
/// label first: Okta Verify also advertises `methodType: otp`, with a
/// different seed.
pub(super) fn select_payload(rem: &serde_json::Value, want: Factor) -> Option<serde_json::Value> {
    let options = authenticator_options(rem);
    let pick = |pred: &dyn Fn(&str, &str) -> bool| -> Option<serde_json::Value> {
        options.iter().find_map(|o| {
            let (id, method) = option_fields(o);
            let (id, method) = (id?, method.unwrap_or_default());
            let label = o["label"].as_str().unwrap_or("");
            pred(&label.to_ascii_lowercase(), &method).then(|| match want {
                Factor::Password => serde_json::json!({ "id": id }),
                Factor::Totp => serde_json::json!({ "id": id, "methodType": "otp" }),
            })
        })
    };

    match want {
        Factor::Password => pick(&|_, m| m == "password"),
        Factor::Totp => pick(&|l, m| m == "otp" && l.contains("google"))
            .or_else(|| pick(&|l, m| m == "otp" && !l.contains("okta verify")))
            .or_else(|| pick(&|_, m| m == "otp")),
    }
}

/// Whatever an IDX response says about itself, for diagnostics. Okta reports
/// some failures as `messages`, others as a bare `errorSummary`.
pub(super) fn summarise(state: &serde_json::Value) -> String {
    if let Some(messages) = state["messages"]["value"].as_array() {
        let joined: Vec<&str> = messages
            .iter()
            .filter_map(|m| m["message"].as_str())
            .collect();
        if !joined.is_empty() {
            return joined.join(" ");
        }
    }
    state["errorSummary"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| "nothing intelligible".to_string())
}

/// Which factor the pending challenge is for, or `None` when it is one we
/// cannot answer and the caller should switch via the chooser. Falls back on
/// flow position when Okta names no `currentAuthenticator`.
pub(super) fn challenged_factor(state: &serde_json::Value, password_done: bool) -> Option<Factor> {
    let current = ["currentAuthenticator", "currentAuthenticatorEnrollment"]
        .iter()
        .map(|k| &state[*k])
        .find(|v| !v.is_null())
        .map(|v| v.get("value").unwrap_or(v));

    let Some(cur) = current else {
        return Some(if password_done {
            Factor::Totp
        } else {
            Factor::Password
        });
    };

    let key = cur["key"].as_str().unwrap_or("");
    let methods: Vec<&str> = cur["methods"]
        .as_array()
        .map(|a| a.iter().filter_map(|m| m["type"].as_str()).collect())
        .unwrap_or_default();

    if key == "okta_password" || methods.contains(&"password") {
        // Re-offered after it was accepted: Okta is moving on.
        return (!password_done).then_some(Factor::Password);
    }
    if key == "google_otp" {
        return Some(Factor::Totp);
    }
    // Okta Verify advertises `totp` too, but its seed is not the one we hold.
    if key != "okta_verify" && methods.iter().any(|m| *m == "otp" || *m == "totp") {
        return Some(Factor::Totp);
    }
    None
}

/// Turn an error carried in an IDX response into the right `LoginError`.
pub(super) fn check_messages(
    state: &serde_json::Value,
    answering: Option<Factor>,
) -> Result<(), LoginError> {
    let Some(messages) = state["messages"]["value"].as_array() else {
        return Ok(());
    };
    let errors: Vec<String> = messages
        .iter()
        .filter(|m| m["class"].as_str() != Some("INFO"))
        .filter_map(|m| m["message"].as_str().map(str::to_string))
        .collect();
    if errors.is_empty() {
        return Ok(());
    }
    let text = errors.join(" ");
    let lower = text.to_ascii_lowercase();

    if lower.contains("locked") || lower.contains("suspended") || lower.contains("too many") {
        return Err(LoginError::Locked(text));
    }
    Err(match answering {
        Some(Factor::Totp) => LoginError::BadTotp(text),
        Some(Factor::Password) => LoginError::BadPassword(text),
        // Before a factor is answered, the only credential in play is the
        // username/password pair on the identify form.
        None => LoginError::BadPassword(text),
    })
}
