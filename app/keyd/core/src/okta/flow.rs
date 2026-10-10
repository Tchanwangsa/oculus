//! The sign-in itself: Canvas's SAML start, Okta's IDX state machine, the
//! SAML assertion back to Canvas and the proof that Canvas accepts the
//! session. Every origin comes from the `Env`; the guard is not here.

use super::jar::Jar;
use super::totp::{seconds_remaining, totp_code};
use super::{Credentials, Env, LoginError};
use crate::clock::Clock;
use crate::paths;

// ── HTTP plumbing ────────────────────────────────────────────────────────────

const IDX_MEDIA: &str = "application/ion+json; okta-version=1.0.0";
/// Hygiene, not a known requirement: every leg here impersonates a browser.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);
/// Stops a policy we did not anticipate from looping.
const MAX_STEPS: usize = 12;

/// An agent that lives for one request. The app enables ureq's `cookies`
/// feature for Echo360, and an agent with it jars `Set-Cookie` and adds its
/// own `Cookie` header beside the jar's; keyd has no such feature. A fresh
/// agent has nothing to replay, so both builds send the same request.
fn agent() -> ureq::Agent {
    // Redirects are walked by hand so cookies can be filed per host.
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
}

/// `identity` is asked for by name: the `gzip` feature (also the app's alone)
/// would otherwise add `Accept-Encoding: gzip`.
fn request(method: &str, url: &str) -> ureq::Request {
    agent()
        .request(method, url)
        .set("Accept-Encoding", "identity")
}

fn host_of(u: &url::Url) -> String {
    u.host_str().unwrap_or_default().to_string()
}

/// Follow 3xx from `start`, filing cookies per host, until a non-redirect.
/// Returns where it landed and the body.
fn walk(jar: &mut Jar, start: &str, max: usize) -> Result<(url::Url, String), LoginError> {
    let mut url = url::Url::parse(start).map_err(|e| LoginError::Unexpected(e.to_string()))?;

    for _ in 0..max {
        let host = host_of(&url);
        let mut req = request("GET", url.as_str());
        let cookie = jar.header(&host);
        if !cookie.is_empty() {
            req = req.set("Cookie", &cookie);
        }
        let resp = match req.call() {
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
        let body = resp.into_string().unwrap_or_default();
        return Ok((url, body));
    }
    Err(LoginError::Unexpected(
        "redirect loop during sign-in".into(),
    ))
}

/// Start the SAML flow and pull the IDX state token out of the login page.
fn bootstrap(jar: &mut Jar, env: &Env) -> Result<(String, String), LoginError> {
    let start = format!("{}/login/saml", env.canvas_base);
    let (landed, body) = walk(jar, &start, 10)?;

    let sso_host = env.sso_host();
    if host_of(&landed) != sso_host {
        return Err(LoginError::Unexpected(format!(
            "SAML start landed on {landed} instead of {sso_host}"
        )));
    }
    let token = extract_state_token(&body).ok_or_else(|| {
        LoginError::Unexpected(
            "no state token on the Okta login page — the sign-in widget may have changed".into(),
        )
    })?;
    Ok((token, landed.to_string()))
}

fn extract_state_token(html: &str) -> Option<String> {
    state_token_candidates(html).into_iter().next()
}

/// Every `stateToken`-shaped value on the page. Inline script mentions the
/// name before the config assigns it, so each candidate must be a quoted
/// value of plausible shape, not just the first mention.
fn state_token_candidates(html: &str) -> Vec<String> {
    const KEY: &str = "stateToken";
    let mut out: Vec<String> = Vec::new();
    let mut from = 0;

    while let Some(i) = html[from..].find(KEY) {
        let at = from + i + KEY.len();
        from = at;
        let tail = &html[at..];

        // Step over the separator: `":"`, `: '`, `= "`, `='`.
        let sep: String = tail
            .chars()
            .take_while(|c| matches!(c, '"' | '\'' | ':' | '=' | ' ' | '\t' | '\n' | '\r'))
            .collect();
        // The separator must end on the quote that opens the value.
        let Some(quote) = sep.chars().rev().find(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let rest = &tail[sep.len()..];
        let Some(end) = rest.find(quote) else {
            continue;
        };

        let raw = unescape_js(&rest[..end]);
        if looks_like_state_token(&raw) && !out.contains(&raw) {
            out.push(raw);
        }
    }
    out
}

/// Okta embeds the token in a JS string literal and escapes `-` as `\x2D`,
/// so a raw grab yields a token the API rejects as malformed.
fn unescape_js(s: &str) -> String {
    s.replace("\\x2D", "-")
        .replace("\\x2d", "-")
        .replace("\\u002D", "-")
        .replace("\\u002d", "-")
        .replace("\\x2F", "/")
        .replace("\\/", "/")
}

fn looks_like_state_token(s: &str) -> bool {
    s.len() >= 20
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '~'))
}

