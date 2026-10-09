//! The attempt guard.
//!
//! The app's startup probe, its keep-alive thread, the browser and the CLI's
//! `auth tick` each sign in on their own, and Okta locks the account after too
//! many attempts. So every attempt goes through one record on disk.

use super::flow::attempt_sign_in;
use super::http::LoginError;
use super::store::{clear_password, Credentials};

/// Who asked for a sign-in. Only `Manual` skips the guard: a person is
/// waiting on the answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    Manual,
    Startup,
    KeepAlive,
    Browser,
}

impl Trigger {
    fn as_str(self) -> &'static str {
        match self {
            Trigger::Manual => "manual",
            Trigger::Startup => "app startup",
            Trigger::KeepAlive => "keep-alive",
            Trigger::Browser => "browser",
        }
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub(super) struct AttemptRecord {
    /// Unix seconds when the last attempt started.
    last: u64,
    /// Failed attempts since the last success; sets the wait.
    pub(super) failures: u32,
    /// Why automatic sign-in is paused, if it is.
    pub(super) paused: Option<String>,
}

/// How long automatic sign-in waits after an attempt.
pub(super) fn wait_after(failures: u32) -> u64 {
    match failures {
        0 | 1 => 600,
        2 => 3600,
        _ => 6 * 3600,
    }
}

/// Whether an attempt may start at `now`, recording it if so.
pub(super) fn admit(r: &mut AttemptRecord, trigger: Trigger, now: u64) -> Result<(), LoginError> {
    if trigger != Trigger::Manual {
        if let Some(why) = &r.paused {
            return Err(LoginError::Paused(why.clone()));
        }
        let ready = r.last.saturating_add(wait_after(r.failures));
        if now < ready {
            return Err(LoginError::Waiting(ready - now));
        }
    }
    r.last = now;
    Ok(())
}

/// Fold an attempt's outcome into the record.
pub(super) fn settle(r: &mut AttemptRecord, result: &Result<String, LoginError>) {
    match result {
        Ok(_) => {
            r.failures = 0;
            r.paused = None;
        }
        // Okta gave no verdict, so it does not count against the account.
        Err(LoginError::Network(_)) => {}
        Err(
            e @ (LoginError::Locked(_)
            | LoginError::BadPassword(_)
            | LoginError::UnsupportedFactor(_)),
        ) => {
            r.failures += 1;
            r.paused = Some(e.to_string());
        }
        Err(_) => r.failures += 1,
    }
}

/// Runs `f` on the record under an exclusive file lock, so two processes
/// cannot both decide to sign in. An unreadable record acts as a blank one.
fn with_record<T>(data_dir: &std::path::Path, f: impl FnOnce(&mut AttemptRecord) -> T) -> T {
    use std::io::{Read, Seek, Write};

    let path = crate::library::paths::sign_in_record_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
    else {
        return f(&mut AttemptRecord::default());
    };
    file.lock().ok();
    let mut text = String::new();
    file.read_to_string(&mut text).ok();
    let mut record: AttemptRecord = serde_json::from_str(&text).unwrap_or_default();
    let out = f(&mut record);
    if let Ok(body) = serde_json::to_string(&record) {
        file.set_len(0).ok();
        file.rewind().ok();
        file.write_all(body.as_bytes()).ok();
    }
    out
}

/// Headless sign-in behind the attempt guard. Automatic attempts wait 10 min
/// after any attempt, then 1 h and 6 h as failures repeat, and stop on a
/// failure retrying cannot fix. Each attempt is a line in `okta-sign-in.log`.
pub fn sign_in(data_dir: &std::path::Path, trigger: Trigger) -> Result<String, LoginError> {
    if trigger != Trigger::Manual && crate::library::paths::signed_out_path(data_dir).exists() {
        return Err(LoginError::SignedOut);
    }
    let creds = Credentials::load()?.ok_or(LoginError::NotConfigured)?;
    let now = crate::runtime::clock::now_secs();
    with_record(data_dir, |r| admit(r, trigger, now))?;

    let result = attempt_sign_in(data_dir, &creds);
    with_record(data_dir, |r| settle(r, &result));
    let outcome = match &result {
        Ok(_) => "signed in".to_string(),
        Err(e) => format!("failed — {e}"),
    };
    crate::library::paths::append_sign_in_log(
        data_dir,
        &format!("{}: {outcome}", trigger.as_str()),
    );
    if let Err(LoginError::BadPassword(_)) = &result {
        // Replaying a wrong password unattended locks the account.
        clear_password().ok();
    }
    result
}

/// Clears the wait and any pause: newly saved credentials, or a sign-in a
/// person finished in the window, deserve an immediate automatic try.
pub fn resume_automatic_sign_in(data_dir: &std::path::Path) {
    with_record(data_dir, |r| *r = AttemptRecord::default());
}
