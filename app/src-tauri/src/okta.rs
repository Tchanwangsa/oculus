//! Headless University of Melbourne SSO sign-in.
//!
//! Canvas authenticates through Okta at `sso.unimelb.edu.au` (an Okta
//! **Identity Engine** org — the security-method chooser and the numbered push
//! challenge are OIE-only UI). OIE's sign-in widget is a thin client over a
//! JSON state machine at `/idp/idx/*`, so the whole flow can run in Rust with
//! no browser: introspect the login page's state token, answer each
//! *remediation* it offers, and finish by replaying the SAML app URL to get a
//! `SAMLResponse` we POST to Canvas.
//!
//! Why not drive the login WebView instead? Same reason the scraper lives in
//! Rust: an off-screen WKWebView gets suspended by macOS (see `canvas.rs`), and
//! a *visible* one defeats the point of automating the sign-in.
//!
//! ## What this can and cannot do
//!
//! It answers **password** and **TOTP** (Google Authenticator) factors. It
//! cannot answer Okta Verify push — that needs a human tapping a number on a
//! phone, which is the entire point of push. So the account must have a TOTP
//! factor enrolled and this module must hold that factor's shared secret.
//!
//! **A TOTP secret cannot be recovered by watching codes.** A code is
//! `HMAC-SHA1(secret, unix_time / 30)` truncated to 6 digits — a one-way
//! function of a 160-bit seed. Observing a million codes reveals nothing about
//! the next one. The seed is shown exactly once, at enrolment, as the QR code
//! and the "setup key" beside it. Getting it means *re-enrolling* the factor
//! and copying that key; there is no other path.
//!
//! ## Security shape
//!
//! Password and TOTP seed both live in the macOS keychain on one machine, so
//! for anything running as this user the second factor is no longer a second
//! factor. That is the deliberate trade: it is the same posture as a password
//! manager that stores TOTP next to the password. It does not weaken the
//! account against anyone who is not already on this Mac as this user.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ── SHA-1 / HMAC / TOTP ──────────────────────────────────────────────────────
//
// Hand-rolled because the sandbox this was written in could not fetch crates,
// and because RFC 6238 is 40 lines over a primitive that has not changed since
// 1995. Correctness is pinned by the RFC's own test vectors in `mod tests` —
// if those pass, this is right. SHA-1 is broken for *collisions*; HMAC-SHA1 is
// not, and is what every authenticator app implements.

fn sha1(msg: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    let bit_len = (msg.len() as u64).wrapping_mul(8);

    let mut data = msg.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in data.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn hmac_sha1(key: &[u8], msg: &[u8]) -> [u8; 20] {
    const BLOCK: usize = 64;
    let mut k = if key.len() > BLOCK { sha1(key).to_vec() } else { key.to_vec() };
    k.resize(BLOCK, 0);

    let mut inner = Vec::with_capacity(BLOCK + msg.len());
    inner.extend(k.iter().map(|b| b ^ 0x36));
    inner.extend_from_slice(msg);
    let inner = sha1(&inner);

    let mut outer = Vec::with_capacity(BLOCK + 20);
    outer.extend(k.iter().map(|b| b ^ 0x5c));
    outer.extend_from_slice(&inner);
    sha1(&outer)
}

/// RFC 4648 base32, which is how every authenticator app writes a seed.
/// Tolerates the spaces and lowercase Okta's setup key is shown with.
pub fn base32_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut bits: u32 = 0;
    let mut nbits: u32 = 0;
    let mut out = Vec::new();
    for ch in s.chars() {
        if ch == '=' || ch.is_whitespace() || ch == '-' {
            continue;
        }
        let v = match ch.to_ascii_uppercase() {
            c @ 'A'..='Z' => c as u32 - 'A' as u32,
            c @ '2'..='7' => c as u32 - '2' as u32 + 26,
            other => return Err(format!("'{other}' is not a base32 character")),
        };
        bits = (bits << 5) | v;
        nbits += 5;
        if nbits >= 8 {
            nbits -= 8;
            out.push((bits >> nbits) as u8);
        }
    }
    if out.is_empty() {
        return Err("secret is empty".to_string());
    }
    Ok(out)
}

