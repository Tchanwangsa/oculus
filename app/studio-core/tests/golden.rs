//! Golden frames of the fixture projects at fixed times.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::{assert_golden, fixture, render};
use std::time::Instant;
use studio_core::{Project, RenderOptions, Renderer, Time};

#[test]
fn title_card() {
    assert_golden("title-card", &render("title-card", Time::Global(1.0)));
}

#[test]
fn layout_nested_flex_clip_image() {
    assert_golden("layout", &render("layout", Time::Global(0.0)));
}

#[test]
fn text_motion_mid_reveal() {
    assert_golden(
        "text-motion-0.45",
        &render("text-motion", Time::Scene { index: 0, t: 0.45 }),
    );
}

#[test]
fn text_motion_settled() {
    assert_golden("text-motion-2.0", &render("text-motion", Time::Global(2.0)));
}

#[test]
fn reveal_by_word_and_line() {
    for t in [0.5, 1.0, 1.6] {
        assert_golden(
            &format!("reveal-{t:.1}"),
            &render("reveal", Time::Global(t)),
        );
    }
}

#[test]
fn stroke_draw_and_arrows() {
    for t in [0.6, 1.6] {
        assert_golden(
            &format!("stroke-draw-{t:.1}"),
            &render("stroke-draw", Time::Global(t)),
        );
    }
}

#[test]
fn tabular_counter_mid_count() {
    assert_golden("counter-1.0", &render("counter", Time::Global(1.0)));
}

#[test]
fn narrated_enter_at_its_word() {
    // The card enters at `word("gradient")`: 0.62 s into the clip plus the
    // 0.25 s lead, so 0.87 s.
    for t in [0.85, 1.05] {
        assert_golden(
            &format!("narrated-{t:.2}"),
            &render("narrated", Time::Global(t)),
        );
    }
}

#[test]
fn enter_presets_keep_their_slots() {
    assert_golden("enter-0.75", &render("enter", Time::Global(0.75)));
}

#[test]
fn camera_zoom() {
    assert_golden("camera-1.0", &render("camera", Time::Global(1.0)));
}

/// Each transition kind at p = 0.25, 0.5 and 0.75 (linear easing, 1 s), one
/// after another in one 640x360 video.
#[test]
fn transitions_at_a_quarter_half_and_three_quarters() {
    let project = Project::load(fixture("transitions")).unwrap();
    let mut r = Renderer::new(&project, RenderOptions::default()).unwrap();
    let kinds = ["cut", "dissolve", "push", "wipe", "zoom", "magic"];
    for (i, kind) in kinds.iter().enumerate() {
        for p in [0.25, 0.5, 0.75] {
            let frame = r.frame(Time::Scene { index: i + 1, t: p }).unwrap();
            assert_golden(&format!("transition-{kind}-{p}"), &frame);
        }
    }
}

/// A card moving between rows inside a Morph, the footer gliding after it.
#[test]
fn in_scene_morph() {
    assert_golden("morph-0.5", &render("morph", Time::Global(0.5)));
}

/// Automatic magic move with unmatched children and a changed title.
#[test]
fn magic_move_with_unmatched_children_and_changed_text() {
    let project = Project::load(fixture("magic")).unwrap();
    let mut r = Renderer::new(&project, RenderOptions::default()).unwrap();
    for p in [0.2, 0.5, 0.8] {
        let frame = r.frame(Time::Scene { index: 1, t: p }).unwrap();
        assert_golden(&format!("magic-{p}"), &frame);
    }
}

#[test]
fn half_scale_proxy_keeps_the_layout() {
    let project = Project::load(fixture("title-card")).unwrap();
    let mut r = Renderer::new(
        &project,
        RenderOptions {
            scale: 0.5,
            motion_blur: None,
        },
    )
    .unwrap();
    let frame = r.frame(Time::Global(1.0)).unwrap();
    assert_eq!((frame.width, frame.height), (960, 540));
    assert_golden("title-card-540p", &frame);
}

const FIXTURES: [(&str, f64); 8] = [
    ("title-card", 1.0),
    ("layout", 0.0),
    ("text-motion", 0.45),
    ("reveal", 1.0),
    ("stroke-draw", 0.6),
    ("counter", 1.0),
    ("enter", 0.75),
    ("camera", 1.0),
];

/// ms per 1080p frame, warm, over 20 frames. Meaningful with --release.
#[test]
fn report_frame_cost() {
    for (name, t) in FIXTURES {
        let project = Project::load(fixture(name)).unwrap();
        let mut r = Renderer::new(&project, RenderOptions::default()).unwrap();
        r.frame(Time::Global(t)).unwrap();
        let n = 20;
        let start = Instant::now();
        for i in 0..n {
            r.frame(Time::Global(t + f64::from(i) * 0.01)).unwrap();
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / f64::from(n);
        println!("frame cost {name}: {ms:.2} ms/frame at 1080p");
    }
}

/// ms per 1080p frame for a scene alone, mid-dissolve and mid-magic (the
/// 640x360 fixtures laid out at 1080p). Meaningful with --release.
#[test]
fn report_transition_cost() {
    let cases = [
        ("transitions", "single scene", 2, 1.5),
        ("transitions", "dissolve", 2, 0.5),
        ("transitions", "push", 3, 0.5),
        ("transitions", "magic", 6, 0.5),
        ("magic", "magic, text halves", 1, 0.3),
        ("morph", "in-scene Morph", 0, 0.5),
    ];
    for (name, what, index, t) in cases {
        let mut project = Project::load(fixture(name)).unwrap();
        project.video.size = studio_core::Size {
            width: 1920,
            height: 1080,
        };
        let mut r = Renderer::new(&project, RenderOptions::default()).unwrap();
        r.frame(Time::Scene { index, t }).unwrap();
        let n = 20;
        let start = Instant::now();
        for i in 0..n {
            let t = t + f64::from(i) * 0.002;
            r.frame(Time::Scene { index, t }).unwrap();
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / f64::from(n);
        println!("frame cost {what} ({name}): {ms:.2} ms/frame at 1080p");
    }
}
