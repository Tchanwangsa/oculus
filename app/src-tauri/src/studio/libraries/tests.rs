//! Seeding copies the starter set into a global library and never replaces
//! an edit; a reset keeps the edit in `.history`.
#![allow(clippy::unwrap_used, clippy::panic)]

use super::libraries_at;
use studio_core::{STARTER_COMPONENTS, STARTER_THEMES};

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("studio-libraries-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn seeding_never_overwrites_an_edit() {
    let root = scratch("seed");
    let global = root.join("library");
    let courses = root.join("courses");
    libraries_at(&global, &courses, false).unwrap();
    for (name, text) in STARTER_COMPONENTS {
        assert_eq!(
            std::fs::read_to_string(global.join("components").join(format!("{name}.tsx"))).unwrap(),
            *text
        );
    }
    // The user edits every starter file; a later load leaves each one alone.
    for (name, _) in STARTER_COMPONENTS {
        std::fs::write(
            global.join("components").join(format!("{name}.tsx")),
            "my edit",
        )
        .unwrap();
    }
    for (name, _) in STARTER_THEMES {
        std::fs::write(
            global.join("themes").join(format!("{name}.json")),
            "my edit",
        )
        .unwrap();
    }
    libraries_at(&global, &courses, false).unwrap();
    for (name, _) in STARTER_COMPONENTS {
        let text = std::fs::read_to_string(global.join("components").join(format!("{name}.tsx")));
        assert_eq!(text.unwrap(), "my edit", "{name}");
    }
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_reset_writes_the_starter_text_and_keeps_the_edit() {
    let root = scratch("reset");
    let global = root.join("library");
    let courses = root.join("courses");
    libraries_at(&global, &courses, false).unwrap();
    if let Some((name, text)) = STARTER_COMPONENTS.first() {
        let path = global.join("components").join(format!("{name}.tsx"));
        std::fs::write(&path, "my edit").unwrap();
        libraries_at(&global, &courses, true).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), *text);
        let kept: Vec<String> = std::fs::read_dir(global.join(".history"))
            .unwrap()
            .flatten()
            .map(|e| std::fs::read_to_string(e.path()).unwrap())
            .collect();
        assert_eq!(kept, ["my edit"], "{kept:?}");
    }
    std::fs::remove_dir_all(&root).ok();
}
