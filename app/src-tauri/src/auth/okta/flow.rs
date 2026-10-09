//! The sign-in itself: answer each remediation, then replay the SAML hop.

use super::http::{agent, bootstrap, host_of, idx, walk, Jar, LoginError, MAX_STEPS, SSO_HOST};
use super::remediation::{
    challenged_factor, check_messages, option_labels, remediation, remediation_names,
    select_payload, summarise, Factor,
};
use super::store::Credentials;
use super::totp::{totp_now, totp_seconds_remaining};

/// Sign in headlessly and persist the resulting Canvas session cookie,
/// returning the cookie header. Driven by whichever remediations Okta offers,
/// since factor order is a policy setting. Only `sign_in` calls this.
pub(super) fn attempt_sign_in(
    data_dir: &std::path::Path,
    creds: &Credentials,
) -> Result<String, LoginError> {
    let mut jar = Jar::default();

    let (state_token, saml_url) = bootstrap(&mut jar)?;
    let idx_base = format!("https://{SSO_HOST}/idp/idx");

    // Introspect takes `stateToken`, but some configurations accept only
    // `stateHandle` here, so retry with that.
    let introspect = format!("{idx_base}/introspect");
    let mut state = idx(
        &mut jar,
        &introspect,
        serde_json::json!({ "stateToken": state_token }),
    )?;
    if state["stateHandle"].as_str().is_none() {
        state = idx(
            &mut jar,
            &introspect,
            serde_json::json!({ "stateHandle": state_token }),
        )?;
    }
    if state["stateHandle"].as_str().is_none() {
        return Err(LoginError::Unexpected(format!(
            "Okta would not open a sign-in transaction — it said: {}",
            summarise(&state)
        )));
    }

    let mut password_done = false;
    let mut identified = false;
    let mut switched_to: Option<Factor> = None;

    for _ in 0..MAX_STEPS {
        if state.get("successWithInteractionCode").is_some() || state.get("success").is_some() {
            break;
        }
        let Some(handle) = state["stateHandle"].as_str().map(str::to_string) else {
            return Err(LoginError::Unexpected(
                "IDX response carried no stateHandle — the session expired mid sign-in".into(),
            ));
        };
        let names = remediation_names(&state);

        // Username, and on some policies the password with it.
        if !identified {
            if let Some(rem) = remediation(&state, "identify") {
                let href = rem["href"]
                    .as_str()
                    .unwrap_or(&format!("{idx_base}/identify"))
                    .to_string();
                let takes_password = rem["value"]
                    .as_array()
                    .is_some_and(|f| f.iter().any(|x| x["name"].as_str() == Some("credentials")));

                let mut body = serde_json::json!({
                    "stateHandle": handle,
                    "identifier": creds.username,
                });
                if takes_password {
                    body["credentials"] = serde_json::json!({ "passcode": creds.password });
                }
                state = idx(&mut jar, &href, body)?;
                check_messages(&state, takes_password.then_some(Factor::Password))?;
                identified = true;
                password_done |= takes_password;
                continue;
            }
        }

        // Answer the challenge BEFORE considering the chooser: OIE offers
        // `select-authenticator-authenticate` alongside every challenge, and
        // taking it re-picks the same authenticator forever.
        if let Some(rem) = remediation(&state, "challenge-authenticator") {
            if let Some(kind) = challenged_factor(&state, password_done) {
                let passcode = match kind {
                    Factor::Password => creds.password.clone(),
                    Factor::Totp => {
                        wait_for_fresh_code();
                        totp_now(&creds.totp_secret).map_err(LoginError::Unexpected)?
                    }
                };
                let href = rem["href"]
                    .as_str()
                    .unwrap_or(&format!("{idx_base}/challenge/answer"))
                    .to_string();
                state = idx(
                    &mut jar,
                    &href,
                    serde_json::json!({ "stateHandle": handle, "credentials": { "passcode": passcode } }),
                )?;
                check_messages(&state, Some(kind))?;
                if kind == Factor::Password {
                    password_done = true;
                }
                continue;
            }
            // Challenged for push or a security key: fall through to the
            // chooser and switch to something answerable.
        }

        // Pick the next factor: password first, then TOTP.
        if let Some(rem) = remediation(&state, "select-authenticator-authenticate") {
            let want = if password_done {
                Factor::Totp
            } else {
                Factor::Password
            };
            // Selecting the same factor twice means the answer never landed.
            if switched_to == Some(want) {
                return Err(LoginError::Unexpected(format!(
                    "Okta re-offered the factor chooser after {want:?} was already selected"
                )));
            }
            let payload = select_payload(rem, want)
                .ok_or_else(|| LoginError::UnsupportedFactor(option_labels(rem)))?;
            let href = rem["href"]
                .as_str()
                .unwrap_or(&format!("{idx_base}/challenge"))
                .to_string();
            state = idx(
                &mut jar,
                &href,
                serde_json::json!({ "stateHandle": handle, "authenticator": payload }),
            )?;
            check_messages(&state, None)?;
            switched_to = Some(want);
            continue;
        }

        return Err(LoginError::UnsupportedFactor(if names.is_empty() {
            option_labels(&state)
        } else {
            names
        }));
    }

    if state.get("success").is_none() && state.get("successWithInteractionCode").is_none() {
        return Err(LoginError::Unexpected(format!(
            "sign-in did not complete in {MAX_STEPS} steps (last offered: {})",
            remediation_names(&state).join(", ")
        )));
    }

    // The success href is what actually sets Okta's session cookie.
    if let Some(href) = state
        .pointer("/success/href")
        .or_else(|| state.pointer("/successWithInteractionCode/href"))
        .and_then(|v| v.as_str())
    {
        walk(&mut jar, href, 10).ok();
    }

    let cookie = complete_saml(&mut jar, &saml_url)?;

    // Prove the cookie authenticates before overwriting one that may still
    // be good.
    let name = verify(&cookie)?;

    let path = crate::library::paths::cookie_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    crate::library::paths::write_private(&path, &cookie)
        .map_err(|e| LoginError::Unexpected(format!("could not save the session cookie: {e}")))?;
    // Okta's session too, so an in-app browser page that redirects to SSO
    // passes straight through (`browser::seed_sessions`).
    let sso = jar.header(SSO_HOST);
    if !sso.is_empty() {
        crate::library::paths::write_private(
            &crate::library::paths::sso_cookie_path(data_dir),
            &sso,
        )
        .ok();
    }
    eprintln!("[oculus] automated sign-in succeeded — Canvas accepted the session as {name}");
    Ok(cookie)
}

