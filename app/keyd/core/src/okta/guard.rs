//! The attempt guard. The app's startup probe, its keep-alive thread, the
//! browser and the CLI's `auth tick` each sign in on their own, and Okta locks
//! the account after too many attempts. So every attempt goes through one
//! record on disk, and an automatic attempt that cannot reach it does not run.

use std::path::Path;

use super::LoginError;
use crate::paths;
use crate::platform::files;

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
    /// The name a client sends keyd, and keyd's reply log uses.
    pub fn wire_name(self) -> &'static str {
        match self {
            Trigger::Manual => "manual",
            Trigger::Startup => "startup",
            Trigger::KeepAlive => "keep-alive",
            Trigger::Browser => "browser",
        }
    }

    pub fn from_wire_name(name: &str) -> Option<Trigger> {
        [
            Trigger::Manual,
            Trigger::Startup,
            Trigger::KeepAlive,
            Trigger::Browser,
        ]
        .into_iter()
        .find(|t| t.wire_name() == name)
    }

    /// The label in `okta-sign-in.log`.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Trigger::Manual => "manual",
            Trigger::Startup => "app startup",
            Trigger::KeepAlive => "keep-alive",
            Trigger::Browser => "browser",
        }
    }
}

#[derive(Default)]
pub(super) struct AttemptRecord {
    /// Unix seconds when the last attempt started.
    last: u64,
    /// Failed attempts since the last success; sets the wait.
    failures: u32,
    /// Why automatic sign-in is paused, if it is.
    paused: Option<String>,
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

impl AttemptRecord {
    /// A record that is empty, or not in the shape `to_json` writes, acts as
    /// a blank one.
    fn from_json(text: &str) -> AttemptRecord {
        let parsed = serde_json::from_str::<serde_json::Value>(text).ok();
        let field = |key: &str| parsed.as_ref().and_then(|v| v.get(key));
        let (Some(last), Some(failures)) = (
            field("last").and_then(|v| v.as_u64()),
            field("failures")
                .and_then(|v| v.as_u64())
                .and_then(|n| u32::try_from(n).ok()),
        ) else {
            return AttemptRecord::default();
        };
        let paused = match field("paused") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(why)) => Some(why.clone()),
            Some(_) => return AttemptRecord::default(),
        };
        AttemptRecord {
            last,
            failures,
            paused,
        }
    }

    fn to_json(&self) -> String {
        format!(
            r#"{{"last":{},"failures":{},"paused":{}}}"#,
            self.last,
            self.failures,
            serde_json::to_string(&self.paused).unwrap_or_else(|_| "null".to_string())
        )
    }
}

/// Runs `f` on the record under an exclusive file lock, so two processes
/// cannot both decide to sign in, and saves what `f` left. `Err` says why the
/// record could not be locked, opened or saved; `f` has not run for the first
/// two. An unreadable record's contents act as a blank one.
pub(super) fn with_record<T>(
    data_dir: &Path,
    f: impl FnOnce(&mut AttemptRecord) -> T,
) -> Result<T, String> {
    use std::io::{Read, Seek, Write};

    let path = paths::sign_in_record(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let fail = |what: &str, e: std::io::Error| format!("{what} {}: {e}", path.display());
    let _lock = files::lock(&path).map_err(|e| fail("locking", e))?;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| fail("opening", e))?;
    let mut text = String::new();
    file.read_to_string(&mut text).ok();
    let mut record = AttemptRecord::from_json(&text);
    let out = f(&mut record);
    file.set_len(0)
        .and_then(|()| file.rewind())
        .and_then(|()| file.write_all(record.to_json().as_bytes()))
        .map_err(|e| fail("saving", e))?;
    Ok(out)
}

/// Whether an attempt may start at `now`, recording it if so. An automatic
/// attempt is refused when the record is out of reach: with no count of the
/// attempts before it, running could lock the account. A manual one runs,
/// since a person is waiting.
pub(super) fn admit_recorded(
    data_dir: &Path,
    trigger: Trigger,
    now: u64,
) -> Result<(), LoginError> {
    match with_record(data_dir, |r| admit(r, trigger, now)) {
        Ok(verdict) => verdict,
        Err(_) if trigger == Trigger::Manual => Ok(()),
        Err(why) => Err(LoginError::Paused(format!(
            "the attempt record could not be used, so there is no telling how recent the \
             last attempt was ({why})"
        ))),
    }
}

/// Folds a finished attempt into the record. Nothing is left to refuse when
/// the record is out of reach, so it is not reported.
pub(super) fn settle_recorded(data_dir: &Path, result: &Result<String, LoginError>) {
    with_record(data_dir, |r| settle(r, result)).ok();
}