/// RFC 6238 TOTP: 6 digits, 30-second step, SHA-1.
pub fn totp_at(secret: &[u8], unix_seconds: u64, step: u64, digits: u32) -> String {
    let counter = unix_seconds / step;
    let mac = hmac_sha1(secret, &counter.to_be_bytes());
    // Dynamic truncation: the low nibble of the last byte picks the offset.
    let off = (mac[19] & 0x0f) as usize;
    let bin = ((mac[off] as u32 & 0x7f) << 24)
        | ((mac[off + 1] as u32) << 16)
        | ((mac[off + 2] as u32) << 8)
        | (mac[off + 3] as u32);
    let code = bin % 10u32.pow(digits);
    format!("{code:0width$}", width = digits as usize)
}

/// The code an authenticator app would be showing right now.
pub fn totp_now(secret_b32: &str) -> Result<String, String> {
    let secret = base32_decode(secret_b32)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    Ok(totp_at(&secret, now, 30, 6))
}

/// Seconds until the current code rolls over. Sign-in waits for a fresh code
/// when this is small: Okta rejects a reused code, and burning our one attempt
/// on a code with 2 seconds left is how an automated retry loop locks an
/// account out.
pub fn totp_seconds_remaining() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    30 - (now % 30)
}

// ── Stored credentials ───────────────────────────────────────────────────────

const KEYCHAIN_SERVICE: &str = "com.oculus.unimelb-sso";

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, account).map_err(|e| e.to_string())
}

fn read(account: &str) -> Option<String> {
    entry(account).ok()?.get_password().ok().filter(|s| !s.is_empty())
}

fn write(account: &str, value: &str) -> Result<(), String> {
    entry(account)?.set_password(value).map_err(|e| e.to_string())
}

fn erase(account: &str) -> Result<(), String> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// What a headless sign-in needs. Never logged, never written outside the
/// keychain, never sent anywhere but `sso.unimelb.edu.au`.
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub totp_secret: String,
}

impl Credentials {
    pub fn load() -> Option<Credentials> {
        Some(Credentials {
            username: read("username")?,
            password: read("password")?,
            totp_secret: read("totp_secret")?,
        })
    }
}

/// Whether each piece is on file, for the settings UI. Values never leave the
/// keychain — the UI only needs to know what is missing.
#[derive(serde::Serialize)]
pub struct CredentialStatus {
    pub username: Option<String>,
    pub has_password: bool,
    pub has_totp: bool,
}

pub fn credential_status() -> CredentialStatus {
    CredentialStatus {
        username: read("username"),
        has_password: read("password").is_some(),
        has_totp: read("totp_secret").is_some(),
    }
}

/// Save credentials, validating the TOTP seed before it is stored — a seed
/// that cannot be decoded would otherwise fail much later, mid sign-in, as an
/// indistinguishable "wrong code".
pub fn store_credentials(username: &str, password: &str, totp_secret: &str) -> Result<(), String> {
    let username = username.trim();
    let secret = totp_secret.trim().replace(' ', "");
    if username.is_empty() {
        return Err("Username is required.".to_string());
    }
    if password.is_empty() {
        return Err("Password is required.".to_string());
    }
    base32_decode(&secret).map_err(|e| format!("That does not look like a TOTP setup key: {e}"))?;

    write("username", username)?;
    write("password", password)?;
    write("totp_secret", &secret)?;
    Ok(())
}

/// Forget everything. Called on explicit disconnect, and on a rejected
/// password so a stale secret is not replayed until Okta locks the account.
pub fn clear_credentials() -> Result<(), String> {
    erase("username")?;
    erase("password")?;
    erase("totp_secret")?;
    Ok(())
}

