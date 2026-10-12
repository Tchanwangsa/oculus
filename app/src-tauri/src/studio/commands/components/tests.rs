//! The playground's commands over a scratch library: the list with two
//! levels, the schema as JSON, writes that must load, history and restore,
//! removing a subject's own version, and a frame of a starter component in
//! a saved or an unsaved theme.

use serde_json::json;
use studio_core::{Library, Theme};

use super::super::levels::Level;
use super::frame::{frame_png, FrameRequest};
use super::history::{list_in as history, restore_in};
use super::rows::{list_in, Kind};
use super::source::source_in;
use super::write::{delete_in, write_in};
use crate::studio::libraries::libraries_at;
use crate::test_support::Scratch;

const BANNER: &str = r#"export const props = { label: str() };
export const examples = { one: { label: "Hello" } };
export default ({ label }, { theme }) => (
  <Text style={{ ...theme.type.h1, color: theme.color.text }}>{label}</Text>
);
"#;

fn library(root: &Scratch) -> Library {
    libraries_at(&root.join("global"), &root.join("subjects"), false).unwrap()
}

fn request(name: &str) -> FrameRequest {
    FrameRequest {
        name: name.into(),
        subject: None,
        theme: "default".into(),
        theme_text: None,
        props: None,
        example: None,
        t: 1.0,
        width: 480,
        duration: None,
    }
}

fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
    let be = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    (be(16), be(20))
}

#[test]
fn the_list_has_the_nearest_level_per_name_and_marks_shells() {
    let root = Scratch::new("comp-list");
    let library = library(&root);
    write_in(&library, Level::Subject, Some("MAST1"), "Banner", BANNER).unwrap();
    write_in(&library, Level::Subject, Some("MAST1"), "Grid", BANNER).unwrap();
    // A library may still hold a shell from before the starters were
    // drawing primitives.
    write_in(
        &library,
        Level::Subject,
        Some("MAST1"),
        "SplitShell",
        BANNER,
    )
    .unwrap();
    std::fs::write(
        root.join("global/components/Broken.tsx"),
        "export default () => <Box",
    )
    .unwrap();

    let global = list_in(&library, None).unwrap();
    let names: Vec<_> = global.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Axes",
            "Bars",
            "Broken",
            "Code",
            "Graph",
            "Grid",
            "NumberLine",
            "Slide",
            "Table",
            "TitleCard"
        ]
    );
    assert!(global.iter().all(|r| r.level == Level::Global));

    let subject = list_in(&library, Some("MAST1")).unwrap();
    let row = |name: &str| subject.iter().find(|r| r.name == name).unwrap();
    assert_eq!(
        (row("Banner").level, row("Banner").shadows),
        (Level::Subject, false)
    );
    assert_eq!(
        (row("Grid").level, row("Grid").shadows),
        (Level::Subject, true)
    );
    assert_eq!(
        (row("TitleCard").level, row("TitleCard").shadows),
        (Level::Global, false)
    );
    assert_eq!(subject.len(), 12);

    let kind = |name: &str| subject.iter().find(|r| r.name == name).unwrap().kind;
    assert_eq!(kind("SplitShell"), Kind::Shell);
    assert_eq!(kind("Banner"), Kind::Component);

    let broken = subject.iter().find(|r| r.name == "Broken").unwrap();
    assert!(broken.problem.is_some() && broken.props.is_empty());
    assert!(subject.iter().filter(|r| r.problem.is_none()).count() == 11);

    // A starter is a starter whichever level holds its file and however it
    // was edited; a component the user made is not.
    assert!(global
        .iter()
        .filter(|r| r.name != "Broken")
        .all(|r| r.starter));
    assert!(!global.iter().find(|r| r.name == "Broken").unwrap().starter);
    assert!(row("Grid").starter && row("TitleCard").starter);
    assert!(!row("Banner").starter && !row("SplitShell").starter);
}

