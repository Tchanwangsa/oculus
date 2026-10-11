//! Motion blur: a still frame is one sample and exactly the sharp frame, a
//! fast move takes more, blurred frames are seekable, and the `motionBlur`
//! token decides which renders blur.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::fixture;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Instant;
use studio_core::{MotionBlur, Project, Purpose, RenderOptions, Renderer, Time};

fn blurred() -> RenderOptions {
    RenderOptions {
        motion_blur: Some(MotionBlur::at(30)),
        ..RenderOptions::default()
    }
}

#[test]
fn a_still_frame_is_one_sample_and_the_sharp_frame() {
    let project = Project::load(fixture("transitions")).unwrap();
    let mut sharp = Renderer::new(&project, RenderOptions::default()).unwrap();
    let mut blur = Renderer::new(&project, blurred()).unwrap();
    // Past the dissolve into scene 2, nothing moves.
    for t in [1.5, 1.99] {
        let at = Time::Scene { index: 2, t };
        let (s, b) = (sharp.frame(at).unwrap(), blur.frame(at).unwrap());
        assert_eq!(b.samples, 1, "t = {t}");
        assert!(s.rgba == b.rgba, "t = {t}: the blurred still differs");
    }
}

#[test]
fn a_fast_push_takes_more_samples() {
    let project = Project::load(fixture("transitions")).unwrap();
    let mut sharp = Renderer::new(&project, RenderOptions::default()).unwrap();
    let mut blur = Renderer::new(&project, blurred()).unwrap();
    // The push moves 640 px a second: 10.7 px over a 1/60 s shutter.
    let at = Time::Scene { index: 3, t: 0.5 };
    let b = blur.frame(at).unwrap();
    assert_eq!(b.samples, 11);
    assert!(b.rgba != sharp.frame(at).unwrap().rgba);
    // A dissolve only fades: nothing moves, one sample.
    let fade = blur.frame(Time::Scene { index: 2, t: 0.5 }).unwrap();
    assert_eq!(fade.samples, 1);
}

#[test]
fn blurred_frames_are_seekable() {
    let project = Project::load(fixture("transitions")).unwrap();
    let times = [0.2, 4.5, 6.5, 10.5, 12.4, 6.2];
    let mut warm = Renderer::new(&project, blurred()).unwrap();
    let sweep: Vec<Vec<u8>> = times
        .iter()
        .map(|&t| warm.frame(Time::Global(t)).unwrap().rgba)
        .collect();
    for (i, &t) in times.iter().enumerate().rev() {
        let cold = Renderer::new(&project, blurred())
            .unwrap()
            .frame(Time::Global(t))
            .unwrap();
        assert!(cold.rgba == sweep[i], "cold t = {t}");
    }
}

/// The transitions fixture with `motion` merged into its video.json.
fn with_motion(name: &str, motion: Value) -> PathBuf {
    let src = fixture("transitions");
    let dir = std::env::temp_dir().join(format!("studio-blur-{name}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("components")).unwrap();
    let mut video: Value =
        serde_json::from_str(&std::fs::read_to_string(src.join("video.json")).unwrap()).unwrap();
    for (k, v) in motion.as_object().unwrap() {
        video["motion"][k] = v.clone();
    }
    std::fs::write(dir.join("video.json"), video.to_string()).unwrap();
    std::fs::copy(
        src.join("components/Slide.tsx"),
        dir.join("components/Slide.tsx"),
    )
    .unwrap();
    dir
}

#[test]
fn the_token_picks_which_renders_blur() {
    let finals = Project::load(fixture("transitions")).unwrap();
    assert_eq!(finals.motion_blur(Purpose::Draft).unwrap(), None);
    assert_eq!(
        finals.motion_blur(Purpose::Final).unwrap(),
        Some(MotionBlur::at(30))
    );
    let always = Project::load(with_motion(
        "always",
        serde_json::json!({ "motionBlur": "always" }),
    ));
    assert!(
        always
            .unwrap()
            .motion_blur(Purpose::Draft)
            .unwrap()
            .is_some()
    );
    let off = Project::load(with_motion(
        "off",
        serde_json::json!({ "motionBlur": "off" }),
    ));
    assert_eq!(off.unwrap().motion_blur(Purpose::Final).unwrap(), None);
}

#[test]
fn a_bad_token_names_its_level() {
    let p = Project::load(with_motion(
        "bad",
        serde_json::json!({ "motionBlur": "final" }),
    ))
    .unwrap();
    let e = p.motion_blur(Purpose::Final).unwrap_err().to_string();
    assert!(
        e.starts_with("video.json motion.motionBlur: unknown motionBlur `final`"),
        "{e}"
    );
    assert!(e.contains("did you mean `finals`"), "{e}");
    let report = p.lint().unwrap();
    let other: Vec<_> = report
        .issues
        .iter()
        .filter(|i| !common::component_rule(&i.to_string()))
        .collect();
    assert_eq!(other.len(), 1, "{report}");
    assert!(report.to_string().contains("video.json motion.motionBlur"));
}

/// ms per 1080p frame with motion blur on, for a still frame and for each
/// transition kind's middle (laid out at 1080p). Meaningful with --release.
#[test]
fn report_motion_blur_cost() {
    let mut project = Project::load(fixture("transitions")).unwrap();
    project.video.size = studio_core::Size {
        width: 1920,
        height: 1080,
    };
    let cases = [
        ("still", 2, 1.5),
        ("dissolve", 2, 0.5),
        ("push", 3, 0.5),
        ("wipe", 4, 0.5),
        ("zoom", 5, 0.5),
        ("magic", 6, 0.5),
    ];
    for options in [RenderOptions::default(), blurred()] {
        let mut r = Renderer::new(&project, options).unwrap();
        for (what, index, t) in cases {
            let samples = r.frame(Time::Scene { index, t }).unwrap().samples;
            let n = 5;
            let start = Instant::now();
            for i in 0..n {
                let t = t + f64::from(i) * 0.002;
                r.frame(Time::Scene { index, t }).unwrap();
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / f64::from(n);
            let blur = if options.motion_blur.is_some() {
                "blur"
            } else {
                "sharp"
            };
            println!("motion blur cost {what} ({blur}): {ms:.2} ms/frame, {samples} samples at t");
        }
    }
}

/// The push at p = 0.5, blurred: 10.7 px of motion over the shutter, two
/// sub-frames.
#[test]
fn blurred_push_golden() {
    let project = Project::load(fixture("transitions")).unwrap();
    let mut r = Renderer::new(&project, blurred()).unwrap();
    let f = r.frame(Time::Scene { index: 3, t: 0.5 }).unwrap();
    common::assert_golden("motion-blur-push-0.5", &f);
}
