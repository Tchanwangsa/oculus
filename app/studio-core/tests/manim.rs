//! manim's motion vocabulary as golden contact sheets: a derivation
//! rewritten in place by `Steps` with lag, `Write` cued per `\k` group,
//! overlays bound to keys following a morph, a formula shrinking to the
//! corner, and a number flying into a symbol's slot.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::{assert_golden, fixture};
use studio_core::{Frame, Project, StripOptions, strip};

fn sheet(name: &str, from: f64, to: f64, n: usize) {
    let project = Project::load(fixture("manim")).unwrap();
    let options = StripOptions {
        from,
        to,
        n,
        cols: 4,
        scale: 0.2,
        threads: Some(2),
        ..StripOptions::default()
    };
    let s = strip(&project, options).unwrap();
    let frame = Frame {
        width: s.width,
        height: s.height,
        rgba: s.rgba,
        logs: Vec::new(),
        samples: 1,
    };
    assert_golden(name, &frame);
}

/// Four states at 0.5, 1.75 and 3 s, lag 0.1: shared terms glide in
/// reading order and new ones fade in beside their neighbours.
#[test]
fn derivation_rewrites_in_place() {
    sheet("manim-derive-strip", 0.4, 4.2, 12);
}

/// `2^4`, `-`, `1` and the fraction each written on their own cue.
#[test]
fn write_cued_per_group() {
    sheet("manim-write-strip", 5.0, 7.9, 8);
}

/// Brace, Surround, Underline, Indicate and a keyed Arrow, through a Steps
/// morph that fades the Surround's and the Arrow's targets out.
#[test]
fn overlays_follow_their_targets() {
    sheet("manim-overlay-strip", 8.0, 11.9, 8);
}

/// The same Math key re-laid out small at the top left: glyphs scale as
/// they glide, and the next formula fades in.
#[test]
fn shrink_to_corner() {
    sheet("manim-corner-strip", 12.3, 14.0, 8);
}

/// The 3 in `r = 3` flies into r's slot in `A = π · 3²`.
#[test]
fn substitution() {
    sheet("manim-substitute-strip", 15.3, 16.5, 8);
}