/// Drop only the password, keeping username and seed. This is the response to
/// `LoginError::BadPassword`: the user changed their UniMelb password, so the
/// stored one is worthless, but re-scanning the authenticator would be a
/// pointless extra chore.
pub fn clear_password() -> Result<(), String> {
    erase("password")
}

// ── HTTP plumbing ────────────────────────────────────────────────────────────

const SSO_HOST: &str = "sso.unimelb.edu.au";
const IDX_MEDIA: &str = "application/ion+json; okta-version=1.0.0";
/// Hygiene, not a known requirement: the flow was made to work without this,
/// but a default `ureq/2.x` is exactly the sort of thing an IdP starts
/// treating differently, and every leg here is impersonating a browser anyway.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);
/// Every remediation step is one round trip; a real sign-in takes four or
/// five. This only has to stop a policy we did not anticipate from looping.
const MAX_STEPS: usize = 12;

/// Cookies kept per host. Okta's `sid` must never be sent to Canvas and the
/// Canvas session must never be sent to Okta — the same per-host discipline
/// `ed.rs` needs for edstem.
#[derive(Default)]
struct Jar(BTreeMap<String, BTreeMap<String, String>>);

impl Jar {
    fn absorb(&mut self, host: &str, resp: &ureq::Response) {
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

    fn header(&self, host: &str) -> String {
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

    fn has(&self, host: &str, name: &str) -> bool {
        self.0.get(host).is_some_and(|m| m.contains_key(name))
    }
}

/// Why an automated sign-in stopped. The variants exist so callers can do the
/// right thing rather than show one opaque failure: a bad password must clear
/// the stored one, a missing factor must tell the user what to enrol, and a
/// flat network must not throw away a session that is probably still fine.
#[derive(Debug)]
pub enum LoginError {
    /// No credentials on file — automated sign-in was never set up.
    NotConfigured,
    BadPassword(String),
    BadTotp(String),
    /// Okta offered only factors we cannot answer. Carries the labels it did
    /// offer, which is exactly what the user needs to see.
    UnsupportedFactor(Vec<String>),
    Locked(String),
    Network(String),
    /// The state machine went somewhere this code does not model. Carries the
    /// remediation names so a first run diagnoses itself instead of needing a
    /// packet capture.
    Unexpected(String),
}

impl std::fmt::Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginError::NotConfigured => write!(
                f,
                "Automated sign-in is not set up — save your username, password and \
                 authenticator setup key first."
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
                if opts.is_empty() { "nothing recognisable".to_string() } else { opts.join(", ") }
            ),
            LoginError::Locked(m) => write!(f, "The account is locked or blocked: {m}"),
            LoginError::Network(m) => write!(f, "Could not reach the sign-in service: {m}"),
            LoginError::Unexpected(m) => write!(f, "Unexpected sign-in step: {m}"),
        }
    }
}

fn agent() -> ureq::Agent {
    // Redirects are walked by hand so cookies can be filed per host.
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
}

fn host_of(u: &url::Url) -> String {
    u.host_str().unwrap_or_default().to_string()
}

/// Follow 3xx from `start`, filing cookies per host, until a non-redirect.
/// Returns where it landed and the body.
fn walk(jar: &mut Jar, start: &str, max: usize) -> Result<(url::Url, String), LoginError> {
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
    Err(LoginError::Unexpected("redirect loop during sign-in".into()))
}

