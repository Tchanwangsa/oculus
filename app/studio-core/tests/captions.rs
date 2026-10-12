//! Burned-in captions: the `captions` fixture is the `narrated` one with
//! video.json's `captions` on.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::{assert_golden, fixture, render};
use studio_core::{MotionBlur, Project, RenderOptions, Renderer, Time, captions};

#[test]
fn a_captioned_frame_highlights_the_word_being_said() {
    // 1.05 s: "gradient" (0.87 to 1.35 s with the lead) is being said.
    assert_golden("captions-1.05", &render("captions", Time::Global(1.05)));
    // 2.6 s: "step", late in the phrase; the earlier words are said.
    assert_golden("captions-2.60", &render("captions", Time::Global(2.6)));
}

#[test]
fn captions_are_off_unless_video_json_turns_them_on() {
    let at = Time::Global(1.05);
    assert_eq!(render("narrated", at).rgba, {
        let project = Project::load(fixture("captions")).unwrap();
        let r = Renderer::new(&project, RenderOptions::default()).unwrap();
        r.without_captions().frame(at).unwrap().rgba
    });
    assert_ne!(render("narrated", at).rgba, render("captions", at).rgba);
    // The silent outro has none.
    let outro = Time::Scene { index: 1, t: 0.75 };
    assert_eq!(
        render("narrated", outro).rgba,
        render("captions", outro).rgba
    );
}

#[test]
fn a_blurred_frame_draws_its_caption_once_and_sharp() {
    let project = Project::load(fixture("captions")).unwrap();
    let blurred = |captioned: bool| {
        // A wide shutter, so the rising card moves several px and the
        // sub-frames span the highlight's move from "the" to "gradient".
        let shutter = MotionBlur {
            fps: 5.0,
            shutter: 1.0,
            max_samples: 32,
        };
        let options = RenderOptions {
            motion_blur: Some(shutter),
            ..RenderOptions::default()
        };
        let r = Renderer::new(&project, options).unwrap();
        let mut r = if captioned { r } else { r.without_captions() };
        // 1.0 s: the card is rising (blurred) and "gradient" is lit.
        r.frame(Time::Global(1.0)).unwrap()
    };
    let (with, without) = (blurred(true), blurred(false));
    assert!(with.samples > 1, "the frame should be blurred");
    let sharp = render("captions", Time::Global(1.0));
    let plain = render("narrated", Time::Global(1.0));
    // Wherever the caption drew on the sharp frame, the blurred frame has
    // the same pixels: the caption was drawn once, at the frame's time.
    let mut caption_pixels = 0;
    for i in (0..sharp.rgba.len()).step_by(4) {
        let under_caption = sharp.rgba[i..i + 4] != plain.rgba[i..i + 4];
        let moved = without.rgba[i..i + 4] != plain.rgba[i..i + 4];
        if under_caption && !moved {
            caption_pixels += 1;
            let d = (0..4)
                .map(|c| with.rgba[i + c].abs_diff(sharp.rgba[i + c]))
                .max()
                .unwrap();
            assert!(d <= 1, "pixel {} differs by {d}", i / 4);
        }
    }
    assert!(caption_pixels > 1000, "{caption_pixels}");
}

#[test]
fn the_fingerprint_follows_the_switch() {
    let on = Project::load(fixture("captions")).unwrap();
    let off = Project::load(fixture("narrated")).unwrap();
    assert_ne!(
        on.scene_fingerprint(0).unwrap(),
        off.scene_fingerprint(0).unwrap()
    );
}

#[test]
fn subtitles_are_the_same_phrases_in_video_time() {
    let project = Project::load(fixture("narrated")).unwrap();
    let cues = project.caption_cues().unwrap();
    assert_eq!(cues.len(), 1);
    assert_eq!(
        cues[0].lines,
        [
            "We follow the gradient downhill,",
            "one small step at a time."
        ]
    );
    // The first word at 0.25 s, shown 0.15 s before it; hidden 0.4 s after
    // the last word ends at 3.25 s.
    assert!((cues[0].start - 0.1).abs() < 1e-9, "{cues:?}");
    assert!((cues[0].end - 3.65).abs() < 1e-9, "{cues:?}");
    assert!(captions::vtt(&cues).starts_with("WEBVTT\n\n00:00:00.100 --> 00:00:03.650\n"));
}
