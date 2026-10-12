//! Seeding offers each starter file once, updates the copies the user never
//! edited and leaves the rest alone; a write snapshots the old text first.
#![allow(clippy::unwrap_used, clippy::panic)]

use super::seed::hash;
use super::shipped::SHIPPED;
use super::{LibrarySettings, STARTER_COMPONENTS, STARTER_THEMES, remove, seed, write};
use std::path::{Path, PathBuf};

/// `seed_files` with no shipped-version table.
fn seed_files<'a>(
    root: &Path,
    files: impl Iterator<Item = (String, &'a str)>,
    reset: bool,
) -> crate::error::Result<Vec<String>> {
    super::seed::seed_files(root, files, &[], reset)
}

fn seeded(root: &Path) -> String {
    std::fs::read_to_string(root.join(".seeded")).unwrap_or_default()
}

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

#[test]
fn a_starter_dropped_from_the_set_leaves_the_users_copy_alone() {
    let root = scratch("dropped");
    let old = [
        ("components/Steps.tsx".to_owned(), "starter steps"),
        ("components/TitleCard.tsx".to_owned(), "starter title"),
    ];
    seed_files(&root, old.into_iter(), false).unwrap();
    write(&root, "components/Steps.tsx", "my steps").unwrap();
    // A later build no longer ships Steps: it is neither written nor deleted,
    // edited or not, and a reset only rewrites the starters that still exist.
    let new = || [("components/TitleCard.tsx".to_owned(), "starter title")].into_iter();
    assert!(seed_files(&root, new(), false).unwrap().is_empty());
    assert!(seed_files(&root, new(), true).unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("components/Steps.tsx")).unwrap(),
        "my steps"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn remove_snapshots_then_deletes() {
    let root = scratch("remove");
    write(&root, "themes/chalk.json", "chalk text").unwrap();
    remove(&root, "themes/chalk.json").unwrap();
    assert!(!root.join("themes/chalk.json").exists());
    let snaps = history(&root);
    assert_eq!(snaps.len(), 1, "{snaps:?}");
    let kept = std::fs::read_to_string(root.join(".history").join(&snaps[0])).unwrap();
    assert_eq!(kept, "chalk text");
    let e = remove(&root, "themes/chalk.json").unwrap_err().to_string();
    assert!(e.contains("no "), "{e}");
    assert!(remove(&root, "../x.json").is_err());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn settings_round_trip_and_refuse_unknown_keys() {
    let root = scratch("settings");
    assert_eq!(
        LibrarySettings::read(&root).unwrap(),
        LibrarySettings::default()
    );
    let warm = LibrarySettings {
        default_theme: Some("warm".into()),
    };
    warm.write(&root).unwrap();
    let text = std::fs::read_to_string(root.join("library.json")).unwrap();
    assert!(text.contains("\"defaultTheme\": \"warm\""), "{text}");
    assert_eq!(LibrarySettings::read(&root).unwrap(), warm);

    LibrarySettings::default().write(&root).unwrap();
    assert!(!root.join("library.json").exists());
    LibrarySettings::default().write(&root).unwrap();

    std::fs::write(root.join("library.json"), r#"{"theme": "warm"}"#).unwrap();
    let e = LibrarySettings::read(&root).unwrap_err().to_string();
    assert!(e.contains("library.json") && e.contains("theme"), "{e}");
    std::fs::write(root.join("library.json"), r#"{"defaultTheme": "../x"}"#).unwrap();
    let e = LibrarySettings::read(&root).unwrap_err().to_string();
    assert!(e.contains("not a theme name"), "{e}");
    let bad = LibrarySettings {
        default_theme: Some("Chalk".into()),
    };
    assert!(bad.write(&root).is_err());
    std::fs::remove_dir_all(&root).ok();
}

const STEPS: &str = "components/Steps.tsx";

fn one(text: &str) -> std::vec::IntoIter<(String, &str)> {
    vec![(STEPS.to_owned(), text)].into_iter()
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap()
}

/// A `.seeded` line for `rel` as last written with `text`.
fn line(rel: &str, text: &str) -> String {
    format!("{rel}\t{}\n", hash(text.as_bytes()))
}

#[test]
fn seeding_records_the_hash_of_what_it_wrote() {
    let root = scratch("records");
    seed_files(&root, one("v1"), false).unwrap();
    assert_eq!(seeded(&root), line(STEPS, "v1"));
    // The record lands through a rename: no temporary file is left behind.
    assert!(!root.join(".seeded.tmp").exists());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn an_untouched_copy_updates_and_snapshots_the_old_text() {
    let root = scratch("update");
    seed_files(&root, one("v1"), false).unwrap();
    let written = seed_files(&root, one("v2"), false).unwrap();
    assert_eq!(written, [STEPS]);
    assert_eq!(read(&root, STEPS), "v2");
    assert_eq!(seeded(&root), line(STEPS, "v2"));
    let snapshots = history(&root);
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        std::fs::read_to_string(root.join(".history").join(&snapshots[0])).unwrap(),
        "v1"
    );
    // A third version updates it again; an unchanged one writes nothing.
    assert_eq!(seed_files(&root, one("v3"), false).unwrap(), [STEPS]);
    assert!(seed_files(&root, one("v3"), false).unwrap().is_empty());
    assert_eq!(history(&root).len(), 2);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn an_edited_copy_is_kept_and_its_hash_never_recorded() {
    let root = scratch("edited");
    seed_files(&root, one("v1"), false).unwrap();
    write(&root, STEPS, "my steps").unwrap();
    for starter in ["v2", "v3"] {
        assert!(seed_files(&root, one(starter), false).unwrap().is_empty());
        assert_eq!(read(&root, STEPS), "my steps");
        assert_eq!(seeded(&root), line(STEPS, "v1"));
    }
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_deleted_copy_stays_deleted_through_an_update() {
    let root = scratch("deleted");
    seed_files(&root, one("v1"), false).unwrap();
    std::fs::remove_file(root.join(STEPS)).unwrap();
    assert!(seed_files(&root, one("v2"), false).unwrap().is_empty());
    assert!(!root.join(STEPS).exists());
    assert_eq!(seeded(&root), line(STEPS, "v1"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_copy_equal_to_the_starter_gets_the_current_hash() {
    let root = scratch("equal");
    seed_files(&root, one("v1"), false).unwrap();
    // The user pastes the newer starter text in by hand: nothing to write,
    // and the record catches up so the next starter reaches it.
    write(&root, STEPS, "v2").unwrap();
    assert!(seed_files(&root, one("v2"), false).unwrap().is_empty());
    assert_eq!(seeded(&root), line(STEPS, "v2"));
    assert_eq!(seed_files(&root, one("v3"), false).unwrap(), [STEPS]);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_legacy_line_with_a_known_old_version_updates() {
    let root = scratch("legacy-known");
    std::fs::create_dir_all(root.join("components")).unwrap();
    std::fs::write(root.join(STEPS), "v1").unwrap();
    std::fs::write(root.join(".seeded"), format!("{STEPS}\n")).unwrap();
    let (v1, v2) = (hash(b"v1"), hash(b"v2"));
    let table: &[(&str, &[&str])] = &[(STEPS, &[&v1, &v2])];
    let written = super::seed::seed_files(&root, one("v3"), table, false).unwrap();
    assert_eq!(written, [STEPS]);
    assert_eq!(read(&root, STEPS), "v3");
    assert_eq!(seeded(&root), line(STEPS, "v3"));
    assert_eq!(history(&root).len(), 1);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_legacy_line_with_unknown_text_is_kept_and_stays_hashless() {
    let root = scratch("legacy-unknown");
    std::fs::create_dir_all(root.join("components")).unwrap();
    std::fs::write(root.join(STEPS), "my steps").unwrap();
    std::fs::write(root.join(".seeded"), format!("{STEPS}\n")).unwrap();
    let v1 = hash(b"v1");
    let table: &[(&str, &[&str])] = &[(STEPS, &[&v1])];
    for starter in ["v2", "v3"] {
        let written = super::seed::seed_files(&root, one(starter), table, false).unwrap();
        assert!(written.is_empty());
        assert_eq!(read(&root, STEPS), "my steps");
        assert_eq!(seeded(&root), format!("{STEPS}\n"));
    }
    assert!(history(&root).is_empty());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_legacy_line_with_the_current_text_gains_its_hash() {
    let root = scratch("legacy-current");
    std::fs::create_dir_all(root.join("components")).unwrap();
    std::fs::write(root.join(STEPS), "v2").unwrap();
    std::fs::write(root.join(".seeded"), format!("{STEPS}\n")).unwrap();
    assert!(seed_files(&root, one("v2"), false).unwrap().is_empty());
    assert_eq!(seeded(&root), line(STEPS, "v2"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_dropped_starter_keeps_its_file_and_its_line() {
    let root = scratch("dropped-line");
    let title = "components/TitleCard.tsx";
    std::fs::create_dir_all(root.join("components")).unwrap();
    std::fs::write(root.join(STEPS), "v1").unwrap();
    std::fs::write(root.join(title), "title").unwrap();
    // One dropped starter has a legacy line, one a hashed line; both lines
    // and files survive an update of another starter and a reset.
    let record = format!("{STEPS}\n{}", line(title, "title"));
    std::fs::write(root.join(".seeded"), &record).unwrap();
    let only = || vec![("components/Other.tsx".to_owned(), "other")].into_iter();
    assert_eq!(
        seed_files(&root, only(), false).unwrap(),
        ["components/Other.tsx"]
    );
    assert_eq!(read(&root, STEPS), "v1");
    assert_eq!(read(&root, title), "title");
    let expected = format!("{record}{}", line("components/Other.tsx", "other"));
    assert_eq!(seeded(&root), expected);
    seed_files(&root, only(), true).unwrap();
    assert_eq!(read(&root, STEPS), "v1");
    assert_eq!(seeded(&root), expected);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn reset_records_the_hashes_it_writes() {
    let root = scratch("reset-hash");
    std::fs::create_dir_all(root.join("components")).unwrap();
    std::fs::write(root.join(STEPS), "my steps").unwrap();
    std::fs::write(root.join("components/Same.tsx"), "same").unwrap();
    std::fs::write(
        root.join(".seeded"),
        format!("{STEPS}\ncomponents/Same.tsx\n"),
    )
    .unwrap();
    let files = vec![
        (STEPS.to_owned(), "v1"),
        ("components/Same.tsx".to_owned(), "same"),
        ("components/Gone.tsx".to_owned(), "gone"),
    ];
    let written = seed_files(&root, files.into_iter(), true).unwrap();
    assert_eq!(written, [STEPS, "components/Gone.tsx"]);
    assert_eq!(
        seeded(&root),
        format!(
            "{}{}{}",
            line(STEPS, "v1"),
            line("components/Same.tsx", "same"),
            line("components/Gone.tsx", "gone")
        )
    );
    assert_eq!(history(&root).len(), 1);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn the_compiled_set_seeds_once_and_then_changes_nothing() {
    let root = scratch("compiled");
    let first = seed(&root, false).unwrap();
    assert_eq!(first.len(), STARTER_COMPONENTS.len() + STARTER_THEMES.len());
    let record = seeded(&root);
    assert_eq!(record.lines().count(), first.len());
    assert!(record.lines().all(|l| l.split_once('\t').is_some()));
    assert!(seed(&root, false).unwrap().is_empty());
    assert_eq!(seeded(&root), record);
    assert!(history(&root).is_empty());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn current_starters_are_in_the_shipped_table() {
    let starters = STARTER_COMPONENTS
        .iter()
        .map(|(n, t)| (format!("components/{n}.tsx"), *t))
        .chain(
            STARTER_THEMES
                .iter()
                .map(|(n, t)| (format!("themes/{n}.json"), *t)),
        );
    for (rel, text) in starters {
        let row = SHIPPED.iter().find(|(p, _)| *p == rel);
        let hashes = row.map_or(&[][..], |(_, h)| h);
        assert!(
            hashes.contains(&hash(text.as_bytes()).as_str()),
            "{rel} is not in the table: run `bun run studio:hashes` in app/"
        );
    }
}

#[test]
fn the_axes_that_took_one_fn_counts_as_shipped() {
    // components/Axes.tsx at b7ebf20d, whose `fn: oneOf([...])` is the copy
    // already installed in libraries.
    let axes = SHIPPED.iter().find(|(p, _)| *p == "components/Axes.tsx");
    let hashes = axes.map_or(&[][..], |(_, h)| h);
    assert!(hashes.contains(&"439a3348bda59204e6216a9f65fee7e6b94d7d8eb093d0f5494b2e2b6891df9e"));
}
