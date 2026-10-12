//! The whole-frame timeline at `video.json`'s fps: scene frame ranges, a
//! global frame time renders the frame an encoder would, motion blur takes
//! the video's fps, and a scene's fingerprint moves when and only when it
//! or the scene before it changes.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::fixture;
use serde_json::json;
use studio_core::{
    MotionBlur, Project, Purpose, RenderOptions, Renderer, StripOptions, Time, strip,
};

fn at_fps(name: &str, fps: u32) -> Project {
    let mut p = Project::load(fixture(name)).unwrap();
    p.video.fps = fps;
    p
}

#[test]
fn fps_defaults_to_30_and_sets_the_frame_ranges() {
    let p = Project::load(fixture("transitions")).unwrap();
    assert_eq!(p.fps(), 30);
    assert_eq!(p.frames().unwrap(), 7 * 60);
    assert_eq!(p.scene_frames(3).unwrap(), 180..240);
    // The narrated scene's 4.25 s rounds up to 128 frames at 30 fps, 107
    // at 25.
    let narrated = Project::load(fixture("narrated")).unwrap();
    assert_eq!(narrated.scene_frames(0).unwrap(), 0..128);
    let narrated = at_fps("narrated", 25);
    assert_eq!(narrated.scene_frames(0).unwrap(), 0..107);
    assert_eq!(narrated.scene_length(0).unwrap(), 107.0 / 25.0);
    assert_eq!(narrated.frames().unwrap(), 107 + 38);
}

#[test]
fn a_frame_at_its_global_time_is_the_encoders_frame() {
    let p = at_fps("transitions", 24);
    let mut by_index = Renderer::new(&p, RenderOptions::default()).unwrap();
    let mut by_time = Renderer::new(&p, RenderOptions::default()).unwrap();
    // Around the cut into scene 1, through the dissolve into scene 2 and
    // the video's last frame.
    let last = p.frames().unwrap() - 1;
    for k in [0, 47, 48, 49, 96, 107, 143, last] {
        let a = by_index.frame(p.frame_time(k).unwrap()).unwrap();
        let b = by_time.frame(Time::Global(k as f64 / 24.0)).unwrap();
        assert!(a.rgba == b.rgba, "frame {k}");
    }
    assert!(p.frame_time(last + 1).is_err());
}

#[test]
fn a_scene_samples_the_same_times_wherever_it_sits() {
    // Lengthening scene 0 shifts every later scene, but scene 3's frames
    // are the same scene-relative times.
    let p = at_fps("transitions", 30);
    let mut moved = p.clone();
    moved.video.scenes[0].hold = Some(2.37);
    let a = p.scene_frames(3).unwrap();
    let b = moved.scene_frames(3).unwrap();
    assert_eq!(b.start, a.start + 12);
    assert_eq!(a.end - a.start, b.end - b.start);
    for j in 0..(a.end - a.start) {
        assert_eq!(
            p.frame_time(a.start + j).unwrap(),
            moved.frame_time(b.start + j).unwrap()
        );
    }
}

#[test]
fn motion_blur_takes_the_videos_fps() {
    let p = at_fps("transitions", 60);
    assert_eq!(
        p.motion_blur(Purpose::Final).unwrap(),
        Some(MotionBlur::at(60))
    );
    assert_eq!(MotionBlur::at(60).fps, 60.0);
}

#[test]
fn strips_agree_across_thread_counts_at_any_fps() {
    let p = at_fps("transitions", 25);
    let options = |threads| StripOptions {
        n: 6,
        cols: 3,
        scale: 0.25,
        threads: Some(threads),
        ..StripOptions::default()
    };
    let one = strip(&p, options(1)).unwrap();
    let three = strip(&p, options(3)).unwrap();
    assert!(one.rgba == three.rgba);
}

/// Every scene's fingerprint.
fn prints(p: &Project) -> Vec<Vec<u8>> {
    (0..p.video.scenes.len())
        .map(|i| p.scene_fingerprint(i).unwrap())
        .collect()
}

/// The scenes whose fingerprint differs between `a` and `b`.
fn changed(a: &Project, b: &Project) -> Vec<usize> {
    let (a, b) = (prints(a), prints(b));
    (0..a.len()).filter(|&i| a[i] != b[i]).collect()
}

