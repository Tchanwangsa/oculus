//! Banding in a dark gradient at 8 bits: a full-frame `#0e0f13` to
//! `#1c1e26` ramp at 1080p, measured along its axis, against the same ramp
//! rounded to 8 bits without dithering.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::render;
use studio_core::Time;

const FROM: [f64; 3] = [14.0, 15.0, 19.0];
const TO: [f64; 3] = [28.0, 30.0, 38.0];

struct Bands {
    /// Distinct 8-bit levels per channel along one row.
    levels: [usize; 3],
    /// The widest run of identical pixels along one row, in px.
    widest: usize,
    /// The largest step between neighbours in any channel along one row.
    step: u8,
    /// The widest run of identical 8x8 means, in px: what the eye sees.
    widest_mean: usize,
}

/// Pixel (x, y) as RGB.
type Image<'a> = &'a dyn Fn(usize, usize) -> [u8; 3];

fn widest_run<T: PartialEq>(v: &[T]) -> usize {
    let (mut widest, mut run) = (1, 1);
    for w in v.windows(2) {
        run = if w[0] == w[1] { run + 1 } else { 1 };
        widest = widest.max(run);
    }
    widest
}

fn measure(px: Image, width: usize, y: usize) -> Bands {
    let row: Vec<[u8; 3]> = (0..width).map(|x| px(x, y)).collect();
    let mut levels = [0; 3];
    for (c, l) in levels.iter_mut().enumerate() {
        let mut seen: Vec<u8> = row.iter().map(|p| p[c]).collect();
        seen.sort_unstable();
        seen.dedup();
        *l = seen.len();
    }
    let step = row
        .windows(2)
        .flat_map(|w| (0..3).map(move |c| w[0][c].abs_diff(w[1][c])))
        .max()
        .unwrap_or(0);
    // Sums over the 8x8 box at each x (one Bayer period each way).
    let means: Vec<[u32; 3]> = (0..width - 8)
        .map(|x| {
            let mut s = [0u32; 3];
            for dy in 0..8 {
                for dx in 0..8 {
                    let p = px(x + dx, y - 4 + dy);
                    for c in 0..3 {
                        s[c] += u32::from(p[c]);
                    }
                }
            }
            s
        })
        .collect();
    Bands {
        levels,
        widest: widest_run(&row),
        step,
        widest_mean: widest_run(&means),
    }
}

fn print(what: &str, b: &Bands) {
    println!(
        "{what}: levels r/g/b {:?}, widest run {} px, largest step {}, widest run of 8x8 means {} px",
        b.levels, b.widest, b.step, b.widest_mean
    );
}

#[test]
fn dark_gradient_does_not_band() {
    let f = render("dark-gradient", Time::Global(0.0));
    let (w, h) = (f.width as usize, f.height as usize);
    let frame = |x: usize, y: usize| {
        let i = (y * w + x) * 4;
        [f.rgba[i], f.rgba[i + 1], f.rgba[i + 2]]
    };
    // The ideal ramp rounded to 8 bits: what an undithered renderer draws.
    let rounded = |x: usize, _y: usize| {
        let s = (x as f64 + 0.5) / w as f64;
        [0, 1, 2].map(|c| (FROM[c] + (TO[c] - FROM[c]) * s).round() as u8)
    };
    let ours = measure(&frame, w, h / 2);
    let plain = measure(&rounded, w, h / 2);
    print("dark gradient, undithered", &plain);
    print("dark gradient, dithered", &ours);
    assert!(plain.widest_mean > 60, "the reference should band");
    assert!(
        ours.widest_mean <= 8,
        "visible bands: {} px",
        ours.widest_mean
    );
    assert!(ours.step <= 1, "a step of {} levels", ours.step);
    assert_eq!(ours.levels, plain.levels, "the same range of levels");
    // Every column averages to the ideal ramp within a fraction of a level.
    for x in (0..w).step_by(37) {
        let s = (x as f64 + 0.5) / w as f64;
        for c in 0..3 {
            let mean: f64 = (0..8)
                .map(|dy| f64::from(frame(x, 536 + dy)[c]))
                .sum::<f64>()
                / 8.0;
            let ideal = FROM[c] + (TO[c] - FROM[c]) * s;
            assert!(
                (mean - ideal).abs() < 0.75,
                "x {x} channel {c}: {mean} vs {ideal}"
            );
        }
    }
}
