use super::*;
use crate::paths;
use crate::test_support::Scratch;

fn on_disk(dir: &Scratch) -> serde_json::Value {
    let text = std::fs::read_to_string(paths::sign_in_record(&dir.0)).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn admit_as(dir: &Scratch, trigger: Trigger, role: Role, now: u64) -> Result<(), LoginError> {
    admit_recorded(&dir.0, trigger, role, now)
}

#[test]
fn an_admitted_attempt_is_written_down_and_the_next_automatic_one_waits() {
    let dir = Scratch::new("guard-wait");
    with_record(&dir.0, |r| admit(r, Trigger::Startup, APP, 1_000))
        .unwrap()
        .unwrap();
    assert_eq!(on_disk(&dir)["last"], 1_000);

    assert!(matches!(
        admit_as(&dir, Trigger::Startup, APP, 1_000 + 599),
        Err(LoginError::Waiting(1))
    ));
    assert_eq!(on_disk(&dir)["last"], 1_000, "a refusal is not an attempt");
    assert!(admit_as(&dir, Trigger::Startup, APP, 1_000 + 600).is_ok());
    assert_eq!(on_disk(&dir)["last"], 1_600);
}

#[test]
fn a_manual_attempt_skips_the_backoff_on_disk_but_not_the_minute() {
    let dir = Scratch::new("guard-manual");
    admit_as(&dir, Trigger::Startup, APP, 1_000).unwrap();
    assert!(admit_as(&dir, Trigger::Browser, APP, 1_001).is_err());
    assert!(matches!(
        admit_as(&dir, Trigger::Manual, CLI, 1_001),
        Err(LoginError::Waiting(59))
    ));
    assert!(admit_as(&dir, Trigger::Manual, CLI, 1_060).is_ok());
    assert_eq!(on_disk(&dir)["last"], 1_060);
}

#[test]
fn manual_failures_and_the_kind_of_pause_are_kept_between_processes() {
    let dir = Scratch::new("guard-kept");
    admit_as(&dir, Trigger::Manual, APP, T).unwrap();
    settle_recorded(
        &dir.0,
        Trigger::Manual,
        &Err(LoginError::Locked("locked".into())),
    );
    let record = on_disk(&dir);
    assert_eq!(record["manual_failures"], 1);
    assert_eq!(record["credentials_paused"], true);

    assert!(matches!(
        admit_as(&dir, Trigger::Manual, CLI, T + 120),
        Err(LoginError::Paused(_))
    ));
    assert!(admit_as(&dir, Trigger::Manual, APP, T + 120).is_ok());
}

#[test]
fn a_record_from_before_these_rules_is_judged_by_them() {
    let dir = Scratch::new("guard-old-rules");
    write_record(
        &dir,
        br#"{"last":1000,"failures":1,"paused":"Okta rejected the password: x"}"#,
    );
    // It cannot say what paused it, so the CLI cannot lift it.
    assert!(matches!(
        admit_as(&dir, Trigger::Manual, CLI, 5_000),
        Err(LoginError::Paused(_))
    ));
    assert!(admit_as(&dir, Trigger::Manual, APP, 5_000).is_ok());
}

#[test]
fn a_lockout_on_disk_pauses_automatic_attempts_until_a_manual_success() {
    let dir = Scratch::new("guard-lockout");
    admit_as(&dir, Trigger::Startup, APP, 10_000).unwrap();
    settle_recorded(
        &dir.0,
        Trigger::Startup,
        &Err(LoginError::Locked("Too many attempts".into())),
    );
    assert_eq!(on_disk(&dir)["failures"], 1);
    assert!(on_disk(&dir)["paused"].is_string());

    assert!(matches!(
        admit_as(&dir, Trigger::Startup, APP, 1_000_000),
        Err(LoginError::Paused(_))
    ));
    admit_as(&dir, Trigger::Manual, APP, 1_000_000).unwrap();
    settle_recorded(&dir.0, Trigger::Manual, &ok());
    assert!(on_disk(&dir)["paused"].is_null());
    assert!(admit_as(&dir, Trigger::Startup, APP, 1_000_600).is_ok());

    resume_automatic_sign_in(&dir.0).unwrap();
    assert_eq!(on_disk(&dir)["failures"], 0);
}

#[test]
fn an_attempt_is_refused_when_the_record_cannot_be_opened_unless_the_app_asked_by_hand() {
    let dir = Scratch::new("guard-closed");
    // A directory where the record belongs: it cannot be read.
    std::fs::create_dir_all(paths::sign_in_record(&dir.0)).unwrap();

    assert!(with_record(&dir.0, |_| ()).is_err());
    for trigger in [Trigger::Startup, Trigger::Browser] {
        let Err(LoginError::Paused(why)) = admit_as(&dir, trigger, APP, 1_000) else {
            panic!("{trigger:?} was admitted without a record");
        };
        assert!(why.contains("sign-in.json"), "{why}");
    }
    // A person is waiting on a manual one from the app; nothing counts the
    // attempts of a command, which could be a loop.
    assert!(admit_as(&dir, Trigger::Manual, APP, 1_000).is_ok());
    let Err(LoginError::Paused(why)) = admit_as(&dir, Trigger::Manual, CLI, 1_000) else {
        panic!("a manual attempt from the CLI ran without a record");
    };
    assert!(why.contains("sign-in.json"), "{why}");
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
        for trigger in [Trigger::Startup, Trigger::Browser] {
            let Err(LoginError::Paused(why)) = admit_as(&dir, trigger, APP, T) else {
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
    assert!(admit_as(&dir, Trigger::Startup, APP, T).is_ok());
    assert_eq!(on_disk(&dir)["last"], T);
}

#[test]
fn a_manual_attempt_repairs_a_damaged_record() {
    for bytes in DAMAGED {
        for role in [APP, CLI] {
            let dir = Scratch::new("guard-repair");
            write_record(&dir, bytes);
            admit_as(&dir, Trigger::Manual, role, T).unwrap();
            settle_recorded(&dir.0, Trigger::Manual, &ok());
            let repaired = on_disk(&dir);
            assert_eq!(repaired["last"], T);
            assert_eq!(repaired["failures"], 0);
            assert!(repaired["paused"].is_null());
            // Automatic attempts judge the repaired record as any other.
            assert!(matches!(
                admit_as(&dir, Trigger::Startup, APP, T + 100),
                Err(LoginError::Waiting(500))
            ));
            assert!(admit_as(&dir, Trigger::Startup, APP, T + 600).is_ok());
        }
    }
}

#[test]
fn a_failed_manual_attempt_on_a_damaged_record_records_its_failure_and_pause() {
    let dir = Scratch::new("guard-repair-fail");
    write_record(&dir, b"");
    admit_as(&dir, Trigger::Manual, CLI, T).unwrap();
    settle_recorded(
        &dir.0,
        Trigger::Manual,
        &Err(LoginError::Locked("Too many attempts".into())),
    );
    assert_eq!(on_disk(&dir)["failures"], 1);
    assert!(on_disk(&dir)["paused"].is_string());
    assert!(matches!(
        admit_as(&dir, Trigger::Startup, APP, 9_000_000),
        Err(LoginError::Paused(_))
    ));
}

#[test]
fn saving_new_credentials_or_a_browser_sign_in_repairs_a_damaged_record() {
    let dir = Scratch::new("guard-resume-repair");
    write_record(&dir, b"{");
    resume_automatic_sign_in(&dir.0).unwrap();
    assert_eq!(on_disk(&dir)["failures"], 0);
    assert!(admit_as(&dir, Trigger::Startup, APP, 1_000).is_ok());
}

#[test]
fn a_damaged_record_cannot_be_used_to_hammer_in_parallel() {
    let dir = Scratch::new("guard-damaged-threads");
    write_record(&dir, b"");
    let path = dir.0.clone();
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let path = path.clone();
            std::thread::spawn(move || admit_recorded(&path, Trigger::Startup, APP, 5_000))
        })
        .collect();
    for t in threads {
        assert!(matches!(t.join().unwrap(), Err(LoginError::Paused(_))));
    }
    assert_eq!(record_bytes(&dir), b"");
}

#[test]
fn only_one_of_many_simultaneous_attempts_is_admitted() {
    for trigger in [Trigger::Startup, Trigger::Manual] {
        let dir = Scratch::new("guard-threads");
        let path = dir.0.clone();
        let threads: Vec<_> = (0..16)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || admit_recorded(&path, trigger, APP, 5_000))
            })
            .collect();
        let verdicts: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        assert_eq!(
            verdicts.iter().filter(|v| v.is_ok()).count(),
            1,
            "{trigger:?}"
        );
        assert!(verdicts
            .iter()
            .filter_map(|v| v.as_ref().err())
            .all(|e| matches!(e, LoginError::Waiting(_))));
    }
}

