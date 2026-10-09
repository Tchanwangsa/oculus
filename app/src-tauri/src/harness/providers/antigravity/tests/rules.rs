use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::test_support::Scratch;

use super::super::install::{apply, install_at};
use super::super::rules::{denied_by, is_valid_rule, rules_for, OculusCli, Rules};

/// Never `~/.gemini`: tests write only below this.
fn scratch(name: &str) -> Scratch {
    Scratch::new(&format!("agy-rules-{name}"))
}

fn rules(allow: &[&str], deny: &[&str]) -> Rules {
    Rules {
        allow: allow.iter().map(|s| s.to_string()).collect(),
        deny: deny.iter().map(|s| s.to_string()).collect(),
    }
}

fn read(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn a_missing_file_is_created_with_only_permissions() {
    let d = scratch("fresh");
    let path = d.join("nested/settings.json");
    let owned = apply(
        &path,
        &Rules::default(),
        &rules(&["command(ls)"], &["command(sqlite3)"]),
    )
    .unwrap();
    assert_eq!(owned, rules(&["command(ls)"], &["command(sqlite3)"]));
    assert_eq!(
        read(&path),
        serde_json::json!({"permissions": {"allow": ["command(ls)"], "deny": ["command(sqlite3)"]}})
    );
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("\n  \"permissions\""));
}

#[test]
fn the_students_keys_and_rules_are_kept() {
    let d = scratch("merge");
    let path = d.join("settings.json");
    std::fs::write(
        &path,
        r#"{
  "trustedWorkspaces": ["/w"],
  "colorScheme": "light",
  "permissions": { "ask": ["command(git)"], "allow": ["command(make)", "command(ls)"] }
}"#,
    )
    .unwrap();
    let owned = apply(
        &path,
        &Rules::default(),
        &rules(&["command(ls)", "command(oculus)"], &["command(sqlite3)"]),
    )
    .unwrap();
    // `command(ls)` was the student's first, so it stays theirs.
    assert_eq!(owned.allow, ["command(oculus)"]);
    let text = std::fs::read_to_string(&path).unwrap();
    let t = text.find("trustedWorkspaces").unwrap();
    let c = text.find("colorScheme").unwrap();
    let p = text.find("permissions").unwrap();
    assert!(t < c && c < p, "key order kept: {text}");
    let v = read(&path);
    assert_eq!(v["colorScheme"], "light");
    assert_eq!(v["trustedWorkspaces"], serde_json::json!(["/w"]));
    assert_eq!(v["permissions"]["ask"], serde_json::json!(["command(git)"]));
    assert_eq!(
        v["permissions"]["allow"],
        serde_json::json!(["command(make)", "command(ls)", "command(oculus)"])
    );
    assert_eq!(
        v["permissions"]["deny"],
        serde_json::json!(["command(sqlite3)"])
    );
}

#[test]
fn a_stale_entry_of_ours_is_taken_out_and_theirs_is_not() {
    let d = scratch("stale");
    let path = d.join("settings.json");
    std::fs::write(
            &path,
            r#"{"permissions":{"allow":["write_file(/old/lib/oculus.db)","command(make)","command(oculus)"]}}"#,
        )
        .unwrap();
    let last = rules(&["write_file(/old/lib/oculus.db)", "command(oculus)"], &[]);
    let owned = apply(
        &path,
        &last,
        &rules(&["write_file(/new/lib/oculus.db)", "command(oculus)"], &[]),
    )
    .unwrap();
    assert_eq!(
        read(&path)["permissions"]["allow"],
        serde_json::json!([
            "command(make)",
            "command(oculus)",
            "write_file(/new/lib/oculus.db)"
        ])
    );
    assert_eq!(
        owned.allow,
        ["write_file(/new/lib/oculus.db)", "command(oculus)"]
    );
}