#[test]
fn the_schema_serialises_as_the_builders_wrote_it() {
    let root = Scratch::new("comp-schema");
    let library = library(&root);
    let rows = list_in(&library, None).unwrap();
    let card = rows.iter().find(|r| r.name == "TitleCard").unwrap();
    let json = serde_json::to_value(card).unwrap();
    assert_eq!(json["level"], "global");
    assert_eq!(json["kind"], "component");
    assert!(json["problem"].is_null());
    assert_eq!(
        json["props"][0],
        json!({ "name": "eyebrow", "kind": "str", "optional": true })
    );
    assert_eq!(
        json["props"][1],
        json!({ "name": "title", "kind": "str", "optional": false })
    );
    assert_eq!(
        json["props"][3],
        json!({ "name": "accent", "kind": "token", "optional": false,
                "token": "color", "default": "accent" })
    );
    assert_eq!(json["examples"]["long"]["title"], json!("Gradient descent"));
    let names: Vec<_> = json["examples"].as_object().unwrap().keys().collect();
    assert_eq!(names, ["short", "long"], "declaration order");

    let grid = rows.iter().find(|r| r.name == "Grid").unwrap();
    let json = serde_json::to_value(grid).unwrap();
    assert_eq!(
        json["props"][1],
        json!({ "name": "tones", "kind": "list", "optional": false, "default": [],
                "of": { "kind": "list", "optional": false,
                        "of": { "kind": "token", "optional": false, "token": "color" } } })
    );
}

#[test]
fn a_component_that_does_not_load_is_refused_and_the_file_is_untouched() {
    let root = Scratch::new("comp-refuse");
    let library = library(&root);
    let path = root.join("global/components/TitleCard.tsx");
    let before = std::fs::read_to_string(&path).unwrap();

    let e = write_in(
        &library,
        Level::Global,
        None,
        "TitleCard",
        "export default () => <Box",
    )
    .unwrap_err();
    assert!(!e.is_empty());
    let e = write_in(
        &library,
        Level::Global,
        None,
        "TitleCard",
        "export const props = {};",
    )
    .unwrap_err();
    assert!(e.contains("default"), "{e}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    assert!(!root.join("global/.history").exists());

    let e = write_in(&library, Level::Global, None, "NewOne", "nonsense (").unwrap_err();
    assert!(!e.is_empty());
    assert!(!root.join("global/components/NewOne.tsx").exists());
}

#[test]
fn names_levels_and_subjects_that_reach_outside_a_library_are_refused() {
    let root = Scratch::new("comp-names");
    let library = library(&root);
    for name in ["../Evil", "evil", "A/B", "Has.Dot", "", "A b"] {
        assert!(write_in(&library, Level::Global, None, name, BANNER)
            .unwrap_err()
            .contains("component name"));
        assert!(source_in(&library, None, name).is_err());
        assert!(history(&library, Level::Global, None, name).is_err());
    }
    assert!(write_in(&library, Level::Subject, None, "Banner", BANNER)
        .unwrap_err()
        .contains("needs a subject"));
    assert!(write_in(&library, Level::Subject, Some("../x"), "Banner", BANNER).is_err());
    assert!(list_in(&library, Some("a/b")).is_err());
    assert!(!root.join("x").exists());
}

#[test]
fn the_source_comes_from_the_nearest_level() {
    let root = Scratch::new("comp-source");
    let library = library(&root);
    let global = source_in(&library, Some("MAST1"), "Grid").unwrap();
    assert_eq!(global.level, Level::Global);
    assert!(global.text.contains("export default"));
    write_in(&library, Level::Subject, Some("MAST1"), "Grid", BANNER).unwrap();
    let own = source_in(&library, Some("MAST1"), "Grid").unwrap();
    assert_eq!((own.level, own.text.as_str()), (Level::Subject, BANNER));
    assert_eq!(source_in(&library, None, "Grid").unwrap(), global);
    let e = source_in(&library, None, "Missing").unwrap_err();
    assert!(e.contains("Missing") && e.contains("TitleCard"), "{e}");
}

#[test]
fn a_write_is_snapshotted_and_restore_can_itself_be_undone() {
    let root = Scratch::new("comp-history");
    let library = library(&root);
    let original = source_in(&library, None, "NumberLine").unwrap().text;
    assert!(history(&library, Level::Global, None, "NumberLine")
        .unwrap()
        .is_empty());

    let edited = BANNER.replace("Hello", "Edited");
    let written = write_in(&library, Level::Global, None, "NumberLine", &edited).unwrap();
    assert_eq!(
        (written.level, written.text.as_str()),
        (Level::Global, edited.as_str())
    );
    assert_eq!(
        std::fs::read_to_string(root.join("global/components/NumberLine.tsx")).unwrap(),
        edited
    );

    // The same text again writes nothing.
    write_in(&library, Level::Global, None, "NumberLine", &edited).unwrap();
    let snapshots = history(&library, Level::Global, None, "NumberLine").unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].id, snapshots[0].saved_at.to_string());
    assert_eq!(
        serde_json::to_value(&snapshots[0]).unwrap()["savedAt"],
        json!(snapshots[0].saved_at)
    );

    let restored = restore_in(
        &library,
        Level::Global,
        None,
        "NumberLine",
        &snapshots[0].id,
    )
    .unwrap();
    assert_eq!(restored.text, original);
    let now = history(&library, Level::Global, None, "NumberLine").unwrap();
    assert_eq!(now.len(), 2);
    assert!(now[0].saved_at > now[1].saved_at, "newest first");

    // Undo of the undo: the newest snapshot is the edit.
    let again = restore_in(&library, Level::Global, None, "NumberLine", &now[0].id).unwrap();
    assert_eq!(again.text, edited);

    // Other levels, other components and made-up ids have no such snapshot.
    assert!(history(&library, Level::Global, None, "Grid")
        .unwrap()
        .is_empty());
    assert!(
        history(&library, Level::Subject, Some("MAST1"), "NumberLine")
            .unwrap()
            .is_empty()
    );
    for id in ["1", "../../x", "", "latest"] {
        assert!(restore_in(&library, Level::Global, None, "NumberLine", id)
            .unwrap_err()
            .contains("no snapshot"));
    }
}

