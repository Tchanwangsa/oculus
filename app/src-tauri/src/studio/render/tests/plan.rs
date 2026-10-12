use std::path::PathBuf;

use super::super::audio::{offset, RATE};
use super::super::cache;
use super::super::key::Trees;
use super::super::plan::{self, Scene};
use super::super::preset::Preset;
use super::super::workers::Job;
use super::settings;
use crate::test_support::Scratch;

fn scene(index: usize, frames: std::ops::Range<u64>, clip: Option<&str>) -> Scene {
    Scene {
        index,
        id: Some(format!("s{index}")),
        frames,
        fingerprint: format!("scene {index}").into_bytes(),
        clip: clip.map(PathBuf::from),
    }
}

fn trees() -> Trees {
    Trees {
        components: "c".into(),
        assets: "a".into(),
    }
}

#[test]
fn a_segment_is_cached_when_its_keyed_file_exists() {
    let dir = Scratch::new("render-plan");
    let settings = settings(Preset::Final, (640, 360), 30);
    let scenes = [scene(0, 0..10, None), scene(1, 10..25, None)];
    let first = plan::segments(&dir, &scenes, &trees(), &settings);
    assert!(first.iter().all(|s| !s.cached));
    assert_ne!(first[0].key, first[1].key);

    std::fs::create_dir_all(cache::segments(&dir)).unwrap();
    std::fs::write(&first[1].path, b"mp4").unwrap();
    let second = plan::segments(&dir, &scenes, &trees(), &settings);
    assert_eq!(
        second.iter().map(|s| s.cached).collect::<Vec<_>>(),
        [false, true]
    );
    assert_eq!(second[1].path, cache::segment(&dir, &second[1].key));
    assert_eq!(second[1].len(), 15);
}

#[test]
fn jobs_cover_uncached_segments_at_scene_relative_frame_times() {
    let dir = Scratch::new("render-jobs");
    let settings = settings(Preset::Final, (640, 360), 25);
    let scenes = [
        scene(0, 0..3, None),
        scene(1, 3..5, None),
        scene(2, 5..7, None),
    ];
    let mut segments = plan::segments(&dir, &scenes, &trees(), &settings);
    segments[1].cached = true;
    let jobs = plan::jobs(&segments, 25);
    let expect = |scene, k: u64, n: u64| Job {
        scene,
        t: k as f64 / 25.0,
        global: n as f64 / 25.0,
    };
    assert_eq!(
        jobs,
        [
            expect(0, 0, 0),
            expect(0, 1, 1),
            expect(0, 2, 2),
            expect(2, 0, 5),
            expect(2, 1, 6),
        ]
    );
}

#[test]
fn a_clip_lands_at_its_scenes_first_frame_plus_lead() {
    let scenes = [
        scene(0, 0..45, None),
        scene(1, 45..161, Some("/n/a.wav")),
        scene(2, 161..200, Some("/n/b.wav")),
    ];
    let clips = plan::clips(&scenes, 30, 0.25);
    assert_eq!(clips.len(), 2);
    assert_eq!(clips[0].path, PathBuf::from("/n/a.wav"));
    // 45 / 30 + 0.25 = 1.75 s; 161 / 30 + 0.25 = 5.61667 s.
    assert_eq!(offset(clips[0].at), 84_000);
    assert_eq!(offset(clips[1].at), 269_600);
    assert_eq!(RATE, 48_000);
}
