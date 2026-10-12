use super::*;

fn timeline() -> Timeline {
    Timeline::new(30, [4.0, 2.5, 1.0].into_iter())
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

#[test]
fn lengths_round_up_to_whole_frames() {
    assert_eq!(
        whole_frames(0.1, 30),
        3,
        "0.1 * 30 is 3, not 3.0000000000000004"
    );
    assert_eq!(whole_frames(3.85, 30), 116);
    assert_eq!(whole_frames(2.0, 24), 48);
    assert_eq!(whole_frames(1.0 / 60.0, 25), 1);
    assert_eq!(whole_frames(0.0, 30), 1, "every scene has a frame");
    let tl = Timeline::new(25, [1.0, 0.33, 2.0].into_iter());
    assert_eq!(tl.frames(), 25 + 9 + 50);
    assert_eq!(tl.scene_frames(1).unwrap(), 25..34);
    assert_eq!(tl.scene_length(1), Some(9.0 / 25.0));
    assert_eq!(tl.length(), 84.0 / 25.0);
    assert_eq!(tl.start(2), 34.0 / 25.0);
    assert_eq!(tl.start(9), tl.length());
}

#[test]
fn a_global_frame_time_is_the_frame() {
    for fps in [24, 25, 30, 50, 60] {
        let tl = Timeline::new(fps, [1.3, 0.1, 2.0 / 3.0, 0.7].into_iter());
        for k in 0..tl.frames() {
            let Time::Scene { index, t } = tl.frame_time(k).unwrap() else {
                panic!("frame_time gives a scene time");
            };
            let range = tl.scene_frames(index).unwrap();
            assert!(range.contains(&k), "fps {fps} frame {k}");
            assert_eq!(t, (k - range.start) as f64 / f64::from(fps));
            assert!(t < tl.scene_length(index).unwrap());
            let global = Time::Global(k as f64 / f64::from(fps));
            assert_eq!(
                tl.locate(global).unwrap(),
                (index, t),
                "fps {fps} frame {k}"
            );
        }
    }
}

#[test]
fn the_last_frame_is_inside_the_video() {
    let tl = Timeline::new(30, [1.0, 0.5].into_iter());
    assert_eq!(tl.frames(), 45);
    assert_eq!(
        tl.frame_time(44).unwrap(),
        Time::Scene {
            index: 1,
            t: 14.0 / 30.0
        }
    );
    let e = tl.frame_time(45).unwrap_err().to_string();
    assert_eq!(
        e,
        "frame 45 does not exist; the video has 45 frames at 30 fps (0-based)"
    );
    assert!(tl.locate(Time::Global(tl.length())).is_err());
}

#[test]
fn off_grid_times_find_their_scene() {
    let tl = Timeline::new(30, [0.5, 0.5].into_iter());
    let (index, t) = tl.locate(Time::Global(0.51)).unwrap();
    assert_eq!(index, 1);
    assert!((t - 0.01).abs() < 1e-12);
    assert_eq!(tl.locate(Time::Global(0.499)).unwrap().0, 0);
}
