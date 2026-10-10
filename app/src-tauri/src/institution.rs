//! Which university this install belongs to: its Canvas address, how its
//! sign-in works, and the per-university details the sync needs.
//!
//! One file, `institution.json` in the data dir, read alike by the app and the
//! CLI. It is not a `settings` row because synchronous code and the launchd
//! `oculus auth tick` need the Canvas host without a database pool. The student
//! pastes their Canvas address; everything else is detected or learned. See
//! `docs/auth.md`.

use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Institution {
    /// `https://host`, no path or trailing slash.
    pub canvas: String,
    /// Where the sign-in window starts. Canvas sends `/login` to the account's
    /// default provider, unless the university puts a portal page there.
    #[serde(default = "default_login_path")]
    pub login_path: String,
    #[serde(default)]
    pub idp: Idp,
    /// The host the sign-in redirect lands on, when it leaves Canvas.
    #[serde(default)]
    pub sso_host: Option<String>,
    /// A Canvas tool path tried after the course's own tabs (`echo360.rs`).
    #[serde(default)]
    pub echo360_tool: Option<String>,
    /// Seconds of intro the university puts on every recording, trimmed off.
    #[serde(default)]
    pub echo360_lead_in_secs: u32,
    /// Ed's API base, learned from the first LTI launch (`ed.rs`).
    #[serde(default)]
    pub ed_api: Option<String>,
    /// Course codes that are never subjects (training shells and the like).
    #[serde(default)]
    pub exclude_code_prefixes: Vec<String>,
}

/// Okta Identity Engine is the one IdP Oculus can answer headlessly
/// (`okta.rs`); every other sign-in happens in the visible window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Idp {
    Okta,
    #[default]
    Other,
}

fn default_login_path() -> String {
    "/login".to_string()
}

/// The profile an install that predates this file was built against.
pub fn unimelb() -> Institution {
    Institution {
        canvas: "https://canvas.lms.unimelb.edu.au".to_string(),
        login_path: "/login/saml".to_string(),
        idp: Idp::Okta,
        sso_host: Some("sso.unimelb.edu.au".to_string()),
        echo360_tool: Some("/external_tools/701".to_string()),
        echo360_lead_in_secs: 14,
        ed_api: Some("https://edstem.org/api".to_string()),
        exclude_code_prefixes: vec!["MPMP".to_string()],
    }
}

pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("institution.json")
}

// ── Loading ──────────────────────────────────────────────────────────────────