/// Clears the wait and any pause: newly saved credentials, or a sign-in a
/// person finished in the window, deserve an immediate automatic try.
pub fn resume_automatic_sign_in(data_dir: &Path) {
    with_record(data_dir, |r| *r = AttemptRecord::default()).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_attempts_back_off_and_manual_ones_do_not() {
        let mut r = AttemptRecord::default();
        assert!(admit(&mut r, Trigger::Startup, 1_000_000).is_ok());
        settle(&mut r, &Err(LoginError::BadTotp(String::new())));
        assert!(matches!(
            admit(&mut r, Trigger::Browser, 1_000_599),
            Err(LoginError::Waiting(1))
        ));
        assert!(admit(&mut r, Trigger::KeepAlive, 1_000_600).is_ok());
        settle(&mut r, &Err(LoginError::Unexpected(String::new())));
        // Two failures in a row: an hour.
        assert!(matches!(
            admit(&mut r, Trigger::KeepAlive, 1_003_000),
            Err(LoginError::Waiting(_))
        ));
        assert!(admit(&mut r, Trigger::Manual, 1_003_000).is_ok());
        settle(&mut r, &Err(LoginError::BadTotp(String::new())));
        assert_eq!(wait_after(r.failures), 6 * 3600);
        settle(&mut r, &Ok(String::new()));
        assert_eq!(r.failures, 0);
    }

    #[test]
    fn a_network_failure_waits_without_counting() {
        let mut r = AttemptRecord::default();
        admit(&mut r, Trigger::Startup, 5_000).unwrap();
        settle(&mut r, &Err(LoginError::Network(String::new())));
        assert_eq!(r.failures, 0);
        assert!(matches!(
            admit(&mut r, Trigger::Startup, 5_100),
            Err(LoginError::Waiting(500))
        ));
    }

    #[test]
    fn a_lockout_pauses_automatic_sign_in_until_a_manual_success() {
        let mut r = AttemptRecord::default();
        admit(&mut r, Trigger::KeepAlive, 10_000).unwrap();
        settle(&mut r, &Err(LoginError::Locked("Too many attempts".into())));
        assert!(matches!(
            admit(&mut r, Trigger::KeepAlive, 1_000_000),
            Err(LoginError::Paused(_))
        ));
        assert!(admit(&mut r, Trigger::Manual, 1_000_000).is_ok());
        settle(&mut r, &Ok(String::new()));
        assert!(r.paused.is_none());
        assert!(admit(&mut r, Trigger::KeepAlive, 1_000_600).is_ok());
    }
    // ── On disk ──────────────────────────────────────────────────────────────

    use crate::test_support::Scratch;

    fn on_disk(dir: &Scratch) -> serde_json::Value {
        let text = std::fs::read_to_string(paths::sign_in_record(&dir.0)).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn an_admitted_attempt_is_written_down_and_the_next_automatic_one_waits() {
        let dir = Scratch::new("guard-wait");
        with_record(&dir.0, |r| admit(r, Trigger::Startup, 1_000))
            .unwrap()
            .unwrap();
        assert_eq!(on_disk(&dir)["last"], 1_000);

        assert!(matches!(
            admit_recorded(&dir.0, Trigger::KeepAlive, 1_000 + 599),
            Err(LoginError::Waiting(1))
        ));
        assert_eq!(on_disk(&dir)["last"], 1_000, "a refusal is not an attempt");
        assert!(admit_recorded(&dir.0, Trigger::KeepAlive, 1_000 + 600).is_ok());
        assert_eq!(on_disk(&dir)["last"], 1_600);
    }

    #[test]
    fn a_manual_attempt_ignores_the_wait_on_disk() {
        let dir = Scratch::new("guard-manual");
        admit_recorded(&dir.0, Trigger::Startup, 1_000).unwrap();
        assert!(admit_recorded(&dir.0, Trigger::Browser, 1_001).is_err());
        assert!(admit_recorded(&dir.0, Trigger::Manual, 1_001).is_ok());
        assert_eq!(on_disk(&dir)["last"], 1_001);
    }

    #[test]
    fn a_lockout_on_disk_pauses_automatic_attempts_until_a_manual_success() {
        let dir = Scratch::new("guard-lockout");
        admit_recorded(&dir.0, Trigger::KeepAlive, 10_000).unwrap();
        settle_recorded(&dir.0, &Err(LoginError::Locked("Too many attempts".into())));
        assert_eq!(on_disk(&dir)["failures"], 1);
        assert!(on_disk(&dir)["paused"].is_string());

        assert!(matches!(
            admit_recorded(&dir.0, Trigger::KeepAlive, 1_000_000),
            Err(LoginError::Paused(_))
        ));
        admit_recorded(&dir.0, Trigger::Manual, 1_000_000).unwrap();
        settle_recorded(&dir.0, &Ok(String::new()));
        assert!(on_disk(&dir)["paused"].is_null());
        assert!(admit_recorded(&dir.0, Trigger::KeepAlive, 1_000_600).is_ok());

        resume_automatic_sign_in(&dir.0);
        assert_eq!(on_disk(&dir)["last"], 0);
    }

    #[test]
    fn an_automatic_attempt_is_refused_when_the_record_cannot_be_opened() {
        let dir = Scratch::new("guard-closed");
        // A directory where the record belongs: it can be neither locked
        // for writing nor opened.
        std::fs::create_dir_all(paths::sign_in_record(&dir.0)).unwrap();

        assert!(with_record(&dir.0, |_| ()).is_err());
        for trigger in [Trigger::Startup, Trigger::KeepAlive, Trigger::Browser] {
            let Err(LoginError::Paused(why)) = admit_recorded(&dir.0, trigger, 1_000) else {
                panic!("{trigger:?} was admitted without a record");
            };
            assert!(why.contains("sign-in.json"), "{why}");
        }
        // A person is waiting on a manual one.
        assert!(admit_recorded(&dir.0, Trigger::Manual, 1_000).is_ok());
    }

    #[test]
    fn the_record_reads_back_what_it_wrote_and_a_damaged_one_is_blank() {
        let mut r = AttemptRecord {
            last: 77,
            failures: 3,
            paused: Some("locked \"out\"".into()),
        };
        let back = AttemptRecord::from_json(&r.to_json());
        assert_eq!((back.last, back.failures), (77, 3));
        assert_eq!(back.paused.as_deref(), Some("locked \"out\""));

        r.paused = None;
        assert!(AttemptRecord::from_json(&r.to_json()).paused.is_none());
        for damaged in [
            "",
            "{",
            "[]",
            r#"{"last":"x","failures":1}"#,
            r#"{"last":1}"#,
        ] {
            let blank = AttemptRecord::from_json(damaged);
            assert_eq!((blank.last, blank.failures, blank.paused), (0, 0, None));
        }
    }
}
