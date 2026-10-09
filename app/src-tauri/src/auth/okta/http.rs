//! HTTP plumbing for the sign-in: a per-host cookie jar, redirect walking and
//! the IDX calls.

use std::collections::BTreeMap;

pub const SSO_HOST: &str = "sso.unimelb.edu.au";
const IDX_MEDIA: &str = "application/ion+json; okta-version=1.0.0";
/// Hygiene, not a known requirement: every leg here impersonates a browser.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);
/// Stops a policy we did not anticipate from looping.
pub(super) const MAX_STEPS: usize = 12;

/// Cookies kept per host: Okta's `sid` must never reach Canvas, nor Canvas's
/// session Okta.
#[derive(Default)]
pub(super) struct Jar(pub(super) BTreeMap<String, BTreeMap<String, String>>);

impl Jar {
    pub(super) fn absorb(&mut self, host: &str, resp: &ureq::Response) {
        let jar = self.0.entry(host.to_string()).or_default();
        for raw in resp.all("set-cookie") {
            let Some((k, v)) = raw.split(';').next().unwrap_or("").split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim().trim_matches('"'));
            if v.is_empty() {
                jar.remove(k);
            } else {
                jar.insert(k.to_string(), v.to_string());
            }
        }
    }

    pub(super) fn header(&self, host: &str) -> String {
        self.0
            .get(host)
            .map(|m| {
                m.iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default()
    }

    pub(super) fn has(&self, host: &str, name: &str) -> bool {
        self.0.get(host).is_some_and(|m| m.contains_key(name))
    }
}

/// Why an automated sign-in stopped. Callers act on the variant: a bad
/// password clears the stored one, a network failure keeps the session.
#[derive(Debug)]
pub enum LoginError {
    /// No credentials on file — automated sign-in was never set up.
    NotConfigured,
    /// The user signed out; only a sign-in they start lifts it.
    SignedOut,
    /// The keychain refused the read (a denied prompt, a sandboxed process);
    /// the credentials may well be on file. Carries the keychain's error.
    UnreadableCredentials(String),
    BadPassword(String),
    BadTotp(String),
    /// Okta offered only factors we cannot answer; carries their labels.
    UnsupportedFactor(Vec<String>),
    Locked(String),
    Network(String),
    /// The state machine went somewhere this code does not model; carries the
    /// remediation names.
    Unexpected(String),
    /// The attempt guard held an automatic sign-in back; carries seconds until
    /// the next one is allowed.
    Waiting(u64),
    /// Automatic sign-in stopped after a failure retrying cannot fix; carries
    /// that failure. A manual sign-in or newly saved credentials resume it.
    Paused(String),
}

impl std::fmt::Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginError::NotConfigured => write!(
                f,
                "Automated sign-in is not set up — save your username, password and \
                 authenticator setup key first."
            ),
            LoginError::SignedOut => write!(
                f,
                "Signed out, so automatic sign-in is off until you sign in from Settings → \
                 Canvas or run `oculus auth auto`."
            ),
            LoginError::UnreadableCredentials(m) => write!(
                f,
                "The keychain refused to give out the saved sign-in credentials ({m}). They \
                 are not missing — macOS denied this process access to them."
            ),
            LoginError::BadPassword(m) => write!(f, "Okta rejected the password: {m}"),
            LoginError::BadTotp(m) => write!(
                f,
                "Okta rejected the authenticator code: {m}. If this keeps happening the \
                 stored setup key is for a factor that has since been re-enrolled, or this \
                 Mac's clock has drifted."
            ),
            LoginError::UnsupportedFactor(opts) => write!(
                f,
                "Okta asked for a factor this app cannot answer. It offered: {}. \
                 Automated sign-in needs Google Authenticator (TOTP) enrolled.",
                if opts.is_empty() {
                    "nothing recognisable".to_string()
                } else {
                    opts.join(", ")
                }
            ),
            LoginError::Locked(m) => write!(f, "The account is locked or blocked: {m}"),
            LoginError::Network(m) => write!(f, "Could not reach the sign-in service: {m}"),
            LoginError::Unexpected(m) => write!(f, "Unexpected sign-in step: {m}"),
            LoginError::Waiting(secs) => write!(
                f,
                "Holding off automatic sign-in for {} more min after the last attempt.",
                secs.div_ceil(60)
            ),
            LoginError::Paused(m) => write!(
                f,
                "Automatic sign-in is paused after: {m}. Sign in from Settings → Canvas \
                 or run `oculus auth auto` to resume it."
            ),
        }
    }
}

pub(super) fn agent() -> ureq::Agent {
    // Redirects are walked by hand so cookies can be filed per host.
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
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
    let agent = agent();
    let mut url = url::Url::parse(start).map_err(|e| LoginError::Unexpected(e.to_string()))?;

    for _ in 0..max {
        let host = host_of(&url);
        let mut req = agent.get(url.as_str());
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
pub(super) fn bootstrap(jar: &mut Jar) -> Result<(String, String), LoginError> {
    let start = format!("{}/login/saml", crate::library::paths::CANVAS_BASE);
    let (landed, body) = walk(jar, &start, 10)?;

    if host_of(&landed) != SSO_HOST {
        return Err(LoginError::Unexpected(format!(
            "SAML start landed on {landed} instead of {SSO_HOST}"
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
pub(super) fn idx(
    jar: &mut Jar,
    url: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, LoginError> {
    let resp = agent()
        .post(url)
        .set("Accept", IDX_MEDIA)
        .set("Content-Type", IDX_MEDIA)
        .set("Cookie", &jar.header(SSO_HOST))
        .send_string(&body.to_string());

    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(e) => return Err(LoginError::Network(e.to_string())),
    };
    jar.absorb(SSO_HOST, &resp);
    let text = resp
        .into_string()
        .map_err(|e| LoginError::Network(e.to_string()))?;
    serde_json::from_str(&text)
        .map_err(|e| LoginError::Unexpected(format!("unreadable IDX response: {e}")))
}