#[test]
fn a_fingerprint_moves_with_its_scene_and_the_one_before() {
    let p = Project::load(fixture("transitions")).unwrap();
    assert_eq!(prints(&p), prints(&p.clone()), "deterministic");
    let all: Vec<usize> = (0..7).collect();
    let edit = |f: &dyn Fn(&mut Project)| {
        let mut q = p.clone();
        f(&mut q);
        changed(&p, &q)
    };

    let props = |q: &mut Project| {
        q.video.scenes[2]
            .props
            .insert("title".into(), json!("Moved"));
    };
    assert_eq!(edit(&props), [2, 3], "props");
    let transition = |q: &mut Project| q.video.scenes[4].transition = None;
    assert_eq!(edit(&transition), [4, 5], "transition");
    let longer = |q: &mut Project| q.video.scenes[0].hold = Some(2.5);
    assert_eq!(edit(&longer), [0, 1], "length");
    let same_frames = |q: &mut Project| q.video.scenes[5].hold = Some(2.0 + 1e-9);
    assert_eq!(edit(&same_frames), [5, 6], "the hold is in the spec");
    let last = |q: &mut Project| q.video.scenes[6].component = "Other".into();
    assert_eq!(edit(&last), [6], "the last scene has no follower");

    let fps = |q: &mut Project| q.video.fps = 25;
    assert_eq!(edit(&fps), all, "fps");
    let size = |q: &mut Project| q.video.size.width = 320;
    assert_eq!(edit(&size), all, "size");
    let motion = |q: &mut Project| q.video.motion.morph_color = Some("oklab".into());
    assert_eq!(edit(&motion), all, "video motion");
    let theme = |q: &mut Project| q.theme.tokens["color"]["accent"] = json!("#ff0000");
    assert_eq!(edit(&theme), all, "theme");
    let voice = |q: &mut Project| q.video.voice = None;
    assert_eq!(edit(&voice), [] as [usize; 0], "voice reaches no frame");
}

#[test]
fn a_fingerprint_moves_with_the_words() {
    let p = Project::load(fixture("narrated")).unwrap();
    let mut q = p.clone();
    let clip = q
        .manifest
        .as_mut()
        .unwrap()
        .scenes
        .get_mut("intro")
        .unwrap();
    clip.words[3].start += 0.05;
    assert_eq!(changed(&p, &q), [0, 1], "word timing");
    let mut q = p.clone();
    let clip = q
        .manifest
        .as_mut()
        .unwrap()
        .scenes
        .get_mut("intro")
        .unwrap();
    clip.words[3].confidence = -9.0;
    clip.audio = "elsewhere.wav".into();
    assert_eq!(
        changed(&p, &q),
        [] as [usize; 0],
        "confidence and the audio file reach no frame"
    );
    let mut q = p.clone();
    q.manifest
        .as_mut()
        .unwrap()
        .scenes
        .get_mut("intro")
        .unwrap()
        .duration += 0.5;
    assert_eq!(changed(&p, &q), [0, 1], "a longer clip is more frames");
}

#[test]
fn a_fingerprint_ignores_key_order() {
    let p = Project::load(fixture("transitions")).unwrap();
    let mut q = p.clone();
    let props = &mut q.video.scenes[1].props;
    let reversed: serde_json::Map<_, _> = props.clone().into_iter().rev().collect();
    *props = reversed;
    assert_eq!(changed(&p, &q), [] as [usize; 0]);
}

/// Every frame of scene `index`, transition included.
fn scene_pixels(p: &Project, index: usize) -> Vec<Vec<u8>> {
    let mut r = Renderer::new(p, RenderOptions::default()).unwrap();
    let frames = p.scene_frames(index).unwrap();
    frames
        .map(|k| r.frame(p.frame_time(k).unwrap()).unwrap().rgba)
        .collect()
}

#[test]
fn an_id_seeds_math_random_wherever_the_scene_sits() {
    // Four Dots scenes "a".."d", whose bars take `Math.random` widths.
    let p = Project::load(fixture("random")).unwrap();
    let mut q = p.clone();
    // a b c d -> b c a d: "c" moves from 2 to 1 behind the same "b".
    let a = q.video.scenes.remove(0);
    q.video.scenes.insert(2, a);
    assert_eq!(
        p.scene_fingerprint(2).unwrap(),
        q.scene_fingerprint(1).unwrap()
    );
    assert!(scene_pixels(&p, 2) == scene_pixels(&q, 1), "c's frames");
    // Inserting a scene before them keeps every later fingerprint.
    let mut r = p.clone();
    let mut new = r.video.scenes[0].clone();
    new.id = Some("new".into());
    r.video.scenes.insert(0, new);
    for i in 1..4 {
        assert_eq!(
            p.scene_fingerprint(i).unwrap(),
            r.scene_fingerprint(i + 1).unwrap()
        );
    }

    // The ids are what tell the scenes apart: same component and props,
    // other numbers.
    assert!(scene_pixels(&p, 0).last() != scene_pixels(&p, 1).last());
    // Without ids the index seeds, and a move moves the fingerprint.
    let mut anon = p.clone();
    for s in &mut anon.video.scenes {
        s.id = None;
    }
    let mut moved = anon.clone();
    let a = moved.video.scenes.remove(0);
    moved.video.scenes.insert(2, a);
    assert_ne!(
        anon.scene_fingerprint(2).unwrap(),
        moved.scene_fingerprint(1).unwrap()
    );
}
