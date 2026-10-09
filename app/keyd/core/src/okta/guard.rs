//! The attempt guard. The app's startup probe, its keep-alive thread, the
//! browser and the CLI's `auth tick` each sign in on their own, and Okta locks
//! the account after too many attempts. So every attempt goes through one
//! record on disk (`record.rs`), and an attempt that cannot reach it or cannot
//! read it does not run, unless the app asked for it by hand.
//!
//! The rules, in `admit`: no two attempts start within `MIN_SPACING`;
//! automatic ones also wait out the back-off and stop at a pause; a manual one
//! skips both, except that three failed manual attempts in a row make it wait
//! like an automatic one, and that a pause from a lockout or a rejected
//! password lifts only for a manual attempt from the app.

use std::path::Path;

use super::LoginError;
use crate::platform::Role;

mod record;
mod trigger;

#[cfg(test)]
mod tests;

pub(super) use record::{with_record, AttemptRecord};
pub use trigger::Trigger;

/// The least time between the start of one attempt and the next, whoever asks.
pub(super) const MIN_SPACING: u64 = 60;

/// Consecutive failed manual attempts after which a manual attempt is held to
/// the automatic back-off. A success or newly saved credentials start over.
pub(super) const MANUAL_ATTEMPTS: u32 = 3;

/// How long automatic sign-in waits after an attempt.
pub(super) fn wait_after(failures: u32) -> u64 {
    match failures {
        0 | 1 => 600,
        2 => 3600,
        _ => 6 * 3600,
    }
}

/// Whether an attempt by `role` may start at `now`, recording it if so.
pub(super) fn admit(
    r: &mut AttemptRecord,
    trigger: Trigger,
    role: Role,
    now: u64,
) -> Result<(), LoginError> {
    if let Some(damage) = &r.damaged {
        if trigger != Trigger::Manual {
            return Err(LoginError::Paused(format!(
                "the attempt record {damage}, so there is no telling how recent the last \
                 attempt was"
            )));
        }
        // A person is waiting. The save after this attempt writes a good record.
        r.damaged = None;
    }
    let held = trigger != Trigger::Manual || r.manual_failures >= MANUAL_ATTEMPTS;
    if held {
        if let Some(why) = &r.paused {
            return Err(LoginError::Paused(why.clone()));
        }
        let wait = if r.forgiven {
            0
        } else {
            wait_after(r.failures)
        };
        let ready = r.last.saturating_add(wait);
        if now < ready {
            return Err(LoginError::Waiting(ready - now));
        }
    } else if let Some(why) = &r.paused {
        // The lockout or the password must be fixed by someone who can see it.
        if r.credentials_paused && role != Role::App {
            return Err(LoginError::Paused(why.clone()));
        }
    }
    let ready = r.last.saturating_add(MIN_SPACING);
    if now < ready {
        return Err(LoginError::Waiting(ready - now));
    }
    r.last = now;
    r.forgiven = false;
    Ok(())
}

/// Fold an attempt's outcome into the record.
pub(super) fn settle(r: &mut AttemptRecord, trigger: Trigger, result: &Result<String, LoginError>) {
    let failed = |r: &mut AttemptRecord| {
        r.failures = r.failures.saturating_add(1);
        if trigger == Trigger::Manual {
            r.manual_failures = r.manual_failures.saturating_add(1);
        }
    };
    match result {
        Ok(_) => {
            r.failures = 0;
            r.manual_failures = 0;
            r.paused = None;
            r.credentials_paused = false;
        }
        // Okta gave no verdict, so it does not count against the account.
        Err(LoginError::Network(_)) => {}
        Err(e @ (LoginError::Locked(_) | LoginError::BadPassword(_))) => {
            failed(r);
            r.paused = Some(e.to_string());
            r.credentials_paused = true;
        }
        Err(e @ LoginError::UnsupportedFactor(_)) => {
            failed(r);
            r.paused = Some(e.to_string());
            r.credentials_paused = false;
        }
        Err(_) => failed(r),
    }
}

/// Forgives the failures, the pause and the back-off, but not the spacing:
/// the next attempt of any kind still starts `MIN_SPACING` after the last.
fn resume(r: &mut AttemptRecord) {
    *r = AttemptRecord {
        last: r.last,
        forgiven: true,
        ..AttemptRecord::default()
    };
}

/// Whether an attempt by `role` may start at `now`, recording it if so. An
/// attempt is refused when the record is out of reach: with no count of the
/// attempts before it, running could lock the account. Only a manual one from
/// the app runs anyway, since a person is waiting.
pub(super) fn admit_recorded(
    data_dir: &Path,
    trigger: Trigger,
    role: Role,
    now: u64,
) -> Result<(), LoginError> {
    match with_record(data_dir, |r| admit(r, trigger, role, now)) {
        Ok(verdict) => verdict,
        Err(_) if trigger == Trigger::Manual && role == Role::App => Ok(()),
        Err(why) => Err(LoginError::Paused(format!(
            "the attempt record could not be used, so there is no telling how recent the \
             last attempt was ({why})"
        ))),
    }
}

/// Folds a finished attempt into the record. Nothing is left to refuse when
/// the record is out of reach, so it is not reported.
pub(super) fn settle_recorded(
    data_dir: &Path,
    trigger: Trigger,
    result: &Result<String, LoginError>,
) {
    with_record(data_dir, |r| settle(r, trigger, result)).ok();
}

/// Clears the failures, the pause and the wait: newly saved credentials, or a
/// sign-in a person finished in the window, deserve an automatic try (once
/// `MIN_SPACING` has passed). A damaged record is repaired by it. `Err` says
/// why the record could not be replaced.
pub fn resume_automatic_sign_in(data_dir: &Path) -> Result<(), String> {
    with_record(data_dir, resume)
}
