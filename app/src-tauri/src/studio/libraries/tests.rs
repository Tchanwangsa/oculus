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
    let subjects = root.join("subjects");
    libraries_at(&global, &subjects, false).unwrap();
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
    libraries_at(&global, &subjects, false).unwrap();
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
    let subjects = root.join("subjects");
    libraries_at(&global, &subjects, false).unwrap();
    if let Some((name, text)) = STARTER_COMPONENTS.first() {
        let path = global.join("components").join(format!("{name}.tsx"));
        std::fs::write(&path, "my edit").unwrap();
        libraries_at(&global, &subjects, true).unwrap();
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

mod promote {
    use super::super::{libraries_at, promote_component, promote_theme, Target};
    use crate::test_support::Scratch;
    use std::path::Path;
    use studio_core::{Library, Project, STARTER_THEMES};

    fn tsx(label: &str) -> String {
        format!("export const props = {{}};\nexport default () => <Text style={{{{ fontSize: 20 }}}}>{label}</Text>;\n")
    }

    /// A video of subject `SUB` with its own `Card` and `DotGrid`, over
    /// libraries under the same root.
    fn fixture(name: &str, subject: Option<&str>) -> (Scratch, std::path::PathBuf, Library) {
        let root = Scratch::new(name);
        let video = root.join("video");
        std::fs::create_dir_all(video.join("components")).unwrap();
        let mut json = serde_json::json!({
            "size": { "width": 64, "height": 36 }, "theme": "default",
            "scenes": [{ "component": "Card", "hold": 1 }]
        });
        if let Some(s) = subject {
            json["subject"] = serde_json::json!(s);
        }
        std::fs::write(video.join("video.json"), json.to_string()).unwrap();
        std::fs::write(video.join("components/Card.tsx"), tsx("card one")).unwrap();
        std::fs::write(video.join("components/DotGrid.tsx"), tsx("my dots")).unwrap();
        let library = libraries_at(&root.join("library"), &root.join("subjects"), false).unwrap();
        (root, video, library)
    }

    fn history(dir: &Path) -> Vec<String> {
        let mut texts: Vec<String> = std::fs::read_dir(dir.join(".history"))
            .map(|rd| {
                rd.flatten()
                    .map(|e| std::fs::read_to_string(e.path()).unwrap())
                    .collect()
            })
            .unwrap_or_default();
        texts.sort();
        texts
    }

    #[test]
    fn a_promoted_component_keeps_the_old_library_text_in_history() {
        let (root, video, library) = fixture("promote-component", Some("SUB"));
        let sub = root.join("subjects/SUB");
        let first = promote_component(&video, "Card", Target::Subject, &library).unwrap();
        assert_eq!(first.path, sub.join("components/Card.tsx"));
        assert_eq!(first.level, "subject SUB");
        assert!(first.changed);
        assert!(history(&sub).is_empty(), "a new file has nothing to keep");

        let again = promote_component(&video, "Card", Target::Subject, &library).unwrap();
        assert!(!again.changed, "the same text is not written again");
        assert!(history(&sub).is_empty());

        std::fs::write(video.join("components/Card.tsx"), tsx("card two")).unwrap();
        promote_component(&video, "Card", Target::Subject, &library).unwrap();
        assert_eq!(
            std::fs::read_to_string(sub.join("components/Card.tsx")).unwrap(),
            tsx("card two")
        );
        assert_eq!(history(&sub), [tsx("card one")]);
        assert!(
            video.join("components/Card.tsx").is_file(),
            "the video keeps its copy"
        );

        // Over a starter file in the global library, the starter text is kept.
        let global = root.join("library");
        let starter = std::fs::read_to_string(global.join("components/DotGrid.tsx")).unwrap();
        let p = promote_component(&video, "DotGrid", Target::Global, &library).unwrap();
        assert_eq!(p.level, "global");
        assert_eq!(history(&global), [starter]);
    }

    #[test]
    fn promote_refuses_a_library_component_and_a_video_without_a_subject() {
        let (_root, video, library) = fixture("promote-refuse", None);
        let err = promote_component(&video, "TitleCard", Target::Global, &library).unwrap_err();
        assert!(err.contains("global library"), "{err}");
        let err = promote_component(&video, "Nope", Target::Global, &library).unwrap_err();
        assert!(err.contains("no component `Nope`"), "{err}");
        let err = promote_component(&video, "Card", Target::Subject, &library).unwrap_err();
        assert!(err.contains("names no subject"), "{err}");
    }

    #[test]
    fn a_promoted_theme_is_named_by_its_stem() {
        let (root, video, library) = fixture("promote-theme", None);
        let (_, warm) = STARTER_THEMES[0];
        std::fs::create_dir_all(video.join("themes")).unwrap();
        std::fs::write(video.join("themes/chalk.json"), warm).unwrap();
        let p = promote_theme(
            &video,
            Path::new("themes/chalk.json"),
            Target::Global,
            &library,
        )
        .unwrap();
        assert_eq!(p.path, root.join("library/themes/chalk.json"));
        let json = std::fs::read_to_string(video.join("video.json"))
            .unwrap()
            .replace("\"default\"", "\"chalk\"");
        std::fs::write(video.join("video.json"), json).unwrap();
        std::fs::remove_dir_all(video.join("themes")).unwrap();
        Project::load_with(&video, &library).expect("the name resolves in the library");

        for bad in [
            "../chalk.json",
            "/tmp/chalk.json",
            "themes/chalk.txt",
            "themes/Chalk.json",
        ] {
            assert!(
                promote_theme(&video, Path::new(bad), Target::Global, &library).is_err(),
                "{bad}"
            );
        }
        std::fs::write(video.join("broken.json"), "{").unwrap();
        assert!(promote_theme(&video, Path::new("broken.json"), Target::Global, &library).is_err());
    }
}