/// Read the profile. A missing file beside sign-in or sync state means an
/// install from before the profile existed, which was UniMelb's, so that
/// profile is written and returned. A fresh install has neither and gets
/// `None` until the student enters their Canvas address.
pub fn load(data_dir: &Path) -> Option<Institution> {
    match std::fs::read_to_string(path(data_dir)) {
        Ok(raw) => match serde_json::from_str(&raw) {
            Ok(inst) => Some(inst),
            Err(e) => {
                eprintln!("[oculus] institution.json is unreadable: {e}");
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && predates_profile(data_dir) => {
            let inst = unimelb();
            if let Err(e) = save(data_dir, &inst) {
                eprintln!("[oculus] could not write institution.json: {e}");
            }
            Some(inst)
        }
        Err(_) => None,
    }
}

/// Only a sign-in or a sync writes these, and neither runs without a profile,
/// so any of them means the data dir is older than the profile.
fn predates_profile(data_dir: &Path) -> bool {
    let courses = data_dir.join("courses");
    crate::paths::cookie_path(data_dir).exists()
        || crate::paths::auth_flag_path(data_dir).exists()
        || crate::paths::sign_in_record_path(data_dir).exists()
        || std::fs::read_dir(&courses).is_ok_and(|mut d| d.next().is_some())
}

pub fn save(data_dir: &Path, inst: &Institution) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let body = serde_json::to_vec_pretty(inst).map_err(|e| e.to_string())?;
    let target = path(data_dir);
    crate::atomic_write::write(&target, &target.with_extension("json.tmp"), &body)
}

/// Keyed on the file's mtime, so a profile the CLI writes reaches a running
/// app (and the reverse) on the next read.
static CACHE: RwLock<Option<(SystemTime, Option<Institution>)>> = RwLock::new(None);

/// The profile for the app's data dir.
pub fn current() -> Option<Institution> {
    let dir = crate::paths::data_dir();
    let mtime = || {
        std::fs::metadata(path(&dir))
            .and_then(|m| m.modified())
            .ok()
    };
    if let Some(m) = mtime() {
        let cache = CACHE.read().unwrap_or_else(|e| e.into_inner());
        if let Some((_, inst)) = cache.as_ref().filter(|(cached, _)| *cached == m) {
            return inst.clone();
        }
    }
    let inst = load(&dir);
    if let Some(m) = mtime() {
        *CACHE.write().unwrap_or_else(|e| e.into_inner()) = Some((m, inst.clone()));
    }
    inst
}

pub const NOT_CONFIGURED: &str =
    "No Canvas address set — add your university's Canvas address in Settings → Canvas.";

/// `https://host` of the university's Canvas.
pub fn canvas_base() -> Result<String, String> {
    current()
        .map(|i| i.canvas)
        .ok_or_else(|| NOT_CONFIGURED.to_string())
}

pub fn canvas_host() -> Option<String> {
    current().and_then(|i| host_of(&i.canvas))
}

/// The SSO host, when sign-in leaves Canvas for one.
pub fn sso_host() -> Option<String> {
    current().and_then(|i| i.sso_host)
}

pub fn is_okta() -> bool {
    current().is_some_and(|i| i.idp == Idp::Okta && i.sso_host.is_some())
}

/// Country codes south of the equator, for reading season-named terms.
const SOUTHERN_TLDS: &[&str] = &["au", "nz", "za", "ar", "br", "cl", "uy"];

/// Whether the university is in the southern hemisphere, judged by its Canvas
/// or SSO host's country code. Only term names lean on it (`terms.rs`).
pub fn southern_hemisphere() -> bool {
    current().is_some_and(|i| is_southern(&i.canvas, i.sso_host.as_deref()))
}

fn is_southern(canvas: &str, sso_host: Option<&str>) -> bool {
    host_of(canvas)
        .into_iter()
        .chain(sso_host.map(str::to_string))
        .any(|h| {
            h.rsplit('.')
                .next()
                .is_some_and(|tld| SOUTHERN_TLDS.contains(&tld))
        })
}

/// Seconds trimmed off the start of every recording.
pub fn echo360_lead_in() -> u32 {
    current().map(|i| i.echo360_lead_in_secs).unwrap_or(0)
}

fn host_of(base: &str) -> Option<String> {
    url::Url::parse(base)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
}

/// Record a fact a launch discovered. Silent: losing it only costs the next
/// launch rediscovering it.
pub fn learn(update: impl FnOnce(&mut Institution)) {
    let dir = crate::paths::data_dir();
    let Some(mut inst) = load(&dir) else {
        return;
    };
    let before = inst.clone();
    update(&mut inst);
    if inst != before {
        if let Err(e) = save(&dir, &inst) {
            eprintln!("[oculus] could not update institution.json: {e}");
        }
    }
}

// ── The institution domain ───────────────────────────────────────────────────

/// Second-level labels under a country code that are registries, not
/// institutions: `edu.au`, `ac.uk`, `co.nz`.
const REGISTRY_LABELS: &[&str] = &["ac", "co", "com", "edu", "gov", "net", "org", "sch"];

/// Hosting providers whose subdomains belong to different customers.
const SHARED_HOSTS: &[&str] = &["instructure.com", "okta.com", "oktapreview.com"];

/// The domain the Canvas and SSO hosts share, when it is the university's own
/// (`unimelb.edu.au`), not a public suffix (`edu.au`) or a shared host
/// (`instructure.com`). The in-app browser seeds sessions for pages inside it.
pub fn institution_domain(canvas_host: &str, sso_host: Option<&str>) -> Option<String> {
    let sso = sso_host?;
    let a: Vec<&str> = canvas_host.rsplit('.').collect();
    let b: Vec<&str> = sso.rsplit('.').collect();
    let shared: Vec<&str> = a
        .iter()
        .zip(b.iter())
        .take_while(|(x, y)| x.eq_ignore_ascii_case(y))
        .map(|(x, _)| *x)
        .collect();
    let minimum = match shared.as_slice() {
        [tld, second, ..] if tld.len() == 2 && REGISTRY_LABELS.contains(second) => 3,
        _ => 2,
    };
    if shared.len() < minimum {
        return None;
    }
    let domain = shared.iter().rev().copied().collect::<Vec<_>>().join(".");
    (!SHARED_HOSTS.contains(&domain.as_str())).then_some(domain)
}

// ── Setting it up ────────────────────────────────────────────────────────────

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
        match crate::okta::probe_sign_in_page(&format!("{base}{login_path}")) {
            Ok((landed, true)) => {
                return (login_path.to_string(), Idp::Okta, left_canvas(&landed));
            }
            Ok((landed, false)) => {
                if first_away.is_none() {
                    first_away = left_canvas(&landed);
                }
            }
            Err(e) => eprintln!("[oculus] sign-in detection: {login_path}: {e}"),
        }
    }
    (default_login_path(), Idp::Other, first_away)
}

