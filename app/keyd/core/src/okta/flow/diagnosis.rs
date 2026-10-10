//! What the sign-in page looks like from here, for when the flow fails.

use super::http::{idx, state_token_candidates, walk};
use super::idx_state::{remediation_names, summarise};
use crate::okta::jar::Jar;
use crate::okta::Env;

/// What the sign-in page looks like from here, for when the flow fails.
/// Reports shapes and lengths, never values: a state token is a live
/// credential.
pub(in crate::okta) fn diagnose(env: &Env) -> String {
    let mut out = String::new();
    let mut jar = Jar::default();
    let sso_host = env.sso_host();

    let start = format!("{}/login/saml", env.canvas_base);
    let (landed, body) = match walk(&mut jar, &start, 10) {
        Ok(v) => v,
        Err(e) => return format!("could not reach the sign-in page: {e}\n"),
    };

    out.push_str(&format!("landed on   {landed}\n"));
    out.push_str(&format!("page size   {} bytes\n", body.len()));
    out.push_str(&format!("okta cookies {}\n", jar.count(&sso_host)));

    let markers = [
        "stateToken",
        "interactionHandle",
        "interaction_code",
        "okta-signin-widget",
        "signin-container",
        "OktaUtil",
    ];
    let seen: Vec<&str> = markers
        .iter()
        .copied()
        .filter(|m| body.contains(m))
        .collect();
    out.push_str(&format!(
        "markers     {}\n",
        if seen.is_empty() {
            "none".to_string()
        } else {
            seen.join(", ")
        }
    ));

    let candidates = state_token_candidates(&body);
    if candidates.is_empty() {
        out.push_str("state token none matched the expected shape\n");
    } else {
        for (i, c) in candidates.iter().enumerate() {
            out.push_str(&format!(
                "state token #{i}  {} chars, starts {:?}\n",
                c.len(),
                &c[..c.len().min(6)]
            ));
        }
    }

    // Try the handshake itself — its answer is the actual diagnosis.
    let Some(token) = candidates.first() else {
        return out;
    };
    let introspect = format!("{}/idp/idx/introspect", env.sso_base);
    for field in ["stateToken", "stateHandle"] {
        match idx(
            &mut jar,
            env,
            &introspect,
            serde_json::json!({ field: token }),
        ) {
            Ok(state) => {
                let names = remediation_names(&state);
                out.push_str(&format!(
                    "introspect  {field}: {}\n",
                    if state["stateHandle"].as_str().is_some() {
                        format!("ok — offers [{}]", names.join(", "))
                    } else {
                        format!("refused — {}", summarise(&state))
                    }
                ));
            }
            Err(e) => out.push_str(&format!("introspect  {field}: {e}\n")),
        }
    }
    out
}
