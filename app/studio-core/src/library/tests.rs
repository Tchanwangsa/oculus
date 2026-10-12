//! Seeding offers each starter file once and never overwrites; a write
//! snapshots the old text first.
#![allow(clippy::unwrap_used, clippy::panic)]

use super::{seed_files, write};
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("studio-library-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn history(root: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root.join(".history"))
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn write_snapshots_the_previous_text() {
    let root = scratch("write");
    write(&root, "components/Steps.tsx", "old text").unwrap();
    assert!(
        history(&root).is_empty(),
        "a new file has nothing to snapshot"
    );
    write(&root, "components/Steps.tsx", "new text").unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("components/Steps.tsx")).unwrap(),
        "new text"
    );
    let snaps = history(&root);
    assert_eq!(snaps.len(), 1, "{snaps:?}");
    assert!(
        snaps[0].starts_with("Steps.") && snaps[0].ends_with(".tsx"),
        "{snaps:?}"
    );
    let kept = std::fs::read_to_string(root.join(".history").join(&snaps[0])).unwrap();
    assert_eq!(kept, "old text");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn two_writes_in_one_millisecond_keep_both_snapshots() {
    let root = scratch("burst");
    for text in ["a", "b", "c", "d"] {
        write(&root, "themes/warm.json", text).unwrap();
    }
    let snaps = history(&root);
    assert_eq!(snaps.len(), 3, "{snaps:?}");
    let mut texts: Vec<String> = snaps
        .iter()
        .map(|s| std::fs::read_to_string(root.join(".history").join(s)).unwrap())
        .collect();
    texts.sort();
    assert_eq!(texts, ["a", "b", "c"]);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn write_refuses_paths_outside_the_library_folders() {
    let root = scratch("paths");
    for bad in [
        "../x.tsx",
        "components/../x.tsx",
        "elsewhere/x.tsx",
        "components/",
        "components/a/b.tsx",
    ] {
        assert!(write(&root, bad, "x").is_err(), "{bad} should be refused");
    }
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn seeding_copies_missing_files_and_never_overwrites() {
    let root = scratch("seed");
    let files = || {
        [
            ("components/Steps.tsx".to_owned(), "starter steps"),
            ("themes/warm.json".to_owned(), "starter warm"),
        ]
        .into_iter()
    };
    let written = seed_files(&root, files(), false).unwrap();
    assert_eq!(written, ["components/Steps.tsx", "themes/warm.json"]);
    // The user edits one; a later seed leaves it alone and writes nothing.
    write(&root, "components/Steps.tsx", "my steps").unwrap();
    assert!(seed_files(&root, files(), false).unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("components/Steps.tsx")).unwrap(),
        "my steps"
    );
    // Reset writes the starter text over it, keeping the edit in .history.
    let reset = seed_files(&root, files(), true).unwrap();
    assert_eq!(reset, ["components/Steps.tsx"]);
    assert_eq!(
        std::fs::read_to_string(root.join("components/Steps.tsx")).unwrap(),
        "starter steps"
    );
    // Two snapshots: the starter text (before the edit) and the edit.
    let mut texts: Vec<String> = history(&root)
        .iter()
        .map(|s| std::fs::read_to_string(root.join(".history").join(s)).unwrap())
        .collect();
    texts.sort();
    assert_eq!(texts, ["my steps", "starter steps"]);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_deleted_starter_stays_deleted_and_a_new_one_still_arrives() {
    let root = scratch("seeded");
    let one = [("components/Steps.tsx".to_owned(), "starter steps")];
    seed_files(&root, one.clone().into_iter(), false).unwrap();
    std::fs::remove_file(root.join("components/Steps.tsx")).unwrap();
    assert!(
        seed_files(&root, one.clone().into_iter(), false)
            .unwrap()
            .is_empty()
    );
    assert!(!root.join("components/Steps.tsx").exists());
    // A later build adds a starter: only that one is copied.
    let two = [
        ("components/Steps.tsx".to_owned(), "starter steps"),
        ("components/BigNumber.tsx".to_owned(), "starter number"),
    ];
    let written = seed_files(&root, two.clone().into_iter(), false).unwrap();
    assert_eq!(written, ["components/BigNumber.tsx"]);
    assert!(!root.join("components/Steps.tsx").exists());
    // Reset brings the deleted one back.
    let reset = seed_files(&root, two.into_iter(), true).unwrap();
    assert_eq!(reset, ["components/Steps.tsx"]);
    std::fs::remove_dir_all(&root).ok();
}
