//! The attempt guard. The app's startup probe, its keep-alive thread, the
//! browser and the CLI's `auth tick` each sign in on their own, and Okta locks
//! the account after too many attempts. So every attempt goes through one
//! record on disk (`record.rs`), and an automatic attempt that cannot reach it
//! or cannot read it does not run.

use std::path::Path;

use super::LoginError;

mod record;

pub(super) use record::{with_record, AttemptRecord};

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
/// A damaged record is repaired by it. `Err` says why the record could not be
/// replaced.
pub fn resume_automatic_sign_in(data_dir: &Path) -> Result<(), String> {
    with_record(data_dir, |r| *r = AttemptRecord::default())
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

    use crate::paths;
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

        resume_automatic_sign_in(&dir.0).unwrap();
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

    fn write_record(dir: &Scratch, bytes: &[u8]) {
        let path = paths::sign_in_record(&dir.0);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn record_bytes(dir: &Scratch) -> Vec<u8> {
        std::fs::read(paths::sign_in_record(&dir.0)).unwrap()
    }

    /// Everything that is not a record, and so must not read as a blank one.
    const DAMAGED: [&[u8]; 11] = [
        b"",
        b"  \n",
        b"{",
        b"[]",
        b"null",
        br#"{"last":1}"#,
        br#"{"failures":1}"#,
        br#"{"last":"x","failures":1}"#,
        br#"{"last":1,"failures":-1,"paused":null}"#,
        br#"{"last":1,"failures":1,"paused":5}"#,
        b"\xff\xfe not text",
    ];

    #[test]
    fn a_damaged_record_pauses_every_automatic_attempt_and_is_left_alone() {
        for bytes in DAMAGED {
            let dir = Scratch::new("guard-damaged");
            write_record(&dir, bytes);
            for trigger in [Trigger::Startup, Trigger::KeepAlive, Trigger::Browser] {
                let Err(LoginError::Paused(why)) = admit_recorded(&dir.0, trigger, 1_000_000)
                else {
                    panic!("{trigger:?} ran on {:?}", String::from_utf8_lossy(bytes));
                };
                assert!(
                    why.contains("sign-in.json is damaged") && why.contains("no telling"),
                    "{why}"
                );
            }
            // Refusing did not repair it into a blank record.
            assert_eq!(record_bytes(&dir), bytes);
        }
    }

    #[test]
    fn a_missing_record_is_a_blank_one_and_the_first_attempt_runs() {
        let dir = Scratch::new("guard-absent");
        assert!(!paths::sign_in_record(&dir.0).exists());
        assert!(admit_recorded(&dir.0, Trigger::Startup, 1_000_000).is_ok());
        assert_eq!(on_disk(&dir)["last"], 1_000_000);
    }

    #[test]
    fn a_manual_attempt_repairs_a_damaged_record() {
        for bytes in DAMAGED {
            let dir = Scratch::new("guard-repair");
            write_record(&dir, bytes);
            admit_recorded(&dir.0, Trigger::Manual, 1_000_000).unwrap();
            settle_recorded(&dir.0, &Ok(String::new()));
            let repaired = on_disk(&dir);
            assert_eq!(repaired["last"], 1_000_000);
            assert_eq!(repaired["failures"], 0);
            assert!(repaired["paused"].is_null());
            // Automatic attempts judge the repaired record as any other.
            assert!(matches!(
                admit_recorded(&dir.0, Trigger::Startup, 1_000_100),
                Err(LoginError::Waiting(500))
            ));
            assert!(admit_recorded(&dir.0, Trigger::Startup, 1_000_600).is_ok());
        }
    }

    #[test]
    fn a_failed_manual_attempt_on_a_damaged_record_records_its_failure_and_pause() {
        let dir = Scratch::new("guard-repair-fail");
        write_record(&dir, b"");
        admit_recorded(&dir.0, Trigger::Manual, 1_000_000).unwrap();
        settle_recorded(&dir.0, &Err(LoginError::Locked("Too many attempts".into())));
        assert_eq!(on_disk(&dir)["failures"], 1);
        assert!(on_disk(&dir)["paused"].is_string());
        assert!(matches!(
            admit_recorded(&dir.0, Trigger::KeepAlive, 9_000_000),
            Err(LoginError::Paused(_))
        ));
    }

    #[test]
    fn saving_new_credentials_or_a_browser_sign_in_repairs_a_damaged_record() {
        let dir = Scratch::new("guard-resume-repair");
        write_record(&dir, b"{");
        resume_automatic_sign_in(&dir.0).unwrap();
        assert_eq!(on_disk(&dir)["failures"], 0);
        assert!(admit_recorded(&dir.0, Trigger::Startup, 1_000).is_ok());
    }

    #[test]
    fn a_damaged_record_cannot_be_used_to_hammer_in_parallel() {
        let dir = Scratch::new("guard-damaged-threads");
        write_record(&dir, b"");
        let path = dir.0.clone();
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || admit_recorded(&path, Trigger::KeepAlive, 5_000))
            })
            .collect();
        for t in threads {
            assert!(matches!(t.join().unwrap(), Err(LoginError::Paused(_))));
        }
        assert_eq!(record_bytes(&dir), b"");
    }

    #[test]
    fn only_one_of_many_simultaneous_automatic_attempts_is_admitted() {
        let dir = Scratch::new("guard-threads");
        let path = dir.0.clone();
        let threads: Vec<_> = (0..16)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || admit_recorded(&path, Trigger::Startup, 5_000))
            })
            .collect();
        let verdicts: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        assert_eq!(verdicts.iter().filter(|v| v.is_ok()).count(), 1);
        assert!(verdicts
            .iter()
            .filter_map(|v| v.as_ref().err())
            .all(|e| matches!(e, LoginError::Waiting(600))));
    }

    #[test]
    fn a_save_interrupted_before_the_rename_leaves_the_record_as_it_was() {
        let dir = Scratch::new("guard-interrupted");
        write_record(&dir, br#"{"last":777,"failures":2,"paused":"locked"}"#);
        // A crash mid-write leaves a partial temp file beside the record.
        let temp = paths::sign_in_record(&dir.0)
            .with_file_name(format!(".sign-in.json.{}.tmp", std::process::id()));
        std::fs::write(&temp, b"{\"last\":9").unwrap();
        assert_eq!(
            record_bytes(&dir),
            br#"{"last":777,"failures":2,"paused":"locked"}"#
        );

        // The next reader sees the old record, pause and all.
        let Err(LoginError::Paused(why)) = admit_recorded(&dir.0, Trigger::KeepAlive, 9_000_000)
        else {
            panic!("the pause was forgotten");
        };
        assert_eq!(why, "locked");
        // And a save over the stale temp completes.
        resume_automatic_sign_in(&dir.0).unwrap();
        assert_eq!(on_disk(&dir)["failures"], 0);
        assert!(!temp.exists(), "the temp file was renamed over the record");
    }

    #[test]
    fn a_save_that_fails_leaves_the_old_record_and_says_so() {
        let dir = Scratch::new("guard-save-fails");
        write_record(&dir, br#"{"last":777,"failures":1,"paused":null}"#);
        // The temp file's name is taken by a directory, so it cannot be written.
        let temp = paths::sign_in_record(&dir.0)
            .with_file_name(format!(".sign-in.json.{}.tmp", std::process::id()));
        std::fs::create_dir(&temp).unwrap();

        let why = with_record(&dir.0, |r| r.last = 5).unwrap_err();
        assert!(
            why.starts_with("saving") && why.contains("sign-in.json"),
            "{why}"
        );
        assert_eq!(
            record_bytes(&dir),
            br#"{"last":777,"failures":1,"paused":null}"#
        );
        // An automatic attempt that cannot be recorded does not run.
        assert!(matches!(
            admit_recorded(&dir.0, Trigger::Startup, 9_000_000),
            Err(LoginError::Paused(_))
        ));
    }

    #[test]
    fn an_old_layout_record_without_a_lock_file_is_read_and_a_lock_file_appears() {
        let dir = Scratch::new("guard-old-layout");
        write_record(&dir, br#"{"last":1000,"failures":0,"paused":null}"#);
        assert!(!paths::sign_in_lock(&dir.0).exists());
        assert!(matches!(
            admit_recorded(&dir.0, Trigger::Startup, 1_100),
            Err(LoginError::Waiting(500))
        ));
        assert!(paths::sign_in_lock(&dir.0).exists());
        assert_ne!(
            paths::sign_in_lock(&dir.0),
            paths::sign_in_record(&dir.0),
            "the lock is not the record's own file"
        );
    }

    #[test]
    fn the_record_and_its_lock_are_private_to_this_user() {
        let dir = Scratch::new("guard-modes");
        admit_recorded(&dir.0, Trigger::Startup, 1_000).unwrap();
        for path in [paths::sign_in_record(&dir.0), paths::sign_in_lock(&dir.0)] {
            assert!(
                crate::platform::files::is_owner_only(&path).unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
