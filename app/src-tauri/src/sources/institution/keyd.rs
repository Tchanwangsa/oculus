//! Telling oculus-keyd which university's origins to send the session to
//! (`keyd_core::site`), and refusing every credential call until it has
//! taken them. keyd drops what it held for an origin that moved, so it hears
//! before the profile is saved: if it refuses, nothing changes.
//!
//! `bound` is the check keyd's client runs before any call that hands keyd a
//! credential or has it use one (`keyd_core::client::bound`). It passes once
//! keyd has taken the profile's site in this process, telling it first when it
//! has not; a keyd too old for `site_set` passes only at UniMelb, the site it
//! serves untold. A profile change holds the lock from telling keyd to saving
//! the file, so no call re-tells keyd the old site in between.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use keyd_core::site::Site;

use super::Institution;
use crate::providers::credentials::{Credentialed, KeydError};

/// What keyd took in this process.
#[derive(Default)]
pub(super) struct Told {
    /// The site keyd last took; `None` before then, and after keyd was absent.
    site: Option<Site>,
    /// Settings is moving to another address: the old session is being
    /// dropped and the new one cannot exist yet.
    moving: bool,
}

impl Told {
    #[cfg(test)]
    pub(super) fn site(&self) -> Option<&Site> {
        self.site.as_ref()
    }
}

static TOLD: Mutex<Told> = Mutex::new(Told {
    site: None,
    moving: false,
});

fn told() -> MutexGuard<'static, Told> {
    TOLD.lock().unwrap_or_else(|e| e.into_inner())
}

/// Runs a profile change with keyd's site held still: `f` tells keyd
/// (`tell_held`) and saves the profile before any credential call can
/// compare the two.
pub(super) fn holding<R>(f: impl FnOnce(&mut Told) -> R) -> R {
    f(&mut told())
}

/// Credential calls are refused until the returned guard drops: Settings is
/// signing out of one university before the next one's profile is saved.
pub(super) fn moving() -> Moving {
    told().moving = true;
    Moving
}

pub(super) struct Moving;

impl Drop for Moving {
    fn drop(&mut self) {
        told().moving = false;
    }
}

/// Sets keyd's site to `inst`'s. With keyd absent there is no session
/// anywhere to send, so nothing to tell; the next credential call tells it.
pub(super) fn tell(data_dir: &Path, inst: &Institution) -> Result<(), String> {
    holding(|told| tell_held(told, data_dir, inst)).map_err(|e| e.to_string())
}

pub(super) fn tell_held(
    told: &mut Told,
    data_dir: &Path,
    inst: &Institution,
) -> Result<(), KeydError> {
    let site = inst.site();
    match Credentialed::at(data_dir).site_set(&site) {
        Ok(change) => {
            if !change.cleared.is_empty() || change.forgot_okta {
                elog!(
                    "[oculus] oculus-keyd moved to {}: dropped {} session(s){}",
                    inst.canvas,
                    change.cleared.len(),
                    if change.forgot_okta {
                        " and the Okta credentials"
                    } else {
                        ""
                    }
                );
            }
            told.site = Some(site);
            Ok(())
        }
        Err(KeydError::Absent) => {
            told.site = None;
            Ok(())
        }
        // A keyd from before `site_set` serves UniMelb and cannot be moved.
        Err(KeydError::Request(detail)) if detail.contains("unknown op") => {
            if site == Site::unimelb() {
                told.site = Some(site);
                Ok(())
            } else {
                Err(KeydError::Site(outdated(&inst.canvas)))
            }
        }
        Err(e) => Err(e),
    }
}

fn outdated(canvas: &str) -> String {
    format!(
        "oculus-keyd is too old to be told your university, so Oculus will not \
         give it your {canvas} session: it would send it to the University of \
         Melbourne. Update oculus-keyd (a release reinstalls it at launch; a dev \
         build needs `bun run keyd`) and restart Oculus."
    )
}

/// The check every credential call through keyd passes first
/// (`keyd_core::client::bound::bind`): the Canvas origin keyd has taken for
/// the profile, or why the call may not go.
pub fn bound() -> Result<String, KeydError> {
    let dir = crate::library::paths::data_dir();
    let Some(inst) = super::load(&dir) else {
        return Err(KeydError::Site(super::NOT_CONFIGURED.to_string()));
    };
    let site = inst.site();
    let mut told = told();
    if told.moving {
        return Err(KeydError::Site(
            "Oculus is changing university; try again once Settings → Canvas has saved it."
                .to_string(),
        ));
    }
    if told.site.as_ref() != Some(&site) {
        tell_held(&mut told, &dir, &inst)?;
    }
    match &told.site {
        Some(taken) if *taken == site => Ok(site.canvas),
        _ => Err(KeydError::Absent),
    }
}

/// At startup, before the session is restored: a keyd installed after the
/// profile was saved, or one an update replaced, learns the site here. A
/// failure is logged here and refuses every credential call after it, each
/// with the reason.
pub fn tell_keyd() {
    let dir = crate::library::paths::data_dir();
    let Some(inst) = super::load(&dir) else {
        return;
    };
    if let Err(e) = tell(&dir, &inst) {
        elog!("[oculus] oculus-keyd did not take the site: {e}");
    }
}
