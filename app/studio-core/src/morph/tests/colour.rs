use crate::morph::colour::{MorphColor, chroma, from_oklab, mix, to_oklab};
use crate::scene::Color;

#[test]
fn oklab_round_trips_within_one_step() {
    for hex in [
        "#000000", "#ffffff", "#5e6ad2", "#f08c00", "#16161d", "#2f9e6e", "#d1495b", "#808080",
    ] {
        let c = Color::parse(hex).unwrap();
        let back = from_oklab(to_oklab(c), c.a);
        for (x, y) in [(c.r, back.r), (c.g, back.g), (c.b, back.b)] {
            assert!((x - y).abs() < 1.0 / 255.0, "{hex}: {c:?} -> {back:?}");
        }
    }
}

#[test]
fn the_midpoint_of_indigo_and_orange_keeps_its_chroma() {
    let (indigo, orange) = (
        Color::parse("#5e6ad2").unwrap(),
        Color::parse("#f08c00").unwrap(),
    );
    let mid = mix(indigo, orange, 0.5, MorphColor::Oklch);
    let floor = chroma(indigo).min(chroma(orange));
    assert!(chroma(mid) > floor, "{} vs {floor}", chroma(mid));
    // A straight sRGB blend is the muddy one.
    let srgb = Color {
        r: (indigo.r + orange.r) / 2.0,
        g: (indigo.g + orange.g) / 2.0,
        b: (indigo.b + orange.b) / 2.0,
        a: 1.0,
    };
    assert!(chroma(srgb) < floor, "{}", chroma(srgb));
}

#[test]
fn mixing_is_exact_at_the_ends_and_a_clear_end_keeps_the_hue() {
    let (a, b) = (
        Color::parse("#5e6ad2").unwrap(),
        Color::parse("#f08c00").unwrap(),
    );
    assert_eq!(mix(a, b, 0.0, MorphColor::Oklch), a);
    assert_eq!(mix(a, b, 1.0, MorphColor::Oklch), b);
    let half = mix(
        Color {
            a: 0.0,
            ..Color::BLACK
        },
        b,
        0.5,
        MorphColor::Oklch,
    );
    assert!((half.a - 0.5).abs() < 1e-6);
    assert!(
        (half.r - b.r).abs() < 1e-3 && (half.g - b.g).abs() < 1e-3,
        "{half:?}"
    );
}

#[test]
fn oklab_is_the_straight_line_and_oklch_keeps_the_chroma() {
    let (indigo, orange) = (
        Color::parse("#5e6ad2").unwrap(),
        Color::parse("#f08c00").unwrap(),
    );
    let (la, lb) = (to_oklab(indigo), to_oklab(orange));
    let want = from_oklab([0, 1, 2].map(|i| (la[i] + lb[i]) / 2.0), 1.0);
    let got = mix(indigo, orange, 0.5, MorphColor::Oklab);
    for (x, y) in [(got.r, want.r), (got.g, want.g), (got.b, want.b)] {
        assert!((x - y).abs() < 1e-6, "{got:?} vs {want:?}");
    }
    // The straight line nearly crosses grey here; the polar path does not.
    assert!(chroma(got) < 0.06, "{}", chroma(got));
    assert!(chroma(mix(indigo, orange, 0.5, MorphColor::Oklch)) > 0.15);
    // With a near-grey end both paths are the same straight line.
    let grey = Color::parse("#f4f4f6").unwrap();
    assert_eq!(
        mix(grey, orange, 0.3, MorphColor::Oklch),
        mix(grey, orange, 0.3, MorphColor::Oklab)
    );
}