#[test]
fn a_starter_component_draws_at_the_width_asked() {
    let root = Scratch::new("comp-frame");
    let library = library(&root);
    let warm = FrameRequest {
        theme: "warm".into(),
        ..request("TitleCard")
    };
    let started = std::time::Instant::now();
    let png = frame_png(&library, request("TitleCard")).unwrap();
    let first = started.elapsed();
    assert_eq!(png_size(&png), (480, 270));
    let png_warm = frame_png(&library, warm).unwrap();
    assert_ne!(png, png_warm, "the theme changes the frame");
    println!(
        "studio_component_frame: first {first:?}, second {:?}",
        started.elapsed() - first
    );

    let grid = FrameRequest {
        width: 1920,
        t: 0.0,
        ..request("Grid")
    };
    let png = frame_png(&library, grid).unwrap();
    assert_eq!(png_size(&png), (1920, 1080));
}

#[test]
fn props_win_over_examples_and_mistakes_come_back_as_sentences() {
    let root = Scratch::new("comp-frame-props");
    let library = library(&root);
    let named = |example: &str| FrameRequest {
        example: Some(example.into()),
        ..request("TitleCard")
    };
    let short = frame_png(&library, named("short")).unwrap();
    let long = frame_png(&library, named("long")).unwrap();
    assert_ne!(short, long);
    let own = FrameRequest {
        props: json!({ "title": "Matrices" }).as_object().cloned(),
        example: Some("long".into()),
        ..request("TitleCard")
    };
    let own = frame_png(&library, own).unwrap();
    assert_ne!(own, long);
    // No props and no example is the first example.
    assert_eq!(frame_png(&library, request("TitleCard")).unwrap(), short);

    let err = |request: FrameRequest| frame_png(&library, request).unwrap_err();
    let e = err(named("nope"));
    assert!(
        e.contains("no example `nope`") && e.contains("short"),
        "{e}"
    );
    let e = err(FrameRequest {
        props: json!({}).as_object().cloned(),
        ..request("TitleCard")
    });
    assert!(e.contains("title"), "{e}");
    let e = err(FrameRequest {
        t: 4.0,
        ..request("TitleCard")
    });
    assert!(e.contains("outside"), "{e}");
    let e = err(FrameRequest {
        t: f64::NAN,
        ..request("TitleCard")
    });
    assert!(e.contains("outside"), "{e}");
    let e = err(FrameRequest {
        width: 0,
        ..request("TitleCard")
    });
    assert!(e.contains("width"), "{e}");
    let e = err(FrameRequest {
        theme: "nothing".into(),
        ..request("TitleCard")
    });
    assert!(e.contains("nothing"), "{e}");
    let e = err(FrameRequest {
        theme: "../x".into(),
        ..request("TitleCard")
    });
    assert!(e.contains("theme name"), "{e}");
    let e = err(request("Missing"));
    assert!(e.contains("Missing"), "{e}");
    let longer = FrameRequest {
        t: 5.0,
        duration: Some(8.0),
        ..request("TitleCard")
    };
    frame_png(&library, longer).unwrap();
}

