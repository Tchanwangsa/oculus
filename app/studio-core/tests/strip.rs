//! `strip`: a contact sheet of frames, the same at any thread count, with
//! its times clamped inside the video and a failing frame named.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::{assert_golden, fixture};
use serde_json::json;
use std::time::Instant;
use studio_core::{Frame, Project, StripOptions, StudioError, strip};

fn as_frame(s: studio_core::Strip) -> Frame {
    Frame {
        width: s.width,
        height: s.height,
        rgba: s.rgba,
        logs: Vec::new(),
        samples: 1,
    }
}

/// Six frames across every transition of the 640x360 fixture, three a row,
/// at 240x135.
fn small(threads: usize) -> StripOptions {
    StripOptions {
        n: 6,
        cols: 3,
        scale: 0.375,
        threads: Some(threads),
        ..StripOptions::default()
    }
}

#[test]
fn transitions_contact_sheet() {
    let project = Project::load(fixture("transitions")).unwrap();
    let s = strip(&project, small(2)).unwrap();
    assert_eq!(s.tile, (240, 135));
    assert_eq!(s.cols, 3);
    assert_eq!(s.times.len(), 6);
    assert_eq!((s.width, s.height), (8 + 3 * 248, 8 + 2 * (135 + 22 + 8)));
    assert_golden("strip-transitions", &as_frame(s));
}

/// At 160 px a tile, the time and the scene stack in a taller bar.
#[test]
fn narrow_tiles_stack_their_labels() {
    let project = Project::load(fixture("transitions")).unwrap();
    let options = StripOptions {
        n: 4,
        cols: 4,
        scale: 0.25,
        from: 4.0,
        to: 7.0,
        threads: Some(2),
        ..StripOptions::default()
    };
    let s = strip(&project, options).unwrap();
    assert_eq!((s.width, s.height), (8 + 4 * 168, 8 + 90 + 38 + 8));
    assert_golden("strip-narrow", &as_frame(s));
}

#[test]
fn one_thread_and_many_agree_byte_for_byte() {
    let project = Project::load(fixture("transitions")).unwrap();
    let one = strip(&project, small(1)).unwrap();
    let many = strip(&project, small(4)).unwrap();
    assert_eq!(many.threads, 4);
    assert!(one.rgba == many.rgba, "1 thread and 4 threads differ");
    assert_eq!(one.times, many.times);
}

#[test]
fn a_tile_is_the_frame_pixel_for_pixel() {
    let project = Project::load(fixture("transitions")).unwrap();
    let s = strip(&project, small(2)).unwrap();
    let mut r = studio_core::Renderer::new(
        &project,
        studio_core::RenderOptions {
            scale: 0.375,
            motion_blur: None,
        },
    )
    .unwrap();
    // Tile 4 is the second of the second row.
    let f = r.frame(studio_core::Time::Global(s.times[4])).unwrap();
    let (x, y) = (8 + 248, 8 + 165);
    for row in 0..135usize {
        let at = ((y + row) * s.width as usize + x) * 4;
        assert_eq!(
            &s.rgba[at..at + 240 * 4],
            &f.rgba[row * 960..(row + 1) * 960],
            "row {row}"
        );
    }
}

#[test]
fn the_end_is_clamped_and_one_frame_is_from() {
    let project = Project::load(fixture("transitions")).unwrap();
    let length = project.length().unwrap();
    let s = strip(
        &project,
        StripOptions {
            from: 13.0,
            to: 100.0,
            n: 2,
            ..small(1)
        },
    )
    .unwrap();
    assert_eq!(s.times[0], 13.0);
    assert!(s.times[1] < length && s.times[1] > length - 1e-5);
    let one = strip(
        &project,
        StripOptions {
            from: 4.5,
            n: 1,
            ..small(1)
        },
    )
    .unwrap();
    assert_eq!(one.times, [4.5]);
    assert_eq!(one.cols, 1);
    assert_eq!((one.width, one.height), (8 + 248, 8 + 165));
}

#[test]
fn an_oversized_sheet_is_refused_before_rendering() {
    let project = Project::load(fixture("transitions")).unwrap();
    let options = StripOptions {
        n: 100_000,
        scale: 1.0,
        ..StripOptions::default()
    };
    let start = Instant::now();
    let e = strip(&project, options).unwrap_err().to_string();
    assert!(e.contains("lower the scale or the number of frames"), "{e}");
    assert!(start.elapsed().as_secs_f64() < 1.0, "it rendered first");
}

#[test]
fn a_failing_frame_names_its_time_and_scene() {
    let dir = std::env::temp_dir().join(format!("studio-strip-fail-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("components")).unwrap();
    let video = json!({
        "size": { "width": 64, "height": 36 },
        "theme": "default",
        "scenes": [
            { "component": "Fine", "hold": 1.0 },
            { "component": "Late", "hold": 2.0, "transition": { "kind": "cut" } }
        ]
    });
    std::fs::write(dir.join("video.json"), video.to_string()).unwrap();
    let fine = "export const props = {};\nexport default () => <Box />;\n";
    let late = "export const props = {};\nexport default (_p, { t }) => {\n  if (t > 1) throw new Error(\"too late\");\n  return <Box />;\n};\n";
    std::fs::write(dir.join("components/Fine.tsx"), fine).unwrap();
    std::fs::write(dir.join("components/Late.tsx"), late).unwrap();
    let project = Project::load(&dir).unwrap();
    // Frames at 0, 0.5, ..., 2.5: 2.5 is the first to throw.
    let options = StripOptions {
        n: 6,
        to: 2.5,
        scale: 1.0,
        threads: Some(3),
        ..StripOptions::default()
    };
    let e = strip(&project, options).unwrap_err();
    let StudioError::Frame { t, scene, .. } = &e else {
        panic!("not a frame error: {e}");
    };
    assert_eq!((*t, *scene), (2.5, 1));
    assert_eq!(e.kind(), "component");
    let message = e.to_string();
    assert!(
        message.starts_with("frame at 2.500 s (scene 1, 1.500 s in): Late.tsx:3"),
        "{message}"
    );
}

/// ms for a default strip of the whole transitions fixture, one thread vs
/// the default count. Meaningful with --release.
#[test]
fn report_strip_cost() {
    let project = Project::load(fixture("transitions")).unwrap();
    for threads in [Some(1), None] {
        let options = StripOptions {
            threads,
            ..StripOptions::default()
        };
        strip(&project, options).unwrap();
        let start = Instant::now();
        let s = strip(&project, options).unwrap();
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        println!(
            "strip cost: {ms:.1} ms for {} frames at {}x{} on {} thread(s)",
            s.times.len(),
            s.tile.0,
            s.tile.1,
            s.threads
        );
    }
}
