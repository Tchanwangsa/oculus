use super::super::cache::{self, Used};
use super::super::mux::concat_list;
use super::super::preset::Preset;
use crate::test_support::Scratch;

#[test]
fn pruning_keeps_what_any_presets_last_render_used() {
    let dir = Scratch::new("render-prune");
    let segments = cache::segments(&dir);
    std::fs::create_dir_all(&segments).unwrap();
    for name in ["a.mp4", "b.mp4", "c.mp4", "d.mp4.part", "notes.txt"] {
        std::fs::write(segments.join(name), b"x").unwrap();
    }
    for name in ["narration-1.wav", "narration-2.wav", "concat-final.txt"] {
        std::fs::write(cache::root(&dir).join(name), b"x").unwrap();
    }
    let used = |segs: &[&str], track: Option<&str>| Used {
        segments: segs.iter().map(|s| s.to_string()).collect(),
        track: track.map(str::to_owned),
        ..Used::default()
    };

    let deleted = cache::record_and_prune(
        &dir,
        Preset::Draft,
        used(&["a.mp4"], Some("narration-1.wav")),
    )
    .unwrap();
    assert_eq!(deleted, ["b.mp4", "c.mp4", "d.mp4.part", "narration-2.wav"]);
    std::fs::write(segments.join("b.mp4"), b"x").unwrap();
    let deleted = cache::record_and_prune(&dir, Preset::Final, used(&["b.mp4"], None)).unwrap();
    assert!(deleted.is_empty(), "{deleted:?}");
    assert!(segments.join("a.mp4").exists() && segments.join("b.mp4").exists());
    assert!(segments.join("notes.txt").exists());
    assert!(cache::root(&dir).join("concat-final.txt").exists());

    // A new draft drops the old draft's segment, not the final's.
    let deleted = cache::record_and_prune(&dir, Preset::Draft, used(&["b.mp4"], None)).unwrap();
    assert_eq!(deleted, ["a.mp4", "narration-1.wav"]);
}

#[test]
fn the_concat_list_quotes_each_path() {
    let list = concat_list(&["/v/.cache/a.mp4".into(), "/it's/b.mp4".into()]);
    assert_eq!(list, "file '/v/.cache/a.mp4'\nfile '/it'\\''s/b.mp4'\n");
}