/// Start the SAML flow and pull the Okta widget's state token out of the
/// login page. The token is the handle to the IDX state machine; everything
/// after this is JSON.
fn bootstrap(jar: &mut Jar) -> Result<(String, String), LoginError> {
    let start = format!("{}/login/saml", crate::paths::CANVAS_BASE);
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

fn extract_state_token(html: &str) -> Option<String> {
    state_token_candidates(html).into_iter().next()
}

/// Every `stateToken`-shaped value on the page.
///
/// The hosted page mentions `stateToken` several times — in inline script
/// logic as well as in the widget config — so taking the *first* mention
/// yields a fragment Okta rejects as an expired session. Each candidate is
/// therefore required to be a quoted value of plausible shape.
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
        let Some(end) = rest.find(quote) else { continue };

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

/// One IDX call. Okta answers 400/401 with a body that explains itself, so a
/// non-2xx status is parsed rather than thrown — the message is the whole
/// point.
fn idx(jar: &mut Jar, url: &str, body: serde_json::Value) -> Result<serde_json::Value, LoginError> {
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
    let text = resp.into_string().map_err(|e| LoginError::Network(e.to_string()))?;
    serde_json::from_str(&text)
        .map_err(|e| LoginError::Unexpected(format!("unreadable IDX response: {e}")))
}

// ── Remediation helpers ──────────────────────────────────────────────────────

fn remediations(state: &serde_json::Value) -> Vec<&serde_json::Value> {
    state["remediation"]["value"].as_array().map(|a| a.iter().collect()).unwrap_or_default()
}

fn remediation<'a>(state: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    remediations(state).into_iter().find(|r| r["name"].as_str() == Some(name))
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
        .and_then(|fields| fields.iter().find(|f| f["name"].as_str() == Some("authenticator")))
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

/// Build the `authenticator` payload that selects `want`.
///
/// TOTP is matched by label first: Okta Verify also advertises `methodType:
/// otp`, and answering *it* with a Google Authenticator seed produces a
/// baffling "invalid code" rather than a useful error.
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
        let joined: Vec<&str> = messages.iter().filter_map(|m| m["message"].as_str()).collect();
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
/// cannot answer and the caller should switch via the chooser.
///
/// Okta names the authenticator under `currentAuthenticator`; when it says
/// nothing we fall back on flow position, which is what this used to do
/// unconditionally.
fn challenged_factor(state: &serde_json::Value, password_done: bool) -> Option<Factor> {
    let current = ["currentAuthenticator", "currentAuthenticatorEnrollment"]
        .iter()
        .map(|k| &state[*k])
        .find(|v| !v.is_null())
        .map(|v| v.get("value").unwrap_or(v));

    let Some(cur) = current else {
        return Some(if password_done { Factor::Totp } else { Factor::Password });
    };

    let key = cur["key"].as_str().unwrap_or("");
    let methods: Vec<&str> = cur["methods"]
        .as_array()
        .map(|a| a.iter().filter_map(|m| m["type"].as_str()).collect())
        .unwrap_or_default();

    if key == "okta_password" || methods.contains(&"password") {
        // Being asked for the password again after it was accepted is Okta
        // moving on, not an instruction to resend it.
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

/// Sign in headlessly and persist the resulting Canvas session cookie.
/// Returns the cookie header on success.
///
/// The loop is written against IDX *remediations* rather than a fixed script,
/// because the order Okta asks for factors in is a policy setting that can
/// change without notice. Anything it offers that we cannot answer comes back
/// as [`LoginError::UnsupportedFactor`] naming the options, so a failure is
/// self-diagnosing.
pub fn sign_in(data_dir: &std::path::Path) -> Result<String, LoginError> {
    let creds = Credentials::load().ok_or(LoginError::NotConfigured)?;
    let mut jar = Jar::default();

    let (state_token, saml_url) = bootstrap(&mut jar)?;
    let idx_base = format!("https://{SSO_HOST}/idp/idx");

    // Introspect is the one call that names the token `stateToken`; every
    // later call echoes back the `stateHandle` the response carries. Some
    // configurations accept only the latter here, so a field-name mismatch
    // falls back instead of failing the whole sign-in.
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
                let href = rem["href"].as_str().unwrap_or(&format!("{idx_base}/identify")).to_string();
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

        // Answer the factor Okta is actually challenging, BEFORE considering
        // the chooser. OIE offers `select-authenticator-authenticate`
        // alongside every challenge as the "verify with something else"
        // escape hatch, so treating that as the next step re-picks the same
        // authenticator forever without ever answering it.
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
            let want = if password_done { Factor::Totp } else { Factor::Password };
            // Selecting the same factor twice means the answer never landed;
            // stop rather than spin out the step budget.
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

    // Prove the cookie authenticates before persisting it. Writing first and
    // discovering the 401 later overwrites a session that may still have been
    // good, and reports a success that is not one.
    let name = verify(&cookie)?;

    let path = crate::paths::cookie_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&path, &cookie)
        .map_err(|e| LoginError::Unexpected(format!("could not save the session cookie: {e}")))?;
    eprintln!("[oculus] automated sign-in succeeded — Canvas accepted the session as {name}");
    Ok(cookie)
}

/// Confirm Canvas accepts the freshly minted cookie, returning the account
/// name it reports.
fn verify(cookie: &str) -> Result<String, LoginError> {
    let url = format!("{}/api/v1/users/self", crate::paths::CANVAS_BASE);
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
            "the SAML round trip finished but Canvas rejected the session (HTTP {}) —              the assertion was not accepted",
            r.status()
        ))),
        Err(e) => Err(LoginError::Network(e.to_string())),
    }
}