/// One IDX call. A non-2xx body is parsed, not thrown: Okta explains its
/// 400/401s there.
fn idx(
    jar: &mut Jar,
    env: &Env,
    url: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, LoginError> {
    let sso_host = env.sso_host();
    let resp = request("POST", url)
        .set("Accept", IDX_MEDIA)
        .set("Content-Type", IDX_MEDIA)
        .set("Cookie", &jar.header(&sso_host))
        .send_string(&body.to_string());

    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(e) => return Err(LoginError::Network(e.to_string())),
    };
    jar.absorb(&sso_host, &resp);
    let text = resp
        .into_string()
        .map_err(|e| LoginError::Network(e.to_string()))?;
    serde_json::from_str(&text)
        .map_err(|e| LoginError::Unexpected(format!("unreadable IDX response: {e}")))
}

// ── Remediation helpers ──────────────────────────────────────────────────────

fn remediations(state: &serde_json::Value) -> Vec<&serde_json::Value> {
    state["remediation"]["value"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default()
}

fn remediation<'a>(state: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    remediations(state)
        .into_iter()
        .find(|r| r["name"].as_str() == Some(name))
}

fn remediation_names(state: &serde_json::Value) -> Vec<String> {
    remediations(state)
        .iter()
        .filter_map(|r| r["name"].as_str().map(str::to_string))
        .collect()
}

