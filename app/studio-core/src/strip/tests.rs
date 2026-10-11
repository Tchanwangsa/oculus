use super::{StripOptions, seconds, times};

fn opts(from: f64, to: f64, n: usize) -> StripOptions {
    StripOptions {
        from,
        to,
        n,
        ..StripOptions::default()
    }
}

#[test]
fn times_are_evenly_spaced_and_inclusive() {
    let t = times(&opts(1.0, 3.0, 5), 10.0).unwrap();
    assert_eq!(t, [1.0, 1.5, 2.0, 2.5, 3.0]);
}

#[test]
fn one_frame_is_from() {
    assert_eq!(times(&opts(2.5, 8.0, 1), 10.0).unwrap(), [2.5]);
}

#[test]
fn the_end_is_clamped_inside_the_video() {
    for to in [10.0, 12.0, f64::INFINITY] {
        let t = times(&opts(0.0, to, 3), 10.0).unwrap();
        assert!(t[2] < 10.0 && t[2] > 10.0 - 1e-5, "{to}: {t:?}");
        assert_eq!(t[0], 0.0);
    }
    // It reads as just inside the end, never as the end itself.
    assert_eq!(seconds(10.0 - 1e-6, 10.0), "9.999");
    assert_eq!(seconds(1.25, 10.0), "1.250");
    assert_eq!(seconds(0.1 + 0.2, 10.0), "0.300");
}

#[test]
fn bad_ranges_are_errors() {
    assert!(times(&opts(0.0, 1.0, 0), 10.0).is_err());
    assert!(times(&opts(10.0, 12.0, 3), 10.0).is_err());
    assert!(times(&opts(-1.0, 2.0, 3), 10.0).is_err());
    assert!(times(&opts(4.0, 2.0, 3), 10.0).is_err());
    assert!(times(&opts(1.0, f64::NAN, 3), 10.0).is_err());
}