/// A TOTP code is single-use and Okta rejects a replay, so spending one that
/// is about to expire wastes an attempt — and repeated attempts are what trip
/// the lockout policy. Wait out the tail of the window instead.
fn wait_for_fresh_code() {
    let left = totp_seconds_remaining();
    if left < 3 {
        std::thread::sleep(std::time::Duration::from_secs(left + 1));
    }
}

/// With an Okta session in hand, replay the SAML app URL: Okta now answers
/// with the auto-submit assertion form, which we POST to Canvas to trade for
/// a `canvas_session` cookie.
fn complete_saml(jar: &mut Jar, saml_url: &str) -> Result<String, LoginError> {
    let canvas_host = url::Url::parse(crate::paths::CANVAS_BASE)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();

    let agent = agent();
    let mut url =
        url::Url::parse(saml_url).map_err(|e| LoginError::Unexpected(e.to_string()))?;
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

        // Only once the assertion has actually been posted. Canvas hands out
        // an anonymous `canvas_session` to the very first visitor — which the
        // SAML start collected before any of this — so checking for the
        // cookie alone reports success while authenticating nothing.
        if posted_assertion && jar.has(&canvas_host, "canvas_session") {
            return Ok(jar.header(&canvas_host));
        }

        let body = resp.into_string().unwrap_or_default();
        let (action, fields) = parse_saml_form(&body).ok_or_else(|| {
            LoginError::Unexpected(format!("no SAML assertion form at {url}"))
        })?;
        url = url
            .join(&action)
            .map_err(|e| LoginError::Unexpected(format!("bad form action: {e}")))?;
        form = Some(
            fields
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}={}",
                        url::form_urlencoded::byte_serialize(k.as_bytes()).collect::<String>(),
                        url::form_urlencoded::byte_serialize(v.as_bytes()).collect::<String>()
                    )
                })
                .collect::<Vec<_>>()
                .join("&"),
        );
        posted_assertion = true;
    }
    Err(LoginError::Unexpected(
        "the SAML assertion never reached Canvas".into(),
    ))
}

/// The auto-submit form carrying the assertion. Matched by the presence of a
/// `SAMLResponse` field, since Okta's pages carry unrelated forms too.
fn parse_saml_form(html: &str) -> Option<(String, Vec<(String, String)>)> {
    let doc = scraper::Html::parse_document(html);
    let form_sel = scraper::Selector::parse("form").ok()?;
    let input_sel = scraper::Selector::parse("input").ok()?;

    for form in doc.select(&form_sel) {
        let fields: Vec<(String, String)> = form
            .select(&input_sel)
            .filter_map(|i| {
                Some((i.value().attr("name")?.to_string(), i.value().attr("value").unwrap_or("").to_string()))
            })
            .collect();
        if fields.iter().any(|(k, _)| k == "SAMLResponse") {
            return Some((form.value().attr("action")?.to_string(), fields));
        }
    }
    None
}