/// The `id` + `methodType` an authenticator option is selected by, read out of
/// the nested form Okta describes each option with.
fn option_fields(option: &serde_json::Value) -> (Option<String>, Option<String>) {
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

fn authenticator_options(rem: &serde_json::Value) -> Vec<&serde_json::Value> {
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

fn option_labels(rem: &serde_json::Value) -> Vec<String> {
    authenticator_options(rem)
        .iter()
        .filter_map(|o| o["label"].as_str().map(str::to_string))
        .collect()
}

/// Which factor we are looking for at this point in the flow.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Factor {
    Password,
    Totp,
}

/// Build the `authenticator` payload that selects `want`. TOTP is matched by
/// label first: Okta Verify also advertises `methodType: otp`, with a
/// different seed.
fn select_payload(rem: &serde_json::Value, want: Factor) -> Option<serde_json::Value> {
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
fn summarise(state: &serde_json::Value) -> String {
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
fn challenged_factor(state: &serde_json::Value, password_done: bool) -> Option<Factor> {
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
fn check_messages(state: &serde_json::Value, answering: Option<Factor>) -> Result<(), LoginError> {
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

// ── The flow ─────────────────────────────────────────────────────────────────

/// Sign in headlessly and persist the resulting Canvas session cookie,
/// returning the cookie header. Driven by whichever remediations Okta offers,
/// since factor order is a policy setting. Only `sign_in` calls this.
pub(super) fn attempt_sign_in(env: &Env, creds: &Credentials) -> Result<String, LoginError> {
    let mut jar = Jar::default();

    let (state_token, saml_url) = bootstrap(&mut jar, env)?;
    let sso_host = env.sso_host();
    let idx_base = format!("{}/idp/idx", env.sso_base);

    // Introspect takes `stateToken`, but some configurations accept only
    // `stateHandle` here, so retry with that.
    let introspect = format!("{idx_base}/introspect");
    let mut state = idx(
        &mut jar,
        env,
        &introspect,
        serde_json::json!({ "stateToken": state_token }),
    )?;
    if state["stateHandle"].as_str().is_none() {
        state = idx(
            &mut jar,
            env,
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
                state = idx(&mut jar, env, &href, body)?;
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
                        wait_for_fresh_code(&env.now);
                        totp_code(&creds.totp_secret, (env.now)())
                            .map_err(LoginError::Unexpected)?
                    }
                };
                let href = rem["href"]
                    .as_str()
                    .unwrap_or(&format!("{idx_base}/challenge/answer"))
                    .to_string();
                state = idx(
                    &mut jar,
                    env,
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
                env,
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

    let cookie = complete_saml(&mut jar, env, &saml_url)?;

    // Prove the cookie authenticates before overwriting one that may still
    // be good.
    let name = verify(env, &cookie)?;

    let path = paths::cookie(&env.data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    paths::write_private(&path, &cookie)
        .map_err(|e| LoginError::Unexpected(format!("could not save the session cookie: {e}")))?;
    // Okta's session too, so an in-app browser page that redirects to SSO
    // passes straight through (`browser::seed_sessions`).
    let sso = jar.header(&sso_host);
    if !sso.is_empty() {
        paths::write_private(&paths::sso_cookie(&env.data_dir), &sso).ok();
    }
    eprintln!("[oculus] automated sign-in succeeded — Canvas accepted the session as {name}");
    Ok(cookie)
}

/// Confirm Canvas accepts the freshly minted cookie, returning the account
/// name it reports.
fn verify(env: &Env, cookie: &str) -> Result<String, LoginError> {
    let url = format!("{}/api/v1/users/self", env.canvas_base);
    let resp = request("GET", &url).set("Cookie", cookie).call();
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
fn wait_for_fresh_code(now: &Clock) {
    let left = seconds_remaining(now());
    if left < 3 {
        std::thread::sleep(std::time::Duration::from_secs(left + 1));
    }
}

/// With an Okta session, replay the SAML app URL and POST the auto-submit
/// assertion form to Canvas for a `canvas_session` cookie.
fn complete_saml(jar: &mut Jar, env: &Env, saml_url: &str) -> Result<String, LoginError> {
    let canvas_host = url::Url::parse(&env.canvas_base)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();

    let mut url = url::Url::parse(saml_url).map_err(|e| LoginError::Unexpected(e.to_string()))?;
    let mut form: Option<String> = None;
    let mut posted_assertion = false;

    for _ in 0..10 {
        let host = host_of(&url);
        let mut req = match form {
            Some(_) => request("POST", url.as_str())
                .set("Content-Type", "application/x-www-form-urlencoded"),
            None => request("GET", url.as_str()),
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
fn parse_saml_form(html: &str) -> Option<(String, Vec<(String, String)>)> {
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

// ── Diagnosis ────────────────────────────────────────────────────────────────

/// What the sign-in page looks like from here, for when the flow fails.
/// Reports shapes and lengths, never values: a state token is a live
/// credential.
pub(super) fn diagnose(env: &Env) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A real token is ~40+ chars; the JS literal escapes `-` as `\x2D`.
    const REAL_TOKEN: &str = "02.id.7Kx9pQ2mNvL4tR8wZ1yB3cD5fG6hJ0kM-aS-eU";

    #[test]
    fn unescapes_the_state_token_okta_embeds() {
        let html = format!(
            r#"<script>var config = {{"stateToken":"{}"}};</script>"#,
            REAL_TOKEN.replace('-', r"\x2D")
        );
        assert_eq!(extract_state_token(&html).unwrap(), REAL_TOKEN);
    }

    /// Inline script names `stateToken` before the config assigns it.
    #[test]
    fn skips_mentions_that_are_not_the_value() {
        let html = format!(
            r#"<script>
                 if (stateToken) {{ render(stateToken); }}
                 var x = {{"stateToken":""}};
                 var config = {{"stateToken":"{REAL_TOKEN}"}};
               </script>"#
        );
        assert_eq!(extract_state_token(&html).unwrap(), REAL_TOKEN);
        assert_eq!(state_token_candidates(&html), vec![REAL_TOKEN.to_string()]);
    }

    #[test]
    fn accepts_the_single_quoted_assignment_form() {
        let html = format!("<script>var stateToken = '{REAL_TOKEN}';</script>");
        assert_eq!(extract_state_token(&html).unwrap(), REAL_TOKEN);
    }

    #[test]
    fn no_state_token_is_not_a_panic() {
        assert!(extract_state_token("<html><body>maintenance</body></html>").is_none());
    }

    fn select_rem(options: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "name": "select-authenticator-authenticate",
            "href": "https://sso.unimelb.edu.au/idp/idx/challenge",
            "value": [{ "name": "authenticator", "type": "object", "options": options }]
        })
    }

    fn option(label: &str, id: &str, method: &str) -> serde_json::Value {
        serde_json::json!({
            "label": label,
            "value": { "form": { "value": [
                { "name": "id", "value": id },
                { "name": "methodType", "value": method }
            ]}}
        })
    }

    #[test]
    fn picks_password_then_google_authenticator() {
        let rem = select_rem(serde_json::json!([
            option("Password", "aut_pw", "password"),
            option("Okta Verify", "aut_ov", "otp"),
            option("Google Authenticator", "aut_ga", "otp"),
        ]));

        assert_eq!(
            select_payload(&rem, Factor::Password).unwrap()["id"],
            "aut_pw"
        );

        // Google Authenticator wins over Okta Verify's TOTP (different seed).
        let totp = select_payload(&rem, Factor::Totp).unwrap();
        assert_eq!(totp["id"], "aut_ga");
        assert_eq!(totp["methodType"], "otp");
    }

    #[test]
    fn a_push_only_account_reports_what_it_was_offered() {
        let rem = select_rem(serde_json::json!([
            option("Get a push notification", "aut_push", "push"),
            option("Security Key or Biometric", "aut_wa", "webauthn"),
        ]));
        assert!(select_payload(&rem, Factor::Totp).is_none());
        assert_eq!(
            option_labels(&rem),
            vec!["Get a push notification", "Security Key or Biometric"]
        );
    }

    fn challenge_state(key: &str, methods: &[&str]) -> serde_json::Value {
        serde_json::json!({
            "currentAuthenticator": { "value": {
                "key": key,
                "methods": methods.iter().map(|m| serde_json::json!({"type": m})).collect::<Vec<_>>()
            }}
        })
    }

    /// Answered on what Okta says it is challenging, not on flow position.
    #[test]
    fn answers_the_factor_okta_says_it_is_challenging() {
        let pw = challenge_state("okta_password", &["password"]);
        assert_eq!(challenged_factor(&pw, false), Some(Factor::Password));
        // Already answered — do not resend it, move on to the second factor.
        assert_eq!(challenged_factor(&pw, true), None);

        let ga = challenge_state("google_otp", &["otp"]);
        assert_eq!(challenged_factor(&ga, true), Some(Factor::Totp));
    }

    #[test]
    fn a_push_challenge_is_not_answerable() {
        let push = challenge_state("okta_verify", &["push"]);
        assert_eq!(challenged_factor(&push, true), None);
        // Okta Verify also advertises totp, but its seed is not ours.
        let ov_totp = challenge_state("okta_verify", &["totp", "push"]);
        assert_eq!(challenged_factor(&ov_totp, true), None);
        let key = challenge_state("webauthn", &["webauthn"]);
        assert_eq!(challenged_factor(&key, true), None);
    }

    /// When Okta describes no authenticator, fall back on flow position.
    #[test]
    fn an_undescribed_challenge_falls_back_to_flow_position() {
        let bare = serde_json::json!({});
        assert_eq!(challenged_factor(&bare, false), Some(Factor::Password));
        assert_eq!(challenged_factor(&bare, true), Some(Factor::Totp));
    }

    #[test]
    fn a_wrong_code_is_a_totp_error_not_a_password_error() {
        let state = serde_json::json!({
            "messages": { "value": [{ "class": "ERROR", "message": "Invalid code. Try again." }] }
        });
        assert!(matches!(
            check_messages(&state, Some(Factor::Totp)),
            Err(LoginError::BadTotp(_))
        ));
        assert!(matches!(
            check_messages(&state, Some(Factor::Password)),
            Err(LoginError::BadPassword(_))
        ));
    }

    #[test]
    fn a_lockout_outranks_the_factor_it_was_reported_on() {
        let state = serde_json::json!({
            "messages": { "value": [{ "class": "ERROR", "message": "Your account is locked." }] }
        });
        assert!(matches!(
            check_messages(&state, Some(Factor::Totp)),
            Err(LoginError::Locked(_))
        ));
    }

    #[test]
    fn informational_messages_are_not_failures() {
        let state = serde_json::json!({
            "messages": { "value": [{ "class": "INFO", "message": "Verify with your password" }] }
        });
        assert!(check_messages(&state, None).is_ok());
    }

    #[test]
    fn finds_the_assertion_form_among_decoys() {
        let html = r#"
            <form action="/search"><input name="q" value=""/></form>
            <form method="post" action="https://canvas.lms.unimelb.edu.au/login/saml">
              <input type="hidden" name="SAMLResponse" value="PHNhbWw+"/>
              <input type="hidden" name="RelayState" value="rs123"/>
            </form>"#;
        let (action, fields) = parse_saml_form(html).unwrap();
        assert_eq!(action, "https://canvas.lms.unimelb.edu.au/login/saml");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0], ("SAMLResponse".into(), "PHNhbWw+".into()));
    }

    use crate::test_support::{Answer, FakeOrigin, Hit};

    /// `/start` redirects to `/next` setting two cookies; `/next` answers
    /// plain text whatever `Accept-Encoding` asked for.
    fn setting_origin() -> FakeOrigin {
        FakeOrigin::start(|hit: &Hit| match hit.path.as_str() {
            "/start" => Answer {
                status: 302,
                headers: vec![
                    ("Location", "/next".to_string()),
                    ("Set-Cookie", "a=1; Path=/; HttpOnly".to_string()),
                    ("Set-Cookie", "b=2; Path=/other".to_string()),
                ],
                body: Vec::new(),
            },
            _ => Answer {
                status: 200,
                headers: Vec::new(),
                body: b"landed".to_vec(),
            },
        })
    }

    fn cookie_headers(hit: &Hit) -> Vec<&str> {
        hit.headers
            .iter()
            .filter(|(k, _)| k == "cookie")
            .map(|(_, v)| v.as_str())
            .collect()
    }

    #[test]
    fn a_set_cookie_is_replayed_only_by_the_jar_whatever_ureq_features_are_on() {
        let origin = setting_origin();
        let mut jar = Jar::default();
        let (landed, body) = walk(&mut jar, &format!("{}/start", origin.origin), 5).unwrap();
        assert_eq!(landed.path(), "/next");
        assert_eq!(body, "landed");

        let hits = origin.hits();
        assert_eq!(hits.len(), 2);
        assert!(cookie_headers(&hits[0]).is_empty());
        // One header, the jar's. A ureq cookie store would add a second.
        assert_eq!(cookie_headers(&hits[1]), ["a=1; b=2"]);

        // A later walk with an empty jar carries nothing from the first.
        walk(&mut Jar::default(), &format!("{}/next", origin.origin), 5).unwrap();
        assert!(cookie_headers(&origin.hits()[2]).is_empty());
    }

    #[test]
    fn every_request_asks_for_an_uncompressed_answer() {
        let origin = setting_origin();
        walk(&mut Jar::default(), &format!("{}/start", origin.origin), 5).unwrap();
        let cookie = "canvas_session=x";
        let url = format!("{}/next", origin.origin);
        assert!(request("GET", &url).set("Cookie", cookie).call().is_ok());
        for hit in origin.hits() {
            assert_eq!(hit.header("accept-encoding"), Some("identity"));
        }
    }
}
