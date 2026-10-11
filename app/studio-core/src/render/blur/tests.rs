use super::MotionBlur;
use super::average::Accumulator;

#[test]
fn samples_grow_with_the_move_and_stop_at_the_cap() {
    let mb = MotionBlur::at(30);
    assert_eq!(mb.samples(0.0), 1);
    assert_eq!(mb.samples(1.0), 1);
    assert_eq!(mb.samples(1.1), 2);
    assert_eq!(mb.samples(10.0), 10);
    assert_eq!(mb.samples(1e6), 32);
    assert_eq!(mb.samples(f64::NAN), 1);
}

#[test]
fn bad_shutters_are_refused() {
    let ok = MotionBlur::at(30);
    assert!(ok.check().is_ok());
    for bad in [
        MotionBlur { fps: 0.0, ..ok },
        MotionBlur { shutter: 0.0, ..ok },
        MotionBlur { shutter: 1.5, ..ok },
        MotionBlur {
            max_samples: 0,
            ..ok
        },
    ] {
        assert!(bad.check().is_err(), "{bad:?}");
    }
}

#[test]
fn averages_premultiplied_and_exactly() {
    let mut acc = Accumulator::new(3, 1);
    // An opaque pixel, one half covered, one never covered.
    acc.add(&[10, 20, 30, 255, 200, 0, 0, 255, 0, 0, 0, 0]);
    acc.add(&[20, 40, 60, 255, 0, 0, 255, 0, 0, 0, 0, 0]);
    let f = acc.finish(Vec::new(), 2);
    // A transparent sample adds no colour: the red stays red, at half alpha.
    assert_eq!(f.rgba, [15, 30, 45, 255, 200, 0, 0, 128, 0, 0, 0, 0]);
    assert_eq!(f.samples, 2);
}

#[test]
fn one_sample_is_the_input() {
    let px = [1, 2, 3, 255, 9, 8, 7, 100];
    let mut acc = Accumulator::new(2, 1);
    acc.add(&px);
    assert_eq!(acc.finish(Vec::new(), 1).rgba, px);
}
