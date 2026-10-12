//! Gallery stills pinned by golden frames, and the gallery's determinism
//! across thread counts.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::{assert_golden, fixture};
use studio_core::gallery::{self, Gallery, GalleryOptions, Moment, Still};
use studio_core::{ComponentSource, Frame, Project, Size, Starter, Theme};

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

/// Every starter component as one sheet, `starter-<Component>.png`: a row
/// per example (in declaration order), a column per moment of a 6 s
/// preview, so a drawing primitive is pinned while it draws on as well as
/// drawn. Laid out at 1920x1080 (the layouts a video gets) and drawn at a
/// quarter. One warm still pins the second bundled theme.
#[test]
fn every_starter_has_its_golden_sheet() {
    let size = Size {
        width: 1920,
        height: 1080,
    };
    let options = GalleryOptions {
        scale: 0.25,
        threads: None,
    };
    let theme = Theme::bundled("default").unwrap();
    for name in Starter.names() {
        let mut stills = Vec::new();
        for t in MOMENTS {
            let moment = Moment {
                t,
                duration: gallery::PREVIEW,
            };
            let g = gallery::render_source(&Starter, &theme, size, None, &name, moment, &options)
                .unwrap();
            assert!(!g.stills.is_empty(), "{name} has no examples");
            stills.push(g.stills);
        }
        // Example-major: row i is example i at each moment.
        let n = stills[0].len();
        let mut columns: Vec<_> = stills.into_iter().map(Vec::into_iter).collect();
        let mut sheet = Vec::new();
        for _ in 0..n {
            for (col, t) in columns.iter_mut().zip(MOMENTS) {
                let s = col.next().unwrap();
                sheet.push(Still {
                    example: format!("{} · {t} s", s.example),
                    frame: s.frame,
                });
            }
        }
        let g = Gallery {
            component: name.clone(),
            moment: Moment {
                t: 0.0,
                duration: gallery::PREVIEW,
            },
            stills: sheet,
            threads: 1,
        };
        assert_golden(&format!("starter-{name}"), &sheet_frame(&g, MOMENTS.len()));
    }
    let lib = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("library/themes/warm.json"),
    )
    .unwrap();
    let warm = Theme::parse(&lib, std::path::Path::new("library/themes/warm.json")).unwrap();
    let moment = Moment {
        t: 1.0,
        duration: 2.0,
    };
    let half = GalleryOptions {
        scale: 0.5,
        threads: None,
    };
    let g =
        gallery::render_source(&Starter, &warm, size, None, "TitleCard", moment, &half).unwrap();
    let long = g.stills.iter().find(|s| s.example == "long").unwrap();
    assert_golden("starter-TitleCard-long-warm", &long.frame);
}

/// Seconds into the preview each starter sheet shows.
const MOMENTS: [f64; 5] = [0.6, 1.6, 2.6, 3.8, 5.6];

/// The gallery's own sheet (labels and all), read back as a frame.
fn sheet_frame(g: &Gallery, cols: usize) -> Frame {
    let path = std::env::temp_dir().join(format!(
        "starter-sheet-{}-{}.png",
        g.component,
        std::process::id()
    ));
    g.write_sheet(&path, cols).unwrap();
    let img = image::open(&path).unwrap().to_rgba8();
    std::fs::remove_file(&path).ok();
    Frame {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
        logs: Vec::new(),
        samples: 1,
    }
}
