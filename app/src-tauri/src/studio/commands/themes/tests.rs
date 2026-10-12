//! The theme list over a scratch library with two levels.

use serde_json::json;
use studio_core::{Library, Theme};

use super::resolve::{resolve, ThemeLevel};
use super::rows::list_in;
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
