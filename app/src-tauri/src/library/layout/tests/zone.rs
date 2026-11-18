use crate::library::layout::{reserved_name, reserved_names, zone_of, Zone};

#[test]
fn every_path_shape_has_its_zone() {
    let cases = [
        ("courses/X/Course/Files/a.pdf", Zone::Synced),
        ("courses/X/Course/Files/.hidden", Zone::Synced),
        ("courses/X/Course/home.md", Zone::Synced),
        ("courses/X/Course/Lessons/1-l/03-q1a/q1a.py", Zone::Synced),
        (
            "courses/X/Course/Recordings/2026-08-04-mo-l105/source1.mp4",
            Zone::Synced,
        ),
        ("courses/X/Course/something-new/a.md", Zone::Synced),
        ("courses/X/Course", Zone::Synced),
        ("courses", Zone::Synced),
        ("courses/X", Zone::Synced),
        ("courses/X/a.pdf", Zone::Own),
        ("courses/X/Exam prep", Zone::Own),
        ("courses/X/Exam prep/deep/er/notes.md", Zone::Own),
        ("courses/X/board.whiteboard", Zone::Own),
        ("courses/X/.assets/p.png", Zone::Own),
        ("courses/X/.oculus/agents/INSTRUCTIONS.md", Zone::Hidden),
        ("courses/X/.oculus/agents/memories/m.md", Zone::Hidden),
        (
            "courses/X/.oculus/derived/Course/Files/a.pages.json",
            Zone::Hidden,
        ),
        ("lectures/abc/source1.mp4", Zone::Hidden),
        ("agents/memories/x.md", Zone::Hidden),
        ("studio/videos/a/video.json", Zone::Hidden),
        ("oculus.db", Zone::Hidden),
    ];
    for (rel, zone) in cases {
        assert_eq!(zone_of(rel), zone, "{rel}");
    }
}

#[test]
fn folder_names_ignore_case_as_the_filesystem_does() {
    assert_eq!(zone_of("courses/X/course/files/a.pdf"), Zone::Synced);
    assert_eq!(zone_of("Courses/X/COURSE/home.md"), Zone::Synced);
    assert_eq!(zone_of("courses/X/.OCULUS/agents/x.md"), Zone::Hidden);
    assert_eq!(zone_of("courses/X/Courses/a.pdf"), Zone::Own);
}

#[test]
fn course_and_oculus_are_reserved_at_a_subjects_root() {
    for name in ["Course", "course", "COURSE", ".oculus", ".Oculus"] {
        assert!(reserved_name(name), "{name}");
    }
    for name in ["Courses", "My Course", "oculus", "Exam prep"] {
        assert!(!reserved_name(name), "{name}");
    }
    assert_eq!(reserved_names(), ["Course", ".oculus"]);
    for name in reserved_names() {
        assert_ne!(
            zone_of(&format!("courses/X/{name}/a")),
            Zone::Own,
            "{name} is not the student's"
        );
    }
}

#[test]
fn any_spelling_that_could_leave_the_data_dir_is_outside() {
    for rel in [
        "",
        "/etc/passwd",
        "..",
        "../oculus.db",
        "./courses/X/Course/Files/a.pdf",
        "courses/../oculus.db",
        "courses/X/Exam prep/../../../oculus.db",
        "courses/X/Exam prep/..",
        "courses//Exam prep/x",
        "courses/X/Course/Files/",
        r"courses\X\Course\Files\a.pdf",
        r"courses/X/Exam prep/..\..\oculus.db",
        "C:/Windows/win.ini",
        "C:x",
        "courses/X/Exam prep/a\0.pdf",
    ] {
        assert_eq!(zone_of(rel), Zone::Outside, "{rel:?}");
    }
}

#[test]
fn unicode_names_are_ordinary_names() {
    // NFC and NFD spellings of the same name; neither is normalised away.
    assert_eq!(zone_of("courses/X/Übung.pdf"), Zone::Own);
    assert_eq!(zone_of("courses/X/U\u{308}bung.pdf"), Zone::Own);
    assert_eq!(zone_of("courses/数学/Course/Files/讲义.pdf"), Zone::Synced);
}
