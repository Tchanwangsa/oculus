//! The theme commands over a scratch library with two levels: the list,
//! the cascade, a theme's text, writes that must parse, history, restore,
//! removal and the libraries' default themes.

use serde_json::json;
use studio_core::{Library, Theme};

use super::super::levels::Level;
use super::defaults::{defaults_in, set_in};
use super::history::{list_in as history, restore_in};
use super::resolve::{resolve, ThemeLevel};
use super::rows::list_in;
use super::source::source_in;
use super::write::{delete_in, write_in};
use crate::studio::libraries::libraries_at;
use crate::test_support::Scratch;

/// A library whose global level has the starter set and `chalk`, and whose
/// subject `MAST1` has its own `chalk` and an unparseable `broken`.
fn library(root: &Scratch) -> Library {
    let library = libraries_at(&root.join("global"), &root.join("subjects"), false).unwrap();
    let theme = |bg: &str| {
        let mut tokens = Theme::bundled("default").unwrap().tokens;
        tokens["color"]["bg"] = json!(bg);
        serde_json::to_string_pretty(&tokens).unwrap()
    };
    let write = |dir: std::path::PathBuf, name: &str, text: String| {
        std::fs::create_dir_all(dir.join("themes")).unwrap();
        std::fs::write(dir.join("themes").join(name), text).unwrap();
    };
    write(root.join("global"), "chalk.json", theme("#111111"));
    write(root.join("subjects/MAST1"), "chalk.json", theme("#222222"));
    write(
        root.join("subjects/MAST1"),
        "broken.json",
        "{ \"color\": {} }".into(),
    );
    library
}

#[test]
fn one_row_per_name_from_the_nearest_level() {
    let root = Scratch::new("themes-list");
    let library = library(&root);

    let global = list_in(&library, None).unwrap();
    let names: Vec<_> = global.iter().map(|r| (r.name.as_str(), r.level)).collect();
    assert_eq!(
        names,
        [
            ("default", ThemeLevel::Bundled),
            ("chalk", ThemeLevel::Global),
            ("warm", ThemeLevel::Global),
        ]
    );
    let default = &global[0];
    assert_eq!(default.swatch.len(), 4);
    assert_eq!(default.swatch[0], "#f4f4f6");
    assert!(default.tokens.as_ref().unwrap()["color"]["accent"].is_string());
    assert_eq!(global[1].swatch[0], "#111111");

    let subject = list_in(&library, Some("MAST1")).unwrap();
    let chalk = subject.iter().find(|r| r.name == "chalk").unwrap();
    assert_eq!(
        (chalk.level, chalk.swatch[0].as_str()),
        (ThemeLevel::Subject, "#222222")
    );
    assert_eq!(chalk.shadows, Some(ThemeLevel::Global));
    let broken = subject.iter().find(|r| r.name == "broken").unwrap();
    assert_eq!(broken.shadows, None);
    let warm = subject.iter().find(|r| r.name == "warm").unwrap();
    assert_eq!((warm.level, warm.shadows), (ThemeLevel::Global, None));
    assert_eq!(global[0].shadows, None);
    assert_eq!(subject.len(), 4, "default, broken, chalk, warm");
}

#[test]
fn an_unparseable_theme_is_a_row_with_a_problem() {
    let root = Scratch::new("themes-broken");
    let library = library(&root);
    let rows = list_in(&library, Some("MAST1")).unwrap();
    let broken = rows.iter().find(|r| r.name == "broken").unwrap();
    assert_eq!(broken.level, ThemeLevel::Subject);
    assert!(broken.tokens.is_none() && broken.swatch.is_empty());
    assert!(broken.problem.as_deref().unwrap().contains("broken.json"));
    assert!(rows.iter().filter(|r| r.problem.is_none()).count() >= 3);
    let json = serde_json::to_value(broken).unwrap();
    assert_eq!(json["level"], "subject");
    assert!(json["tokens"].is_null());
}

