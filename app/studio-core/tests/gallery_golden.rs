//! Gallery stills pinned by golden frames, and the gallery's determinism
//! across thread counts.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::{assert_golden, fixture};
use studio_core::gallery::{self, GalleryOptions, Moment};
use studio_core::{ComponentSource, Project, Size, Starter, Theme};

#[test]
fn every_example_is_a_still_in_declaration_order() {
    let project = Project::load(fixture("gallery")).unwrap();
    let g = gallery::render(&project, "Tile", None, &GalleryOptions::default()).unwrap();
    let names: Vec<&str> = g.stills.iter().map(|s| s.example.as_str()).collect();
    assert_eq!(names, ["short", "long"]);
    // The default moment is the middle of the scene's 2 s hold.
    assert_eq!((g.moment.t, g.moment.duration), (1.0, 2.0));
    for (name, still) in names.iter().zip(&g.stills) {
        assert_golden(&format!("gallery-tile-{name}"), &still.frame);
    }
}

#[test]
fn stills_are_the_same_at_any_thread_count() {
    let project = Project::load(fixture("gallery")).unwrap();
    let one = |threads| {
        gallery::render(
            &project,
            "Tile",
            Some(0.5),
            &GalleryOptions {
                scale: 1.0,
                threads: Some(threads),
            },
        )
        .unwrap()
    };
    let serial = one(1);
    let parallel = one(2);
    assert_eq!(serial.threads, 1);
    assert_eq!(parallel.threads, 2);
    for (a, b) in serial.stills.iter().zip(&parallel.stills) {
        assert_eq!(a.example, b.example);
        assert!(a.frame.rgba == b.frame.rgba, "{}: frames differ", a.example);
    }
}

#[test]
fn a_moment_outside_the_preview_is_refused() {
    let project = Project::load(fixture("gallery")).unwrap();
    let err = gallery::render(&project, "Tile", Some(2.0), &GalleryOptions::default()).unwrap_err();
    assert!(err.to_string().contains("t must be in [0, 2)"), "{err}");
}

#[test]
fn the_sheet_is_one_png() {
    let project = Project::load(fixture("gallery")).unwrap();
    let g = gallery::render(&project, "Tile", None, &GalleryOptions::default()).unwrap();
    let path = std::env::temp_dir().join(format!("gallery-sheet-{}.png", std::process::id()));
    g.write_sheet(&path, 2).unwrap();
    let (w, h) = image::image_dimensions(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert!(w >= 2 * 640 && h >= 360, "{w}x{h}");
}

/// Every example of every starter component, at the default theme, pinned
/// by a golden each (`starter-<Component>-<example>.png`); and one warm
/// still, so the second bundled theme is pinned too.
#[test]
fn every_starter_example_has_its_golden() {
    // Laid out at 1920x1080 and drawn at half scale (the 540p proxy), so the
    // layouts are the ones a video gets.
    let size = Size {
        width: 1920,
        height: 1080,
    };
    let moment = Moment {
        t: 1.0,
        duration: 2.0,
    };
    let options = GalleryOptions {
        scale: 0.5,
        threads: None,
    };
    let theme = Theme::bundled("default").unwrap();
    for name in Starter.names() {
        let g =
            gallery::render_source(&Starter, &theme, size, None, &name, moment, &options).unwrap();
        assert!(!g.stills.is_empty(), "{name} has no examples");
        for still in &g.stills {
            assert_golden(&format!("starter-{name}-{}", still.example), &still.frame);
        }
    }
    let lib = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("library/themes/warm.json"),
    )
    .unwrap();
    let warm = Theme::parse(&lib, std::path::Path::new("library/themes/warm.json")).unwrap();
    let g =
        gallery::render_source(&Starter, &warm, size, None, "TitleCard", moment, &options).unwrap();
    let long = g.stills.iter().find(|s| s.example == "long").unwrap();
    assert_golden("starter-TitleCard-long-warm", &long.frame);
}