/// Keep-alive's entry point: confirm the saved session, and quietly rebuild it
/// if it has lapsed. `Ok(false)` means the existing session was already fine.
///
/// A network failure is deliberately *not* an attempt to sign in again — an
/// offline laptop is not an expired session, the same distinction
/// `AuthProbe::Unreachable` exists to draw.
pub fn ensure_session(data_dir: &std::path::Path) -> Result<bool, LoginError> {
    let canvas = crate::canvas::Canvas::open(data_dir);
    if canvas.has_session() {
        match canvas.get("/api/v1/users/self") {
            Ok(r) if r.ok() => return Ok(false),
            Ok(r) if r.status == 401 => {}
            Ok(r) => {
                return Err(LoginError::Network(format!("Canvas returned HTTP {}", r.status)))
            }
            Err(e) => return Err(LoginError::Network(e)),
        }
    }
    sign_in(data_dir).map(|_| true)
}

// ── Diagnosis ────────────────────────────────────────────────────────────────

/// What the sign-in page actually looks like from here, for when the flow
/// fails against the live Okta policy. Reports shapes and lengths, never
/// values: a state token is a live credential for the duration of a login.
pub fn diagnose() -> String {
    let mut out = String::new();
    let mut jar = Jar::default();

    let start = format!("{}/login/saml", crate::paths::CANVAS_BASE);
    let (landed, body) = match walk(&mut jar, &start, 10) {
        Ok(v) => v,
        Err(e) => return format!("could not reach the sign-in page: {e}\n"),
    };

    out.push_str(&format!("landed on   {landed}\n"));
    out.push_str(&format!("page size   {} bytes\n", body.len()));
    out.push_str(&format!(
        "okta cookies {}\n",
        jar.0.get(SSO_HOST).map(|m| m.len()).unwrap_or(0)
    ));

    let markers = [
        "stateToken",
        "interactionHandle",
        "interaction_code",
        "okta-signin-widget",
        "signin-container",
        "OktaUtil",
    ];
    let seen: Vec<&str> = markers.iter().copied().filter(|m| body.contains(m)).collect();
    out.push_str(&format!(
        "markers     {}\n",
        if seen.is_empty() { "none".to_string() } else { seen.join(", ") }
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
    let introspect = format!("https://{SSO_HOST}/idp/idx/introspect");
    for field in ["stateToken", "stateHandle"] {
        match idx(&mut jar, &introspect, serde_json::json!({ field: token })) {
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

// ── Tauri commands ───────────────────────────────────────────────────────────

fn data_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    app.path().app_data_dir().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn okta_credential_status() -> CredentialStatus {
    credential_status()
}

#[tauri::command]
pub fn okta_save_credentials(
    username: String,
    password: String,
    totp_secret: String,
) -> Result<(), String> {
    store_credentials(&username, &password, &totp_secret)
}

#[tauri::command]
pub fn okta_clear_credentials() -> Result<(), String> {
    clear_credentials()
}

#[tauri::command]
pub async fn okta_sign_in(app: tauri::AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = data_dir(&app)?;
        run_sign_in(&app, &dir)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Sign in, then bring the rest of the app's auth state with it: the flag file
/// `get_auth_status` reads and the event the UI listens on, so an automated
/// sign-in is indistinguishable from the interactive one downstream.
fn run_sign_in(app: &tauri::AppHandle, dir: &std::path::Path) -> Result<String, String> {
    use tauri::{Emitter, Manager};

    match sign_in(dir) {
        Ok(_) => {
            let flag = crate::auth::auth_flag_path(app);
            if let Some(parent) = flag.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(&flag, b"1").ok();
            if let Some(state) = app.try_state::<crate::auth::AuthState>() {
                *state.0.lock().unwrap() = true;
            }
            // Proof the headless path works on this account — the only
            // condition under which a LaunchAgent that re-authenticates is
            // worth installing. Push/WebAuthn orgs never reach here.
            crate::keepalive::ensure_installed(app);
            app.emit("canvas-auth-success", "ok").ok();
            crate::canvas::Canvas::open(dir).whoami()
        }
        Err(e @ LoginError::BadPassword(_)) => {
            // Drop the stored password rather than let the keep-alive replay a
            // wrong one every six hours until Okta locks the account.
            clear_password().ok();
            Err(e.to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Called when a probe finds the session dead: rebuild it silently if
/// automated sign-in is set up. `false` means the caller should fall back to
/// asking the user, exactly as before.
pub fn try_auto_recover(app: &tauri::AppHandle) -> bool {
    if Credentials::load().is_none() {
        return false;
    }
    let Ok(dir) = data_dir(app) else {
        return false;
    };
    match run_sign_in(app, &dir) {
        Ok(name) => {
            eprintln!("[oculus] session rebuilt without a browser ({name})");
            true
        }
        Err(e) => {
            eprintln!("[oculus] automated re-sign-in failed: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4226 appendix D, the canonical HOTP vectors.
    #[test]
    fn matches_the_rfc_4226_hotp_vectors() {
        let secret = b"12345678901234567890";
        let expected = [
            "755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583",
            "399871", "520489",
        ];
        for (counter, want) in expected.iter().enumerate() {
            // TOTP with step 1 at time == counter is exactly HOTP(counter).
            assert_eq!(&totp_at(secret, counter as u64, 1, 6), want, "counter {counter}");
        }
    }

    /// RFC 6238 appendix B, the SHA-1 rows.
    #[test]
    fn matches_the_rfc_6238_totp_vectors() {
        let secret = b"12345678901234567890";
        for (time, want) in [
            (59u64, "94287082"),
            (1_111_111_109, "07081804"),
            (1_111_111_111, "14050471"),
            (1_234_567_890, "89005924"),
            (2_000_000_000, "69279037"),
        ] {
            assert_eq!(totp_at(secret, time, 30, 8), want, "t={time}");
        }
    }

    #[test]
    fn decodes_base32_the_way_authenticator_apps_write_it() {
        assert_eq!(base32_decode("GEZDGNBVGY3TQOJQ").unwrap(), b"12345678901234567890"[..10].to_vec());
        // Okta shows the setup key in spaced, lowercase groups.
        assert_eq!(
            base32_decode("gezd gnbv gy3t qojq").unwrap(),
            base32_decode("GEZDGNBVGY3TQOJQ").unwrap()
        );
        assert!(base32_decode("not-valid-1890").is_err());
        assert!(base32_decode("").is_err());
    }

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

    /// The bug this replaced: Okta's page names `stateToken` in inline script
    /// logic before the config assigns it, so taking the first mention got a
    /// fragment and introspect answered "The session has expired".
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

        assert_eq!(select_payload(&rem, Factor::Password).unwrap()["id"], "aut_pw");

        // Google Authenticator must win over Okta Verify's TOTP, which
        // advertises the same methodType but has a different seed.
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

    /// The loop bug this guards: OIE offers the factor chooser alongside every
    /// challenge, so the challenge must be answered on what Okta says it is
    /// challenging, not on where we think we are in the flow.
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
        assert!(matches!(check_messages(&state, Some(Factor::Totp)), Err(LoginError::Locked(_))));
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

    #[test]
    fn cookies_are_filed_per_host() {
        let mut jar = Jar::default();
        jar.0.entry("a.example".into()).or_default().insert("sid".into(), "1".into());
        jar.0.entry("b.example".into()).or_default().insert("other".into(), "2".into());
        assert_eq!(jar.header("a.example"), "sid=1");
        assert!(!jar.has("b.example", "sid"));
        assert_eq!(jar.header("nowhere.example"), "");
    }
}