/// Check the address is Canvas, detect how it signs in, and save the
/// profile. Re-saving the same address keeps what launches have learned.
/// Callers refuse a new address while a library exists (`library_exists`).
pub fn configure(data_dir: &Path, input: &str) -> Result<Institution, String> {
    let canvas = normalise(input)?;
    check_is_canvas(&canvas)?;
    let (login_path, idp, sso_host) = detect_sign_in(&canvas);

    let previous = load(data_dir);
    let same = previous.as_ref().is_some_and(|p| p.canvas == canvas);
    let mut inst = match previous.filter(|_| same) {
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
            exclude_code_prefixes: Vec::new(),
        },
    };
    inst.login_path = login_path;
    inst.idp = idp;
    inst.sso_host = sso_host;

    if !same {
        // The old university's session and IdP credentials mean nothing here.
        crate::paths::sign_out(data_dir).map_err(|e| e.to_string())?;
        crate::okta::clear_credentials().ok();
    }
    save(data_dir, &inst)?;
    Ok(inst)
}

/// Whether any subject has been synced. Subject rows are keyed by Canvas
/// course id, which another Canvas would reuse, so the address cannot change
/// under a library; Start over (`storage.rs`) clears it first.
pub async fn library_exists() -> Result<bool, String> {
    if !crate::store::db_path().exists() {
        return Ok(false);
    }
    let pool = crate::store::open_pool().await?;
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

// ── Tauri commands ───────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct InstitutionView {
    pub canvas: Option<String>,
    pub host: Option<String>,
    pub okta: bool,
    pub sso_host: Option<String>,
    /// The address cannot change while a library exists.
    pub locked: bool,
}

async fn view() -> Result<InstitutionView, String> {
    let inst = current();
    Ok(InstitutionView {
        host: inst.as_ref().and_then(|i| host_of(&i.canvas)),
        okta: is_okta(),
        sso_host: inst.as_ref().and_then(|i| i.sso_host.clone()),
        canvas: inst.map(|i| i.canvas),
        locked: library_exists().await?,
    })
}

#[tauri::command]
pub async fn institution_get() -> Result<InstitutionView, String> {
    view().await
}

#[tauri::command]
pub async fn institution_configure(
    app: tauri::AppHandle,
    address: String,
) -> Result<InstitutionView, String> {
    let wanted = normalise(&address)?;
    let changing = current().is_none_or(|i| i.canvas != wanted);
    if changing && library_exists().await? {
        return Err(LOCKED.to_string());
    }
    if changing {
        // While the old hosts are still the profile's, so the browser jar
        // loses the old session rather than keeping it.
        crate::auth::end_session(&app).await?;
    }
    let dir = crate::paths::data_dir();
    crate::blocking::run(move || configure(&dir, &address)).await?;
    view().await
}