#[test]
fn invalid_json_is_refused_and_left_alone() {
    let d = scratch("invalid");
    let path = d.join("settings.json");
    std::fs::write(&path, "{ not json").unwrap();
    let err = apply(&path, &Rules::default(), &rules(&["command(ls)"], &[])).unwrap_err();
    assert!(err.contains("not valid JSON"), "{err}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
    std::fs::write(&path, r#"{"permissions":{"allow":"command(ls)"}}"#).unwrap();
    assert!(apply(&path, &Rules::default(), &rules(&["command(ls)"], &[])).is_err());
}

#[test]
fn a_second_identical_write_changes_nothing() {
    let d = scratch("idempotent");
    let path = d.join("settings.json");
    std::fs::write(&path, r#"{"colorScheme":"dark"}"#).unwrap();
    let want = rules(&["command(oculus)"], &["command(sqlite3)"]);
    let owned = apply(&path, &Rules::default(), &want).unwrap();
    let first = std::fs::read_to_string(&path).unwrap();
    let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let again = apply(&path, &owned, &want).unwrap();
    assert_eq!(again, owned);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        mtime,
        "not rewritten"
    );
}

#[test]
fn approvals_come_and_go_through_the_record() {
    let d = scratch("install");
    let (settings, state, lib) = (d.join("settings.json"), d.join("state.json"), d.join("lib"));
    install_at(
        &settings,
        &state,
        &lib,
        None,
        Some(vec!["command(python3)".into()]),
    )
    .unwrap();
    let allow = |p: &Path| read(p)["permissions"]["allow"].clone();
    assert!(allow(&settings)
        .as_array()
        .unwrap()
        .contains(&"command(python3)".into()));
    install_at(&settings, &state, &lib, None, None).unwrap();
    assert!(allow(&settings)
        .as_array()
        .unwrap()
        .contains(&"command(python3)".into()));
    install_at(&settings, &state, &lib, None, Some(vec![])).unwrap();
    assert!(!allow(&settings)
        .as_array()
        .unwrap()
        .contains(&"command(python3)".into()));
}

#[test]
fn the_rule_set_mirrors_claudes() {
    let lib = Path::new("/lib");
    let cli = OculusCli {
        bin: PathBuf::from("/repo/target/release/oculus"),
        launcher: Some(PathBuf::from("/home/u/.local/bin/oculus")),
    };
    let r = rules_for(lib, Some(&cli), &["command(python3)".into()]);
    for want in [
        "write_file(/lib/oculus.db)",
        "write_file(/lib/oculus.db-wal)",
        "write_file(/lib/oculus.db-shm)",
        "read_file(/repo/target/release)",
        "read_file(/home/u/.local/bin)",
        "command(oculus)",
        "command(/repo/target/release/oculus)",
        "command(/home/u/.local/bin/oculus)",
        "command(ls)",
        "command(python3)",
    ] {
        assert!(
            r.allow.iter().any(|a| a == want),
            "allow {want}: {:?}",
            r.allow
        );
    }
    assert!(!r
        .allow
        .iter()
        .any(|a| a == "write_file(/lib)" || a == "write_file(/lib/agents)"));
    assert!(!r
        .allow
        .iter()
        .any(|a| a == "command(find)" || a == "command(rg)"));
    assert!(!r.deny.iter().any(|d| d == "command(sqlite3)"));
    for want in [
        "write_file(/lib/courses)",
        "write_file(/lib/agents/skills)",
        "write_file(/lib/canvas-session.cookie)",
        "write_file(/lib/antigravity-rules.json)",
        "command(sqlite3 /lib/oculus.db)",
    ] {
        assert!(
            r.deny.iter().any(|a| a == want),
            "deny {want}: {:?}",
            r.deny
        );
    }
}

#[test]
fn sqlite3_is_denied_on_both_spellings_of_the_database() {
    let lib = scratch("spellings");
    let real = lib.canonicalize().unwrap();
    let r = rules_for(&lib, None, &[]);
    let want = |l: &Path| format!("command(sqlite3 {})", l.join("oculus.db").display());
    assert!(r.deny.contains(&want(&lib)), "{:?}", r.deny);
    assert!(r.deny.contains(&want(&real)), "{:?}", r.deny);
}

#[test]
fn a_database_path_with_a_space_is_denied_however_it_is_quoted() {
    let lib = Path::new("/Users/s/Library/Application Support/com.tchan.oculus");
    let deny = rules_for(lib, None, &[]).deny;
    for want in [
        "command(sqlite3 /Users/s/Library/Application Support/com.tchan.oculus/oculus.db)",
        "command(sqlite3 \"/Users/s/Library/Application Support/com.tchan.oculus/oculus.db\")",
        "command(sqlite3 '/Users/s/Library/Application Support/com.tchan.oculus/oculus.db')",
        "command(sqlite3 /Users/s/Library/Application\\ Support/com.tchan.oculus/oculus.db)",
    ] {
        assert!(deny.iter().any(|d| d == want), "{want}: {deny:?}");
    }
}

#[test]
fn rules_are_validated_and_denies_win() {
    for ok in [
        "command(python3)",
        "write_file(/a/b)",
        "read_file(/a)",
        "read_url(example.com)",
    ] {
        assert!(is_valid_rule(ok), "{ok}");
    }
    for bad in [
        "command()",
        "mcp(x/y)",
        "Bash(ls)",
        "command(ls)\ncommand(rm)",
        "command(ls) ",
    ] {
        assert!(!is_valid_rule(bad), "{bad:?}");
    }
    let lib = Path::new("/lib");
    assert!(denied_by(lib, "write_file(/lib/courses/COMP30026)").is_some());
    assert!(denied_by(lib, "command(sqlite3 /lib/oculus.db)").is_some());
    assert!(denied_by(lib, "command(sqlite3 /lib/oculus.db .dump)").is_some());
    assert!(denied_by(lib, "command(sqlite3)").is_none());
    assert!(denied_by(lib, "write_file(/lib/agents/notes)").is_none());
    assert!(denied_by(lib, "command(python3)").is_none());
}
