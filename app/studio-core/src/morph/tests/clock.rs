//! Lag windows, fade windows and the overlap fade's riding.

use super::{bx, keyed, leaf};
use crate::layout::{LNode, Typeset};
use crate::morph::clock::local;
use crate::morph::fade::{arriving, leaving};
use crate::morph::{Ink, MorphColor, morph};
use crate::scene::{Color, Curve, Fade, Timing};

const INK: Ink = Ink {
    text: Color::BLACK,
    colour: MorphColor::Oklch,
};

fn lagged(lag: f32) -> Timing {
    Timing {
        lag,
        ..Timing::default()
    }
}

#[test]
fn lag_starts_each_window_a_fraction_of_a_piece_after_the_last() {
    // Three pieces at lag 0.5 run over 2 piece lengths: piece i runs over
    // p in [i / 4, i / 4 + 1 / 2].
    let t = lagged(0.5);
    let at = |p: f32, i: u32| local(p, &t, 3, Some((i, i)));
    assert_eq!(at(0.0, 0), 0.0);
    assert_eq!(at(0.25, 0), 0.5);
    assert_eq!(at(0.25, 1), 0.0);
    assert_eq!(at(0.5, 0), 1.0);
    assert_eq!(at(0.5, 1), 0.5);
    assert_eq!(at(0.5, 2), 0.0);
    assert_eq!(at(1.0, 2), 1.0);
    // A window over pieces 0..=2 spans the whole run.
    assert_eq!(local(0.5, &t, 3, Some((0, 2))), 0.5);
    // No lag: every window is p itself, overshoot included.
    assert_eq!(local(1.1, &Timing::default(), 3, Some((2, 2))), 1.1);
    // With lag an overshooting p carries every window the same way past 1.
    assert!((at(1.1, 0) - 1.1).abs() < 1e-6);
}

#[test]
fn the_ease_applies_to_each_window() {
    let t = Timing {
        lag: 0.5,
        ease: Curve::Table([0.0, 0.25, 1.0].into()),
        ..Timing::default()
    };
    // Piece 0 at p 0.125 is a quarter through: table at 0.25 is 0.125.
    assert!((local(0.125, &t, 3, Some((0, 0))) - 0.125).abs() < 1e-6);
}

#[test]
fn fades_by_mode() {
    let s = Fade::Sequential;
    assert_eq!(leaving(s, 0.0), 1.0);
    assert_eq!(leaving(s, 0.4), 0.0);
    assert_eq!(arriving(s, 0.4), 0.0);
    assert_eq!(arriving(s, 1.0), 1.0);
    let o = Fade::Overlap;
    assert_eq!(leaving(o, 0.6), 0.0);
    assert_eq!(arriving(o, 0.4), 0.0);
    // Both are partly visible together in the middle.
    assert!(leaving(o, 0.5) > 0.0 && arriving(o, 0.5) > 0.0);
}

fn run(a: LNode, b: LNode, p: f32, t: &Timing) -> LNode {
    morph(a, b, p, t, &mut Typeset::new(), INK).unwrap()
}

fn row(keys: &[(&str, f64)]) -> LNode {
    bx(
        0.0,
        0.0,
        400.0,
        100.0,
        keys.iter().map(|(k, x)| keyed(k, leaf(*x, 0.0))).collect(),
    )
}

fn x_of(n: &LNode, key: &str) -> f64 {
    n.children
        .iter()
        .find(|c| c.key.as_deref() == Some(key))
        .unwrap()
        .rect
        .x0
}

#[test]
fn with_lag_matched_pieces_move_in_reading_order() {
    let a = || row(&[("a", 0.0), ("b", 20.0), ("c", 40.0)]);
    let b = || row(&[("a", 100.0), ("b", 120.0), ("c", 140.0)]);
    let out = run(a(), b(), 0.5, &lagged(1.0));
    // lag 1: one after another, each a third of the run.
    assert_eq!(x_of(&out, "a"), 100.0);
    assert!((x_of(&out, "b") - 70.0).abs() < 1e-3);
    assert_eq!(x_of(&out, "c"), 40.0);
}

#[test]
fn an_arriving_piece_rides_beside_its_matched_neighbour() {
    // `n` arrives right of `a`, which moves 100 px; with the overlap fade
    // it keeps its B offset from `a` the whole way; sequential waits in
    // place.
    let a = || row(&[("a", 0.0)]);
    let b = || row(&[("a", 100.0), ("n", 120.0)]);
    let overlap = run(a(), b(), 0.7, &Timing::default());
    assert!((x_of(&overlap, "n") - x_of(&overlap, "a") - 20.0).abs() < 1e-6);
    let still = Timing {
        fade: Fade::Sequential,
        ..Timing::default()
    };
    let seq = run(a(), b(), 0.7, &still);
    assert_eq!(x_of(&seq, "n"), 120.0);
}

#[test]
fn a_leaving_piece_rides_out_with_its_matched_neighbour() {
    let a = || row(&[("a", 0.0), ("old", 20.0)]);
    let b = || row(&[("a", 100.0)]);
    let out = run(a(), b(), 0.3, &Timing::default());
    assert!((x_of(&out, "old") - x_of(&out, "a") - 20.0).abs() < 1e-6);
}