pub const LOCKED: &str = "Your library was synced from this Canvas. Use Start over in \
     Settings → Storage to clear it before changing university.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    #[test]
    fn normalise_keeps_only_the_origin() {
        let ok = |s: &str| normalise(s).unwrap();
        assert_eq!(
            ok("canvas.lms.unimelb.edu.au"),
            "https://canvas.lms.unimelb.edu.au"
        );
        assert_eq!(
            ok(" RMIT.instructure.com/ "),
            "https://rmit.instructure.com"
        );
        assert_eq!(
            ok("https://rmit.instructure.com/courses/12/modules?x=1"),
            "https://rmit.instructure.com"
        );
        assert_eq!(ok("http://canvas.foo.edu"), "https://canvas.foo.edu");
        assert!(normalise("").is_err());
        assert!(normalise("localhost").is_err());
        assert!(normalise("ftp://canvas.foo.edu").is_err());
    }

    #[test]
    fn a_fresh_data_dir_has_no_profile() {
        let dir = Scratch::new("inst-fresh");
        // The database exists before anyone signs in, so it is no evidence.
        std::fs::write(crate::paths::db_path(&dir), b"").unwrap();
        assert_eq!(load(&dir), None);
        assert!(!path(&dir).exists());
    }

    #[test]
    fn an_older_install_is_given_the_unimelb_profile() {
        let dir = Scratch::new("inst-legacy");
        std::fs::write(crate::paths::cookie_path(&dir), b"canvas_session=x").unwrap();
        assert_eq!(load(&dir), Some(unimelb()));
        assert!(path(&dir).exists(), "the migration is written once");

        let synced = Scratch::new("inst-legacy-courses");
        std::fs::create_dir_all(synced.join("courses/COMP30026_2026_SM2")).unwrap();
        assert_eq!(load(&synced), Some(unimelb()));
    }

    #[test]
    fn a_saved_profile_round_trips_and_defaults_missing_fields() {
        let dir = Scratch::new("inst-roundtrip");
        std::fs::write(path(&dir), r#"{"canvas":"https://rmit.instructure.com"}"#).unwrap();
        let inst = load(&dir).unwrap();
        assert_eq!(inst.login_path, "/login");
        assert_eq!(inst.idp, Idp::Other);
        assert_eq!(inst.echo360_lead_in_secs, 0);

        save(&dir, &unimelb()).unwrap();
        assert_eq!(load(&dir), Some(unimelb()));
    }

    #[test]
    fn the_hemisphere_comes_from_either_host() {
        assert!(is_southern("https://canvas.lms.unimelb.edu.au", None));
        assert!(is_southern(
            "https://rmit.instructure.com",
            Some("myapps.rmit.edu.au")
        ));
        assert!(!is_southern(
            "https://canvas.harvard.edu",
            Some("key.harvard.edu")
        ));
        assert!(!is_southern("https://foo.instructure.com", None));
    }

    #[test]
    fn the_institution_domain_is_the_universitys_own() {
        let d = institution_domain;
        assert_eq!(
            d("canvas.lms.unimelb.edu.au", Some("sso.unimelb.edu.au")).as_deref(),
            Some("unimelb.edu.au")
        );
        assert_eq!(
            d("canvas.harvard.edu", Some("key.harvard.edu")).as_deref(),
            Some("harvard.edu")
        );
        assert_eq!(
            d("canvas.ox.ac.uk", Some("idp.ox.ac.uk")).as_deref(),
            Some("ox.ac.uk")
        );
        // A shared public suffix is not a domain anyone owns.
        assert_eq!(d("canvas.unimelb.edu.au", Some("sso.monash.edu.au")), None);
        assert_eq!(d("rmit.instructure.com", Some("myapps.rmit.edu.au")), None);
        assert_eq!(d("a.instructure.com", Some("b.instructure.com")), None);
        assert_eq!(d("canvas.foo.edu", None), None);
    }
}
