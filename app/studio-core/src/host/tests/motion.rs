//! Prelude motion helpers against reference numbers: the closed-form spring
//! against a fine RK4 integration, cubicBezier against bisection, and the
//! counter, reveal and timing helpers.

use super::{call, host};

/// Evaluates a JS expression list in a component and returns the numbers.
fn numbers(js: &str) -> Vec<f64> {
    let mut h = host();
    let tsx = format!(
        "export const props = {{}};\nexport default () => <Text>{{JSON.stringify([{js}])}}</Text>;\n"
    );
    h.load("Nums", &tsx).unwrap();
    let (tree, _) = call(&mut h, "Nums").unwrap();
    serde_json::from_str(tree["text"].as_str().unwrap()).unwrap()
}

/// x'' = (k (1 - x) - c x') / m from rest at 0, RK4 with a 10 µs step.
fn rk4_spring(t: f64, k: f64, c: f64, m: f64) -> f64 {
    let f = |x: f64, v: f64| (k * (1.0 - x) - c * v) / m;
    let (mut x, mut v) = (0.0, 0.0);
    let n = (t / 1e-5).round() as usize;
    let dt = t / n as f64;
    for _ in 0..n {
        let (k1x, k1v) = (v, f(x, v));
        let (k2x, k2v) = (
            v + dt / 2.0 * k1v,
            f(x + dt / 2.0 * k1x, v + dt / 2.0 * k1v),
        );
        let (k3x, k3v) = (
            v + dt / 2.0 * k2v,
            f(x + dt / 2.0 * k2x, v + dt / 2.0 * k2v),
        );
        let (k4x, k4v) = (v + dt * k3v, f(x + dt * k3x, v + dt * k3v));
        x += dt / 6.0 * (k1x + 2.0 * k2x + 2.0 * k3x + k4x);
        v += dt / 6.0 * (k1v + 2.0 * k2v + 2.0 * k3v + k4v);
    }
    x
}

#[test]
fn spring_matches_an_integrated_spring() {
    // Under-, critically and over-damped.
    let presets = [(180.0, 12.0, 1.0), (100.0, 20.0, 1.0), (120.0, 40.0, 2.0)];
    let ts = [0.05, 0.2, 0.5, 1.3];
    for (k, c, m) in presets {
        let js: Vec<String> = ts
            .iter()
            .map(|t| format!("spring({t}, {{ stiffness: {k}, damping: {c}, mass: {m} }})"))
            .collect();
        let got = numbers(&js.join(", "));
        for (t, g) in ts.iter().zip(got) {
            let want = rk4_spring(*t, k, c, m);
            assert!(
                (g - want).abs() < 1e-9,
                "spring k={k} c={c} m={m} t={t}: {g} vs {want}"
            );
        }
    }
}

#[test]
fn cubic_bezier_matches_bisection() {
    let (x1, y1, x2, y2) = (0.05, 0.7, 0.1, 1.0);
    let bez = |a: f64, b: f64, s: f64| {
        3.0 * a * s * (1.0 - s).powi(2) + 3.0 * b * s * s * (1.0 - s) + s.powi(3)
    };
    let ps = [0.1, 0.3, 0.5, 0.8, 0.95];
    let js: Vec<String> = ps
        .iter()
        .map(|p| format!("cubicBezier({x1}, {y1}, {x2}, {y2})({p})"))
        .collect();
    for (p, g) in ps.iter().zip(numbers(&js.join(", "))) {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..200 {
            let mid = (lo + hi) / 2.0;
            if bez(x1, x2, mid) < *p {
                lo = mid
            } else {
                hi = mid
            }
        }
        let want = bez(y1, y2, (lo + hi) / 2.0);
        assert!((g - want).abs() < 1e-6, "p={p}: {g} vs {want}");
    }
    let eased = numbers("easeOutCubic(0.5), easeInOutCubic(0.25), easeInOutCubic(0.75)");
    assert_eq!(eased, [0.875, 0.0625, 0.9375]);
}

#[test]
fn counters_reveals_and_start_times() {
    let mut h = host();
    h.load(
        "Count",
        "export const props = {};\nexport default ({}, { t }) => <Text>{[counter(t, 0, 200, { dur: 1, easing: linear }), counter(t, 0, 1, { at: 0.25, dur: 0.5, decimals: 2 }), counter(t, 3, 0, { at: 1 }), revealProgress(t, 0.25, 1)].join(' ')}</Text>;\n",
    )
    .unwrap();
    let (tree, _) = call(&mut h, "Count").unwrap();
    assert_eq!(tree["text"], "100 0.88 3 0.25");
    let starts =
        numbers("...series([1, 2, 0.5], { gap: 0.25, at: 1 }), ...parallel([1, 2], { at: 3 })");
    assert_eq!(starts, [1.0, 2.25, 4.5, 3.0, 3.0]);
}
