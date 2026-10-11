use super::*;

#[test]
fn easings_end_at_zero_and_one_and_rise() {
    for e in [
        Easing::Linear,
        Easing::OutCubic,
        Easing::InOutCubic,
        Easing::Bezier([0.2, 0.0, 0.0, 1.0]),
    ] {
        assert_eq!(e.apply(0.0), 0.0, "{e:?}");
        assert!((e.apply(1.0) - 1.0).abs() < 1e-12, "{e:?}");
        let mut last = 0.0;
        for i in 1..=100 {
            let v = e.apply(f64::from(i) / 100.0);
            assert!(v >= last - 1e-12, "{e:?} falls at {i}");
            last = v;
        }
    }
    assert!((Easing::InOutCubic.apply(0.5) - 0.5).abs() < 1e-12);
    assert!((in_out_cubic(0.25) - 0.0625).abs() < 1e-7);
}

#[test]
fn cubic_bezier_solves_x_then_reads_y() {
    // ease (0.25, 0.1, 0.25, 1) at x = 0.5, by bisection to 1e-12.
    let (x1, y1, x2, y2) = (0.25, 0.1, 0.25, 1.0);
    let bx = |s: f64| 3.0 * x1 * s * (1.0 - s).powi(2) + 3.0 * x2 * s * s * (1.0 - s) + s.powi(3);
    let by = |s: f64| 3.0 * y1 * s * (1.0 - s).powi(2) + 3.0 * y2 * s * s * (1.0 - s) + s.powi(3);
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if bx(mid) < 0.5 { lo = mid } else { hi = mid }
    }
    let want = by((lo + hi) / 2.0);
    assert!((cubic_bezier(x1, y1, x2, y2, 0.5) - want).abs() < 1e-6);
}