#[test]
fn removing_an_own_version_falls_back_to_the_global_one_and_can_be_restored() {
    let root = Scratch::new("comp-delete");
    let library = library(&root);
    write_in(&library, Level::Subject, Some("MAST1"), "Grid", BANNER).unwrap();
    assert_eq!(
        source_in(&library, Some("MAST1"), "Grid").unwrap().level,
        Level::Subject
    );
    delete_in(&library, "MAST1", "Grid").unwrap();
    assert!(!root.join("subjects/MAST1/components/Grid.tsx").exists());
    assert_eq!(
        source_in(&library, Some("MAST1"), "Grid").unwrap().level,
        Level::Global
    );
    let snapshots = history(&library, Level::Subject, Some("MAST1"), "Grid").unwrap();
    assert_eq!(snapshots.len(), 1);
    let back = restore_in(
        &library,
        Level::Subject,
        Some("MAST1"),
        "Grid",
        &snapshots[0].id,
    )
    .unwrap();
    assert_eq!(back.text, BANNER);

    let e = delete_in(&library, "MAST1", "TitleCard").unwrap_err();
    assert!(e.contains("no "), "{e}");
    assert!(delete_in(&library, "../x", "Grid").is_err());
    assert!(delete_in(&library, "MAST1", "../Grid").is_err());
    assert!(root.join("global/components/Grid.tsx").exists());
}

#[test]
fn an_unsaved_theme_draws_in_place_of_the_saved_one() {
    let root = Scratch::new("comp-frame-draft");
    let library = library(&root);
    let mut tokens = Theme::bundled("default").unwrap().tokens;
    let saved = frame_png(&library, request("TitleCard")).unwrap();
    let same = FrameRequest {
        theme_text: Some(serde_json::to_string(&tokens).unwrap()),
        ..request("TitleCard")
    };
    assert_eq!(frame_png(&library, same).unwrap(), saved);

    tokens["color"]["ink"] = json!("#102030");
    let draft = FrameRequest {
        theme_text: Some(serde_json::to_string(&tokens).unwrap()),
        ..request("TitleCard")
    };
    assert_ne!(frame_png(&library, draft).unwrap(), saved);

    tokens["color"]["ink"] = json!("blue");
    let bad = FrameRequest {
        theme_text: Some(serde_json::to_string(&tokens).unwrap()),
        ..request("TitleCard")
    };
    let e = frame_png(&library, bad).unwrap_err();
    assert!(e.contains("blue") && e.contains("default.json"), "{e}");
    let huge = FrameRequest {
        theme_text: Some(" ".repeat(300 * 1024)),
        ..request("TitleCard")
    };
    assert!(frame_png(&library, huge).unwrap_err().contains("KB"));
}
