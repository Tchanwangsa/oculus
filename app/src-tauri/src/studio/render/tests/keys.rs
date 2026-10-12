use super::super::key::{segment_key, tree_hash, Trees};
use super::super::quality::{Quality, Settings};
use crate::test_support::Scratch;

fn settings(q: Quality) -> Settings {
    Settings::new(q, (640, 360), 30, None).unwrap()
}

#[test]
fn a_segment_key_changes_with_each_input() {
    let trees = Trees {
        components: "c".into(),
        assets: "a".into(),
    };
    let base = segment_key(b"scene", &trees, &settings(Quality::Final));
    assert_eq!(
        base,
        segment_key(b"scene", &trees, &settings(Quality::Final))
    );
    let others = [
        segment_key(b"scene!", &trees, &settings(Quality::Final)),
        segment_key(b"scene", &trees, &settings(Quality::Draft)),
        segment_key(
            b"scene",
            &Trees {
                components: "c2".into(),
                ..trees.clone()
            },
            &settings(Quality::Final),
        ),
        segment_key(
            b"scene",
            &Trees {
                assets: "a2".into(),
                ..trees.clone()
            },
            &settings(Quality::Final),
        ),
        segment_key(
            b"scene",
            &trees,
            &Settings::new(Quality::Final, (640, 360), 60, None).unwrap(),
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
