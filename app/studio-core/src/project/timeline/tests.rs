use super::*;

fn timeline() -> Timeline {
    Timeline::new([4.0, 2.5, 1.0].into_iter())
}

#[test]
fn global_time_finds_the_scene() {
    let tl = timeline();
    assert_eq!(tl.locate(Time::Global(0.0)).unwrap(), (0, 0.0));
    assert_eq!(tl.locate(Time::Global(4.0)).unwrap(), (1, 0.0));
    assert_eq!(tl.locate(Time::Global(6.75)).unwrap(), (2, 0.25));
    assert_eq!(tl.start(2), 6.5);
    assert_eq!(tl.scene_length(1), Some(2.5));
    assert_eq!(tl.scene_length(3), None);
}

#[test]
fn out_of_range_states_the_length() {
    let tl = timeline();
    let e = tl.locate(Time::Global(7.5)).unwrap_err().to_string();
    assert!(e.contains("7.500 s long"), "{e}");
    assert!(tl.locate(Time::Global(-0.1)).is_err());
    assert!(tl.locate(Time::Global(f64::NAN)).is_err());
    let e = tl
        .locate(Time::Scene { index: 3, t: 0.0 })
        .unwrap_err()
        .to_string();
    assert!(e.contains("has 3 scenes"), "{e}");
    let e = tl
        .locate(Time::Scene { index: 1, t: 2.5 })
        .unwrap_err()
        .to_string();
    assert!(e.contains("which lasts 2.500 s"), "{e}");
}
