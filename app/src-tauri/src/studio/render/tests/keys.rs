use super::super::key::{segment_key, tree_hash, Trees};
use super::super::preset::Preset;
use super::super::settings::Settings;
use crate::test_support::Scratch;

fn settings(p: Preset) -> Settings {
    super::settings(p, (640, 360), 30)
}

#[test]
fn a_segment_key_changes_with_each_input() {
    let trees = Trees {
        components: "c".into(),
        assets: "a".into(),
    };
    let base = segment_key(b"scene", &trees, &settings(Preset::Final));
    assert_eq!(
        base,
        segment_key(b"scene", &trees, &settings(Preset::Final))
    );
    let others = [
        segment_key(b"scene!", &trees, &settings(Preset::Final)),
        segment_key(b"scene", &trees, &settings(Preset::Draft)),
        segment_key(
            b"scene",
            &Trees {
                components: "c2".into(),
                ..trees.clone()
            },
            &settings(Preset::Final),
        ),
        segment_key(
            b"scene",
            &Trees {
                assets: "a2".into(),
                ..trees.clone()
            },
            &settings(Preset::Final),
        ),
        segment_key(
            b"scene",
            &trees,
            &super::settings(Preset::Final, (640, 360), 60),
        ),
    ];
    for other in others {
        assert_ne!(other, base);
    }
}

#[test]
fn a_tree_hash_follows_names_and_bytes_and_skips_ds_store() {
    let dir = Scratch::new("render-tree");
    let empty = tree_hash(&dir.join("missing")).unwrap();
    assert_eq!(empty, tree_hash(&dir).unwrap());

    std::fs::create_dir_all(dir.join("img")).unwrap();
    std::fs::write(dir.join("img/a.png"), b"one").unwrap();
    let one = tree_hash(&dir).unwrap();
    assert_ne!(one, empty);

    std::fs::write(dir.join(".DS_Store"), b"finder").unwrap();
    assert_eq!(tree_hash(&dir).unwrap(), one);

    std::fs::write(dir.join("img/a.png"), b"two").unwrap();
    let two = tree_hash(&dir).unwrap();
    assert_ne!(two, one);

    std::fs::rename(dir.join("img/a.png"), dir.join("img/b.png")).unwrap();
    assert_ne!(tree_hash(&dir).unwrap(), two);
}

/// A video of one scene using `Card`, with `Card` in the project folder, the
/// subject's library or the global one; the trees of its render.
#[test]
fn a_library_component_edit_changes_the_trees_of_the_scenes_that_use_it() {
    use studio_core::{Library, Project};
    let root = Scratch::new("render-keys-library");
    let video = root.join("video");
    let global = root.join("library");
    std::fs::create_dir_all(&video).unwrap();
    std::fs::create_dir_all(global.join("components")).unwrap();
    let video_json = serde_json::json!({
        "size": { "width": 64, "height": 36 }, "theme": "default",
        "scenes": [{ "component": "Card", "hold": 1 }, { "component": "Other", "hold": 1 }],
    });
    std::fs::write(video.join("video.json"), video_json.to_string()).unwrap();
    let tsx = |label: &str| {
        format!("export const props = {{}};\nexport default () => <Text style={{{{ fontSize: 20 }}}}>{label}</Text>;\n")
    };
    std::fs::write(global.join("components/Card.tsx"), tsx("one")).unwrap();
    std::fs::write(global.join("components/Other.tsx"), tsx("other")).unwrap();
    let library = Library {
        global: Some(global.clone()),
        ..Library::default()
    };
    let trees = |library: &Library| {
        let project = Project::load_with(&video, library).unwrap();
        Trees::of(&video, &project).unwrap()
    };
    let before = trees(&library);
    assert_eq!(before, trees(&library));
    std::fs::write(global.join("components/Card.tsx"), tsx("two")).unwrap();
    assert_ne!(trees(&library).components, before.components);
    // A component no scene uses is not part of the key.
    let used = trees(&library).components;
    std::fs::write(global.join("components/Unused.tsx"), tsx("unused")).unwrap();
    assert_eq!(trees(&library).components, used);
}
