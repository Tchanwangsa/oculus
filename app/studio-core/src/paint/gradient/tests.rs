use super::ramp;
use crate::scene::Color;
use vello_cpu::kurbo::{Affine, Rect};

fn grey(v: f32) -> Color {
    Color {
        r: v,
        g: v,
        b: v,
        a: 1.0,
    }
}

/// The red channel of each pixel of a ramp over `shape`, row-major.
fn reds(stops: &[(f32, Color)], shape: Rect) -> (Vec<u8>, u16) {
    let r = Rect::new(0.0, 0.0, 64.0, 16.0);
    let (img, _) = ramp(Affine::IDENTITY, r, 90.0, stops, 1.0, shape, (64, 16)).unwrap();
    (img.data().iter().map(|p| p.r).collect(), img.width())
}

#[test]
fn a_level_between_two_is_dithered_half_and_half() {
    // 20.5 / 255: each 8x8 tile is 32 pixels of 20 and 32 of 21.
    let c = grey(20.5 / 255.0);
    let (px, w) = reds(&[(0.0, c), (1.0, c)], Rect::new(0.0, 0.0, 64.0, 16.0));
    assert_eq!(w, 64);
    let tile: Vec<u8> = (0..64).map(|k| px[(k / 8) * 64 + k % 8]).collect();
    assert_eq!(tile.iter().filter(|&&v| v == 20).count(), 32);
    assert_eq!(tile.iter().filter(|&&v| v == 21).count(), 32);
}

#[test]
fn whole_levels_are_not_dithered() {
    let c = grey(20.0 / 255.0);
    let (px, _) = reds(&[(0.0, c), (1.0, c)], Rect::new(0.0, 0.0, 64.0, 16.0));
    assert!(px.iter().all(|&v| v == 20));
}

#[test]
fn the_pattern_is_fixed_to_the_canvas() {
    // The same device pixel gets the same value whatever the shape's bounds.
    let stops = [(0.0, grey(0.05)), (1.0, grey(0.1))];
    let (all, w) = reds(&stops, Rect::new(0.0, 0.0, 64.0, 16.0));
    let (part, pw) = reds(&stops, Rect::new(13.0, 3.0, 40.0, 16.0));
    assert_eq!(pw, 27);
    for y in 3..16usize {
        for x in 13..40usize {
            assert_eq!(part[(y - 3) * 27 + x - 13], all[y * w as usize + x]);
        }
    }
}

#[test]
fn a_ramp_off_the_canvas_draws_nothing() {
    let r = Rect::new(100.0, 0.0, 164.0, 16.0);
    let stops = [(0.0, grey(0.0)), (1.0, grey(1.0))];
    assert!(ramp(Affine::IDENTITY, r, 90.0, &stops, 1.0, r, (64, 16)).is_none());
}

/// Reference: the exact offset and colour per pixel, in f64.
fn reference(stops: &[(f32, Color)], angle: f32, w: u16, h: u16) -> Vec<u8> {
    let r = Rect::new(0.0, 0.0, f64::from(w), f64::from(h));
    let (start, d) = super::line(r, angle);
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let p = vello_cpu::kurbo::Point::new(f64::from(x) + 0.5, f64::from(y) + 0.5);
            let s = (p - start).dot(d) / d.hypot2();
            let v = |c: &Color| f64::from(c.r) * 255.0;
            let first = stops[0];
            let last = stops[stops.len() - 1];
            let mut c = if s <= f64::from(first.0) {
                v(&first.1)
            } else {
                v(&last.1)
            };
            for pair in stops.windows(2) {
                let (o0, o1) = (f64::from(pair[0].0), f64::from(pair[1].0));
                if s > o0 && s <= o1 && o1 > o0 {
                    c = v(&pair[0].1) + (v(&pair[1].1) - v(&pair[0].1)) * (s - o0) / (o1 - o0);
                }
            }
            let b = f64::from(super::BAYER[usize::from(y & 7)][usize::from(x & 7)]);
            out.push((c + (b + 0.5) / 64.0).floor().clamp(0.0, 255.0) as u8);
        }
    }
    out
}

#[test]
fn rows_match_the_exact_ramp() {
    let stops = [
        (0.0, grey(0.1)),
        (0.3, grey(0.6)),
        (0.3, grey(0.2)),
        (0.8, grey(0.9)),
        (1.0, grey(0.4)),
    ];
    for angle in [90.0, 270.0, 120.0, 200.0, 0.0] {
        let (w, h) = (96u16, 40u16);
        let r = Rect::new(0.0, 0.0, f64::from(w), f64::from(h));
        let (img, _) = ramp(Affine::IDENTITY, r, angle, &stops, 1.0, r, (w, h)).unwrap();
        let got: Vec<u8> = img.data().iter().map(|p| p.r).collect();
        let want = reference(&stops, angle, w, h);
        let off = got.iter().zip(&want).filter(|(a, b)| a != b).count();
        let worst = got.iter().zip(&want).map(|(a, b)| a.abs_diff(*b)).max();
        // f32 stepping may flip a pixel sitting exactly on a threshold.
        assert!(worst <= Some(1), "{angle}: worst {worst:?}");
        assert!(
            off * 100 < got.len(),
            "{angle}: {off} of {} differ",
            got.len()
        );
    }
}