#[test]
fn resolve_follows_the_cascade_and_refuses_paths() {
    let root = Scratch::new("themes-resolve");
    let library = library(&root);
    let bg = |subject: Option<&str>, name: &str| {
        resolve(&library, subject, name).unwrap().tokens["color"]["bg"].clone()
    };
    assert_eq!(bg(None, "chalk"), json!("#111111"));
    assert_eq!(bg(Some("MAST1"), "chalk"), json!("#222222"));
    assert_eq!(bg(Some("MAST1"), "default"), json!("#f4f4f6"));
    for bad in ["../x", "Chalk", "a/b", ""] {
        assert!(resolve(&library, None, bad)
            .unwrap_err()
            .contains("theme name"));
    }
    assert!(resolve(&library, Some("../x"), "default").is_err());
    assert!(resolve(&library, None, "nothing")
        .unwrap_err()
        .contains("bundled"));
}

/// A valid theme's text with `bg` as its canvas.
fn theme_text(bg: &str) -> String {
    let mut tokens = Theme::bundled("default").unwrap().tokens;
    tokens["color"]["bg"] = json!(bg);
    serde_json::to_string_pretty(&tokens).unwrap()
}

#[test]
fn the_source_is_the_winning_file_or_the_bundled_text() {
    let root = Scratch::new("themes-source");
    let library = library(&root);
    let bundled = source_in(&library, None, "default").unwrap();
    assert_eq!(bundled.level, ThemeLevel::Bundled);
    assert_eq!(bundled.text, Theme::bundled_text("default").unwrap());
    let own = source_in(&library, Some("MAST1"), "chalk").unwrap();
    assert_eq!(own.level, ThemeLevel::Subject);
    assert!(own.text.contains("#222222"));
    assert!(source_in(&library, None, "chalk")
        .unwrap()
        .text
        .contains("#111111"));
    assert!(source_in(&library, None, "nothing").is_err());
    assert!(source_in(&library, None, "../x")
        .unwrap_err()
        .contains("theme name"));
    let json = serde_json::to_value(&own).unwrap();
    assert_eq!(json["level"], "subject");
}

#[test]
fn a_write_must_parse_is_snapshotted_and_can_be_undone() {
    let root = Scratch::new("themes-write");
    let library = library(&root);
    let path = root.join("global/themes/chalk.json");
    let before = std::fs::read_to_string(&path).unwrap();

    let e = write_in(&library, Level::Global, None, "chalk", "{ \"color\": {} }").unwrap_err();
    assert!(e.contains("chalk.json"), "{e}");
    let mut bad: serde_json::Value = serde_json::from_str(&theme_text("#000000")).unwrap();
    bad["font"]["body"] = json!("Comic Sans");
    let e = write_in(&library, Level::Global, None, "chalk", &bad.to_string()).unwrap_err();
    assert!(e.contains("Comic Sans"), "{e}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    assert!(history(&library, Level::Global, None, "chalk")
        .unwrap()
        .is_empty());

    let edited = theme_text("#333333");
    let written = write_in(&library, Level::Global, None, "chalk", &edited).unwrap();
    assert_eq!(written.level, ThemeLevel::Global);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), edited);
    write_in(&library, Level::Global, None, "chalk", &edited).unwrap();
    let snapshots = history(&library, Level::Global, None, "chalk").unwrap();
    assert_eq!(snapshots.len(), 1, "the same text again writes nothing");

    let restored = restore_in(&library, Level::Global, None, "chalk", &snapshots[0].id).unwrap();
    assert_eq!(restored.text, before);
    let now = history(&library, Level::Global, None, "chalk").unwrap();
    assert_eq!(now.len(), 2);
    assert!(restore_in(&library, Level::Global, None, "chalk", "1")
        .unwrap_err()
        .contains("no snapshot"));
    // A component's snapshots are not a theme's.
    std::fs::write(root.join("global/.history/chalk.5.tsx"), "x").unwrap();
    assert_eq!(
        history(&library, Level::Global, None, "chalk")
            .unwrap()
            .len(),
        2
    );

    // A new subject theme, and a subject copy of a global one.
    write_in(&library, Level::Subject, Some("MAST2"), "chalk", &edited).unwrap();
    assert!(root.join("subjects/MAST2/themes/chalk.json").exists());
}

