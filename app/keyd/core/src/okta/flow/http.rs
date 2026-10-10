//! The HTTP legs of the sign-in: a browser-shaped request, a redirect walk,
//! the state token in Okta's login page and the IDX calls.

use crate::okta::jar::Jar;
use crate::okta::{Env, LoginError};

pub(super) const IDX_MEDIA: &str = "application/ion+json; okta-version=1.0.0";
/// Hygiene, not a known requirement: every leg here impersonates a browser.
pub(super) const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
pub(super) const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);
/// Stops a policy we did not anticipate from looping.
pub(super) const MAX_STEPS: usize = 12;

/// An agent that lives for one request. The app enables ureq's `cookies`
/// feature for Echo360, and an agent with it jars `Set-Cookie` and adds its
/// own `Cookie` header beside the jar's; keyd has no such feature. A fresh
/// agent has nothing to replay, so both builds send the same request.
pub(super) fn agent() -> ureq::Agent {
    // Redirects are walked by hand so cookies can be filed per host.
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
}

/// `identity` is asked for by name: the `gzip` feature (also the app's alone)
/// would otherwise add `Accept-Encoding: gzip`.
pub(super) fn request(method: &str, url: &str) -> ureq::Request {
    agent()
        .request(method, url)
        .set("Accept-Encoding", "identity")
}

pub(super) fn host_of(u: &url::Url) -> String {
    u.host_str().unwrap_or_default().to_string()
}

/// Follow 3xx from `start`, filing cookies per host, until a non-redirect.
/// Returns where it landed and the body.
pub(super) fn walk(
    jar: &mut Jar,
    start: &str,
    max: usize,
) -> Result<(url::Url, String), LoginError> {
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
pub(super) fn bootstrap(jar: &mut Jar, env: &Env) -> Result<(String, String), LoginError> {
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

pub(super) fn extract_state_token(html: &str) -> Option<String> {
    state_token_candidates(html).into_iter().next()
}

/// Every `stateToken`-shaped value on the page. Inline script mentions the
/// name before the config assigns it, so each candidate must be a quoted
/// value of plausible shape, not just the first mention.
pub(super) fn state_token_candidates(html: &str) -> Vec<String> {
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
pub(super) fn unescape_js(s: &str) -> String {
    s.replace("\\x2D", "-")
        .replace("\\x2d", "-")
        .replace("\\u002D", "-")
        .replace("\\u002d", "-")
        .replace("\\x2F", "/")
        .replace("\\/", "/")
}

pub(super) fn looks_like_state_token(s: &str) -> bool {
    s.len() >= 20
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '~'))
}

/// One IDX call. A non-2xx body is parsed, not thrown: Okta explains its
/// 400/401s there.
pub(super) fn idx(
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