/// Confirm Canvas accepts the freshly minted cookie, returning the account
/// name it reports.
fn verify(cookie: &str) -> Result<String, LoginError> {
    let url = format!("{}/api/v1/users/self", crate::library::paths::CANVAS_BASE);
    let resp = agent().get(&url).set("Cookie", cookie).call();
    match resp {
        Ok(r) if r.status() == 200 => {
            let body = r.into_string().unwrap_or_default();
            let v: serde_json::Value = serde_json::from_str(&body)
                .map_err(|e| LoginError::Unexpected(format!("unreadable Canvas reply: {e}")))?;
            Ok(v["name"]
                .as_str()
                .or_else(|| v["short_name"].as_str())
                .unwrap_or("Canvas user")
                .to_string())
        }
        Ok(r) | Err(ureq::Error::Status(_, r)) => Err(LoginError::Unexpected(format!(
            "the SAML round trip finished but Canvas rejected the session (HTTP {}) — the assertion was not accepted",
            r.status()
        ))),
        Err(e) => Err(LoginError::Network(e.to_string())),
    }
}

/// Okta rejects a replayed code and repeated failures trip the lockout, so
/// never spend one that is about to expire.
fn wait_for_fresh_code() {
    let left = totp_seconds_remaining();
    if left < 3 {
        std::thread::sleep(std::time::Duration::from_secs(left + 1));
    }
}

/// With an Okta session, replay the SAML app URL and POST the auto-submit
/// assertion form to Canvas for a `canvas_session` cookie.
fn complete_saml(jar: &mut Jar, saml_url: &str) -> Result<String, LoginError> {
    let canvas_host = url::Url::parse(crate::library::paths::CANVAS_BASE)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();

    let agent = agent();
    let mut url = url::Url::parse(saml_url).map_err(|e| LoginError::Unexpected(e.to_string()))?;
    let mut form: Option<String> = None;
    let mut posted_assertion = false;

    for _ in 0..10 {
        let host = host_of(&url);
        let mut req = match form {
            Some(_) => agent
                .post(url.as_str())
                .set("Content-Type", "application/x-www-form-urlencoded"),
            None => agent.get(url.as_str()),
        };
        let cookie = jar.header(&host);
        if !cookie.is_empty() {
            req = req.set("Cookie", &cookie);
        }

        let resp = match form.take() {
            Some(body) => req.send_string(&body),
            None => req.call(),
        };
        let resp = match resp {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(LoginError::Network(e.to_string())),
        };
        jar.absorb(&host, &resp);

        if (300..400).contains(&resp.status()) {
            let loc = resp
                .header("Location")
                .ok_or_else(|| LoginError::Unexpected("redirect without Location".into()))?;
            url = url
                .join(loc)
                .map_err(|e| LoginError::Unexpected(format!("bad redirect target: {e}")))?;
            continue;
        }

        // Only after the assertion is posted: Canvas hands an anonymous
        // `canvas_session` to every first visitor.
        if posted_assertion && jar.has(&canvas_host, "canvas_session") {
            return Ok(jar.header(&canvas_host));
        }

        let body = resp.into_string().unwrap_or_default();
        let (action, fields) = parse_saml_form(&body)
            .ok_or_else(|| LoginError::Unexpected(format!("no SAML assertion form at {url}")))?;
        url = url
            .join(&action)
            .map_err(|e| LoginError::Unexpected(format!("bad form action: {e}")))?;
        form = Some(
            url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(&fields)
                .finish(),
        );
        posted_assertion = true;
    }
    Err(LoginError::Unexpected(
        "the SAML assertion never reached Canvas".into(),
    ))
}

/// The form carrying a `SAMLResponse` field; Okta's pages carry others too.
pub(super) fn parse_saml_form(html: &str) -> Option<(String, Vec<(String, String)>)> {
    let doc = scraper::Html::parse_document(html);
    let form_sel = scraper::Selector::parse("form").ok()?;
    let input_sel = scraper::Selector::parse("input").ok()?;

    for form in doc.select(&form_sel) {
        let fields: Vec<(String, String)> = form
            .select(&input_sel)
            .filter_map(|i| {
                Some((
                    i.value().attr("name")?.to_string(),
                    i.value().attr("value").unwrap_or("").to_string(),
                ))
            })
            .collect();
        if fields.iter().any(|(k, _)| k == "SAMLResponse") {
            return Some((form.value().attr("action")?.to_string(), fields));
        }
    }
    None
}