#[test]
fn bundled_names_and_paths_are_refused() {
    let root = Scratch::new("themes-refuse");
    let library = library(&root);
    let text = theme_text("#000000");
    let e = write_in(&library, Level::Global, None, "default", &text).unwrap_err();
    assert!(e.contains("bundled"), "{e}");
    assert!(!root.join("global/themes/default.json").exists());
    for bad in ["../x", "Chalk", "a/b", "", "x.json"] {
        assert!(write_in(&library, Level::Global, None, bad, &text)
            .unwrap_err()
            .contains("theme name"));
        assert!(history(&library, Level::Global, None, bad).is_err());
    }
    assert!(write_in(&library, Level::Subject, None, "chalk", &text)
        .unwrap_err()
        .contains("needs a subject"));
    assert!(delete_in(&library, Level::Global, None, "default").is_err());
    let huge = format!("{text}{}", " ".repeat(300 * 1024));
    assert!(write_in(&library, Level::Global, None, "big", &huge)
        .unwrap_err()
        .contains("KB"));
}

#[test]
fn removing_a_theme_snapshots_it_and_the_cascade_falls_back() {
    let root = Scratch::new("themes-delete");
    let library = library(&root);
    delete_in(&library, Level::Subject, Some("MAST1"), "chalk").unwrap();
    assert_eq!(
        source_in(&library, Some("MAST1"), "chalk").unwrap().level,
        ThemeLevel::Global
    );
    let snapshots = history(&library, Level::Subject, Some("MAST1"), "chalk").unwrap();
    assert_eq!(snapshots.len(), 1);
    restore_in(
        &library,
        Level::Subject,
        Some("MAST1"),
        "chalk",
        &snapshots[0].id,
    )
    .unwrap();
    assert_eq!(
        source_in(&library, Some("MAST1"), "chalk").unwrap().level,
        ThemeLevel::Subject
    );
    assert!(delete_in(&library, Level::Global, None, "nothing").is_err());
}

#[test]
fn defaults_cascade_from_the_subject_to_the_global_library_to_default() {
    let root = Scratch::new("themes-defaults");
    let library = library(&root);
    let effective = |subject: Option<&str>| defaults_in(&library, subject).unwrap().effective;
    assert_eq!(effective(None), "default");
    assert_eq!(effective(Some("MAST1")), "default");

    let set = set_in(&library, Level::Global, None, Some("warm")).unwrap();
    assert_eq!(
        (set.global.as_deref(), set.effective.as_str()),
        (Some("warm"), "warm")
    );
    assert!(std::fs::read_to_string(root.join("global/library.json"))
        .unwrap()
        .contains("\"defaultTheme\": \"warm\""));
    assert_eq!(effective(Some("MAST1")), "warm");

    let subject = set_in(&library, Level::Subject, Some("MAST1"), Some("chalk")).unwrap();
    assert_eq!(subject.subject.as_deref(), Some("chalk"));
    assert_eq!(subject.global.as_deref(), Some("warm"));
    assert_eq!(subject.effective, "chalk");
    assert_eq!(effective(None), "warm");
    assert_eq!(effective(Some("MAST2")), "warm");

    // A default the libraries no longer have is passed over.
    std::fs::remove_file(root.join("subjects/MAST1/themes/chalk.json")).unwrap();
    assert_eq!(
        effective(Some("MAST1")),
        "chalk",
        "the global chalk still exists"
    );
    std::fs::remove_file(root.join("global/themes/chalk.json")).unwrap();
    assert_eq!(effective(Some("MAST1")), "warm");

    // Only a theme in the level's reach can be its default.
    let e = set_in(&library, Level::Global, None, Some("broken")).unwrap_err();
    assert!(
        e.contains("broken"),
        "a subject's theme is out of the global reach: {e}"
    );
    set_in(&library, Level::Subject, Some("MAST1"), Some("broken")).unwrap();
    assert!(set_in(&library, Level::Global, None, Some("../x")).is_err());

    let cleared = set_in(&library, Level::Global, None, None).unwrap();
    assert_eq!(cleared.global, None);
    assert!(!root.join("global/library.json").exists());

    std::fs::write(root.join("global/library.json"), "{\"theme\": 1}").unwrap();
    let e = defaults_in(&library, None).unwrap_err();
    assert!(e.contains("library.json"), "{e}");
}
