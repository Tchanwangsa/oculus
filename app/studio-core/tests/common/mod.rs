//! Shared test helpers: fixture paths and golden-image comparison.
#![allow(dead_code)] // each test crate uses a subset

use std::path::{Path, PathBuf};
use studio_core::{Frame, Project, RenderOptions, Renderer, Time};

/// Whether a lint message is one of the component-source rules (token use,
/// required props). Renderer fixtures and the inline test components use
/// literal colours and numbers on purpose, so tests about other lint rules
/// filter these out.
pub fn component_rule(message: &str) -> bool {
    message.contains("is a literal; use theme.")
        || message.contains("equals theme.")
        || message.contains("has required props")
        || message.contains("omits required prop")
}

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub fn render(name: &str, at: Time) -> Frame {
    let project = Project::load(fixture(name)).unwrap();
    let mut r = Renderer::new(&project, RenderOptions::default()).unwrap();
    r.frame(at).unwrap()
}

/// Compares against `tests/golden/<name>.png`: a pixel differs when any
/// channel is off by more than 2, and at most 0.1% may differ. On failure
/// the actual frame and a diff image go to `target/golden-failures/`.
/// `STUDIO_BLESS=1` rewrites the golden instead.
pub fn assert_golden(name: &str, frame: &Frame) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let golden = root.join("tests/golden").join(format!("{name}.png"));
    if std::env::var_os("STUDIO_BLESS").is_some() {
        frame.write_png(&golden).unwrap();
        return;
    }
    let expected = image::open(&golden)
        .unwrap_or_else(|e| {
            panic!(
                "{}: {e}; run with STUDIO_BLESS=1 to create it",
                golden.display()
            )
        })
        .to_rgba8();
    assert_eq!(
        (expected.width(), expected.height()),
        (frame.width, frame.height),
        "{name}: size differs from the golden"
    );
    let mut differing = 0usize;
    let mut worst = 0u8;
    let mut diff = vec![0u8; frame.rgba.len()];
    for (i, (a, b)) in frame
        .rgba
        .chunks_exact(4)
        .zip(expected.as_raw().chunks_exact(4))
        .enumerate()
    {
        let d = a
            .iter()
            .zip(b)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        worst = worst.max(d);
        if d > 2 {
            differing += 1;
        }
        let v = if d > 2 { 255 } else { d.saturating_mul(60) };
        diff[i * 4..i * 4 + 4].copy_from_slice(&[v, 0, 0, 255]);
    }
    let pixels = (frame.width * frame.height) as usize;
    if differing * 1000 > pixels {
        let out = root.join("target/golden-failures");
        std::fs::create_dir_all(&out).unwrap();
        frame
            .write_png(out.join(format!("{name}.actual.png")))
            .unwrap();
        image::save_buffer(
            out.join(format!("{name}.diff.png")),
            &diff,
            frame.width,
            frame.height,
            image::ColorType::Rgba8,
        )
        .unwrap();
        panic!(
            "{name}: {differing} of {pixels} pixels differ (worst channel diff {worst}); see {}",
            out.display()
        );
    }
}
