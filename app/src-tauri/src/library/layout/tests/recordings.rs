use std::path::Path;

use crate::library::layout::{
    new_recording_rel, recording_dir, recording_name, recording_of, recordings_rel,
};

#[test]
fn a_recording_lives_in_its_subjects_course_folder() {
    assert_eq!(
        recordings_rel("MULT20015_2026_SM2"),
        "courses/MULT20015_2026_SM2/Course/Recordings"
    );
    let rel = "courses/X/Course/Recordings/2026-08-04-mo-l105";
    assert_eq!(
        recording_dir(Path::new("/d"), rel).unwrap(),
        Path::new("/d/courses/X/Course/Recordings/2026-08-04-mo-l105")
    );
}

#[test]
fn the_name_is_the_date_and_the_title_without_the_subject_code() {
    let code = "MULT20015_2026_SM2";
    assert_eq!(
        recording_name(
            "2026-07-27T11:05:00.000",
            "MULT20015_2026_SM2 MO L105",
            code
        ),
        "2026-07-27-mo-l105"
    );
    assert_eq!(
        recording_name(
            "2026-08-04T09:00:00.000",
            "Lecture 3: Übung & Café!",
            "COMP30026_2026_SM2"
        ),
        "2026-08-04-lecture-3-ubung-cafe"
    );
    assert_eq!(
        recording_name("2026-08-04", code, code),
        "2026-08-04-lecture"
    );
    assert_eq!(recording_name("", "Intro", code), "undated-intro");
    let long = recording_name("2026-08-04", &"word ".repeat(40), code);
    assert!(long.len() <= "2026-08-04-".len() + 60, "{long}");
    assert!(!long.ends_with('-'), "{long}");
}

#[test]
fn a_taken_name_steps_aside() {
    let taken = [
        "courses/x/course/recordings/2026-08-04-intro",
        "courses/x/course/recordings/2026-08-04-intro-2",
    ];
    let rel = new_recording_rel("X", "2026-08-04", "Intro", |rel| {
        taken.contains(&rel.to_lowercase().as_str())
    });
    assert_eq!(rel, "courses/X/Course/Recordings/2026-08-04-intro-3");
}

#[test]
fn a_path_inside_a_recording_names_its_folder() {
    assert_eq!(
        recording_of("courses/X/Course/Recordings/2026-08-04-intro/frames/live/12-source1.jpg")
            .as_deref(),
        Some("courses/X/Course/Recordings/2026-08-04-intro")
    );
    assert_eq!(
        recording_of("courses/X/Course/Recordings/2026-08-04-intro").as_deref(),
        Some("courses/X/Course/Recordings/2026-08-04-intro")
    );
    for rel in [
        "courses/X/Course/Recordings",
        "courses/X/Course/Files/a.pdf",
        "courses/X/Recordings/2026-08-04-intro",
        "lectures/abc-123",
        "courses/X/Course/Recordings/.part",
    ] {
        assert_eq!(recording_of(rel), None, "{rel}");
    }
}

#[test]
fn only_a_recording_folder_resolves() {
    for rel in [
        "",
        "lectures/abc",
        "courses/X/Course/Recordings",
        "courses/X/Course/Recordings/a/b",
        "courses/X/Course/Recordings/../../../oculus.db",
        "courses/X/Course/Files/a.pdf",
    ] {
        assert!(recording_dir(Path::new("/d"), rel).is_err(), "{rel:?}");
    }
}