#[test]
fn a_save_interrupted_before_the_rename_leaves_the_record_as_it_was() {
    let dir = Scratch::new("guard-interrupted");
    write_record(
        &dir,
        br#"{"last":777,"failures":2,"paused":"locked","credentials_paused":true,"manual_failures":0}"#,
    );
    let before = record_bytes(&dir);
    // A crash mid-write leaves a partial temp file beside the record.
    let temp = paths::sign_in_record(&dir.0)
        .with_file_name(format!(".sign-in.json.{}.tmp", std::process::id()));
    std::fs::write(&temp, b"{\"last\":9").unwrap();
    assert_eq!(record_bytes(&dir), before);

    // The next reader sees the old record, pause and all.
    let Err(LoginError::Paused(why)) = admit_as(&dir, Trigger::Startup, APP, 9_000_000) else {
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
        admit_as(&dir, Trigger::Startup, APP, 9_000_000),
        Err(LoginError::Paused(_))
    ));
}

#[test]
fn an_old_layout_record_without_a_lock_file_is_read_and_a_lock_file_appears() {
    let dir = Scratch::new("guard-old-layout");
    write_record(&dir, br#"{"last":1000,"failures":0,"paused":null}"#);
    assert!(!paths::sign_in_lock(&dir.0).exists());
    assert!(matches!(
        admit_as(&dir, Trigger::Startup, APP, 1_100),
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
    admit_as(&dir, Trigger::Startup, APP, 1_000).unwrap();
    for path in [paths::sign_in_record(&dir.0), paths::sign_in_lock(&dir.0)] {
        assert!(
            crate::platform::files::is_owner_only(&path).unwrap(),
            "{}",
            path.display()
        );
    }
}
