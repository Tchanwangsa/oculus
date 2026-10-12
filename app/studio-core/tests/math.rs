//! `<Math>`: a corpus of KaTeX constructs as a golden frame, and a Morph
//! between two formulas sharing a key as a golden contact sheet.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::{assert_golden, fixture, render};
use studio_core::{Frame, Project, StripOptions, Time, strip};

/// Fractions (nested), powers and subscripts, roots, `\gcd`, `\bmod`,
/// `\pmod`, `\left( \right)`, `\sum` limits, a matrix, colour, `\k`
/// groups, an overline, an accent and a brace, each beside a text label on
/// its baseline.
#[test]
fn corpus() {
    assert_golden(
        "math-corpus",
        &render("math", Time::Scene { index: 0, t: 0.0 }),
    );
}

/// `2^4 - 1` into `(2^2 - 1)(2^2 + 1)`: the 2, the minus and the 1 glide
/// into the first factor, the 4 fades out by p = 0.4 and the new terms
/// fade in after it.
#[test]
fn factor_morph_strip() {
    let project = Project::load(fixture("math")).unwrap();
    let options = StripOptions {
        from: 1.0,
        to: 2.0,
        n: 5,
        cols: 5,
        scale: 0.3,
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
    assert_golden("math-factor-strip", &frame);
}
