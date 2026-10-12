//! Setting the profile up from the address a student pastes: check it is
//! Canvas, detect how it signs in, and save it.

use std::path::Path;

use super::{default_login_path, host_of, load, save, unimelb, Idp, Institution};

/// `canvas.foo.edu`, `https://foo.instructure.com/courses/12/` →
/// `https://host`.
pub fn normalise(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Enter your university's Canvas address.".to_string());
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let url =
        url::Url::parse(&with_scheme).map_err(|_| format!("“{trimmed}” is not a web address."))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!("“{trimmed}” is not a web address."));
    }
    let host = url
        .host_str()
        .filter(|h| h.contains('.'))
        .ok_or_else(|| format!("“{trimmed}” is not a web address."))?;
    Ok(format!("https://{}", host.to_ascii_lowercase()))
}

const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Canvas answers an anonymous `users/self` with 401 and
/// `{"status":"unauthenticated"}`; anything else is not Canvas.
fn check_is_canvas(base: &str) -> Result<(), String> {
    let url = format!("{base}/api/v1/users/self");
    let resp = ureq::get(&url)
        .timeout(PROBE_TIMEOUT)
        .set("Accept", "application/json")
        .call();
    let resp = match resp {
        Err(ureq::Error::Status(_, r)) => r,
        Ok(r) => r,
        Err(e) => return Err(format!("Could not reach {base}: {e}")),
    };
    let status = resp.status();
    let canvas_header = resp.header("x-canvas-meta").is_some();
    let body = resp.into_string().unwrap_or_default();
    if status == 401 && (canvas_header || body.contains("unauthenticated")) {
        Ok(())
    } else {
        Err(format!(
            "{base} does not look like Canvas. Use the address you open Canvas at, \
             such as canvas.youruni.edu or youruni.instructure.com."
        ))
    }
}

/// Where the sign-in flow starts and what it lands on, found by walking the
/// redirects anonymously. Some universities put a portal page on `/login`
/// (UniMelb), so `/login/saml` is tried when `/login` does not reach Okta.
fn detect_sign_in(base: &str) -> (String, Idp, Option<String>) {
    let canvas_host = host_of(base);
    let left_canvas = |landed: &url::Url| {
        landed
            .host_str()
            .filter(|h| Some(h.to_string()) != canvas_host)
            .map(str::to_string)
    };
    let mut first_away = None;
    for login_path in ["/login", "/login/saml"] {
        match keyd_core::okta::probe_sign_in_page(&format!("{base}{login_path}")) {
            Ok((landed, true)) => {
                return (login_path.to_string(), Idp::Okta, left_canvas(&landed));
            }
            Ok((landed, false)) => {
                if first_away.is_none() {
                    first_away = left_canvas(&landed);
                }
            }
            Err(e) => elog!("[oculus] sign-in detection: {login_path}: {e}"),
        }
    }
    (default_login_path(), Idp::Other, first_away)
}

/// Check the address is Canvas, detect how it signs in, and save the
/// profile. Callers refuse a new address while a library exists
/// (`library_exists`).
pub fn configure(data_dir: &Path, input: &str) -> Result<Institution, String> {
    let inst = probe(data_dir, input)?;
    apply(data_dir, inst)
}

/// The profile `input` would save, from the network alone: nothing is signed
/// out or written, so a mistyped address costs nothing. Re-saving the same
/// address keeps what launches have learned.
pub fn probe(data_dir: &Path, input: &str) -> Result<Institution, String> {
    let canvas = normalise(input)?;
    check_is_canvas(&canvas)?;
    let (login_path, idp, sso_host) = detect_sign_in(&canvas);

    let previous = load(data_dir).filter(|p| p.canvas == canvas);
    // A probe that misses Okta at a university already known to use it is
    // more likely a network blip or a changed widget than a new IdP, and
    // keyd treats a lost Okta origin as a move, dropping the saved setup key.
    if let Some(p) = previous
        .as_ref()
        .filter(|p| p.is_okta() && idp != Idp::Okta)
    {
        elog!("[oculus] sign-in detection missed Okta at {canvas}; keeping it");
        return Ok(p.clone());
    }
    let mut inst = match previous {
        Some(p) => p,
        // What is known about UniMelb (the lead-in, the Echo360 tool) holds
        // for a student who types its address, not only for an old install.
        None if canvas == unimelb().canvas => unimelb(),
        None => Institution {
            canvas: canvas.clone(),
            login_path: default_login_path(),
            idp: Idp::Other,
            sso_host: None,
            echo360_tool: None,
            echo360_lead_in_secs: 0,
            ed_api: None,
            ed_region: None,
            exclude_code_prefixes: Vec::new(),
        },
    };
    inst.login_path = login_path;
    inst.idp = idp;
    inst.sso_host = sso_host;
    Ok(inst)
}

/// Save `inst`, telling keyd first. A new address signs out and forgets the
/// old university's IdP credentials.
pub fn apply(data_dir: &Path, inst: Institution) -> Result<Institution, String> {
    if load(data_dir).is_none_or(|p| p.canvas != inst.canvas) {
        crate::auth::sign_out(data_dir)?;
        crate::auth::okta::clear_credentials().ok();
    }
    super::keyd::tell(data_dir, &inst)?;
    save(data_dir, &inst)?;
    Ok(inst)
}

/// Whether any subject has been synced. Subject rows are keyed by Canvas
/// course id, which another Canvas would reuse, so the address cannot change
/// under a library; Start over (`library::storage`) clears it first.
pub async fn library_exists() -> Result<bool, String> {
    if !crate::db::store::db_path().exists() {
        return Ok(false);
    }
    let pool = crate::db::store::open_pool().await?;
    let n: Result<i64, sqlx::Error> = sqlx::query_scalar("SELECT COUNT(*) FROM subjects")
        .fetch_one(&pool)
        .await;
    match n {
        Ok(n) => Ok(n > 0),
        // Before the first migration there is no table, and so no library.
        Err(e) if e.to_string().contains("no such table") => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}
