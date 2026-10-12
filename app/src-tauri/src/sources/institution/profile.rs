//! Reading and writing `institution.json`, and the questions the rest of the
//! app asks of the current profile.

use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::SystemTime;

use super::{unimelb, Institution, NOT_CONFIGURED};

pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("institution.json")
}

/// Read the profile. A missing file beside sign-in or sync state means an
/// install from before the profile existed, which was UniMelb's, so that
/// profile is written and returned. A fresh install has neither and gets
/// `None` until the student enters their Canvas address.
pub fn load(data_dir: &Path) -> Option<Institution> {
    match std::fs::read_to_string(path(data_dir)) {
        Ok(raw) => match serde_json::from_str(&raw) {
            Ok(inst) => Some(inst),
            Err(e) => {
                elog!("[oculus] institution.json is unreadable: {e}");
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && predates_profile(data_dir) => {
            let inst = unimelb();
            if let Err(e) = save(data_dir, &inst) {
                elog!("[oculus] could not write institution.json: {e}");
            }
            Some(inst)
        }
        Err(_) => None,
    }
}

/// Only a sign-in or a sync writes these, and neither runs without a profile,
/// so any of them means the data dir is older than the profile. The database
/// and `agents/` are no evidence: a fresh install creates them before anyone
/// signs in.
fn predates_profile(data_dir: &Path) -> bool {
    use keyd_core::paths;

    let courses = data_dir.join("courses");
    paths::cookie(data_dir).exists()
        || paths::authenticated(data_dir).exists()
        || paths::signed_out(data_dir).exists()
        || paths::sign_in_record(data_dir).exists()
        || std::fs::read_dir(&courses).is_ok_and(|mut d| d.next().is_some())
}

pub fn save(data_dir: &Path, inst: &Institution) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let body = serde_json::to_vec_pretty(inst).map_err(|e| e.to_string())?;
    let target = path(data_dir);
    crate::runtime::atomic_write::write(&target, &target.with_extension("json.tmp"), &body)
}

/// Keyed on the file's mtime, so a profile the CLI writes reaches a running
/// app (and the reverse) on the next read.
static CACHE: RwLock<Option<(SystemTime, Option<Institution>)>> = RwLock::new(None);

/// The profile for the app's data dir.
pub fn current() -> Option<Institution> {
    let dir = crate::library::paths::data_dir();
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
    current().is_some_and(|i| i.is_okta())
}

pub fn host_of(base: &str) -> Option<String> {
    url::Url::parse(base)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
}

/// Record a fact a launch discovered. oculus-keyd hears first when it moves
/// an origin keyd sends to, so the two never disagree. Whether the profile
/// now holds the fact: false with no profile, when keyd refused the move, or
/// when the file could not be written (each logged).
pub fn learn(data_dir: &Path, update: impl FnOnce(&mut Institution)) -> bool {
    let Some(mut inst) = load(data_dir) else {
        return false;
    };
    let before = inst.clone();
    update(&mut inst);
    if inst == before {
        return true;
    }
    if inst.site() == before.site() {
        if let Err(e) = save(data_dir, &inst) {
            elog!("[oculus] could not update institution.json: {e}");
            return false;
        }
        return true;
    }
    let saved = super::keyd::holding(|told| {
        super::keyd::tell_held(told, data_dir, &inst)
            .map_err(|e| format!("oculus-keyd did not take the new site: {e}"))?;
        save(data_dir, &inst).map_err(|e| format!("could not update institution.json: {e}"))
    });
    if let Err(e) = saved {
        elog!("[oculus] {e}");
        return false;
    }
    true
}
