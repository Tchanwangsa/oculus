//! The layout the Drive plan moves to (`Course/…` beside the student's own
//! files, `.oculus/` hidden), checked with the same rules as today's.

use crate::library::layout::shape::Shape;
use crate::library::layout::{OwnKind, SyncedFile, SyncedKind, Zone};

const TARGET: Shape = Shape {
    synced_root: Some("Course"),
    synced_dirs: &[
        (SyncedKind::Files, "Files"),
        (SyncedKind::Media, "Media"),
        (SyncedKind::Lessons, "Lessons"),
        (SyncedKind::Discussion, "Discussion"),
        (SyncedKind::Pages, "Pages"),
        (SyncedKind::Assignments, "Assignments"),
        (SyncedKind::Quizzes, "Quizzes"),
        (SyncedKind::Announcements, "Announcements"),
        (SyncedKind::Modules, "Modules"),
        (SyncedKind::Images, "Images"),
    ],
    synced_files: &[
        (SyncedFile::Home, "home.md"),
        (SyncedFile::Syllabus, "syllabus.md"),
    ],
    own_dirs: &[
        (OwnKind::Upload, ""),
        (OwnKind::Document, ""),
        (OwnKind::Canvas, ""),
        (OwnKind::Notebook, ""),
    ],
    own_kind_by_folder: false,
    hidden: &[".oculus"],
    agents: ".oculus/agents",
    note_assets: ".assets",
    recordings_root: None,
};

#[test]
fn course_is_synced_and_everything_beside_it_is_the_students() {
    let cases = [
        ("courses/X/Course/Files/a.pdf", Zone::Synced),
        (
            "courses/X/Course/Recordings/2026-08-04-intro/source1.mp4",
            Zone::Synced,
        ),
        ("courses/X/course/files/a.pdf", Zone::Synced),
        ("courses/X/Exam prep/notes.md", Zone::Own),
        ("courses/X/a.pdf", Zone::Own),
        ("courses/X/.oculus/agents/INSTRUCTIONS.md", Zone::Hidden),
        ("courses/X", Zone::Synced),
        ("lectures/abc/source1.mp4", Zone::Hidden),
        ("courses/X/Exam prep/../../../oculus.db", Zone::Outside),
    ];
    for (rel, zone) in cases {
        assert_eq!(TARGET.zone_of(rel), zone, "{rel}");
    }
}

#[test]
fn own_files_go_by_extension_at_any_depth() {
    assert!(TARGET.own_path_ok("courses/X/a.pdf"));
    assert!(TARGET.own_path_ok("courses/X/Exam prep/week 3/notes.md"));
    assert!(!TARGET.own_path_ok("courses/X/Course/Files/a.pdf"));
    assert!(!TARGET.own_path_ok("courses/X/.oculus/agents/x.md"));
    assert!(!TARGET.own_path_ok("courses/X/Exam prep/.assets/p.png"));
    assert_eq!(
        TARGET.own_kind_of("courses/X/Exam prep/notes.md"),
        Some(OwnKind::Document)
    );
    assert_eq!(
        TARGET.own_kind_of("courses/X/b.canvas"),
        Some(OwnKind::Canvas)
    );
    assert_eq!(
        TARGET.own_kind_of("courses/X/w/p.notebook"),
        Some(OwnKind::Notebook)
    );
    assert_eq!(
        TARGET.own_kind_of("courses/X/scan.png"),
        Some(OwnKind::Upload)
    );
    assert!(TARGET.own_file_ok("courses/X/Exam prep/notes.md", OwnKind::Document));
    assert!(!TARGET.own_file_ok("courses/X/notes.pdf", OwnKind::Document));
}

#[test]
fn categories_follow_course_folders_and_own_extensions() {
    let cases = [
        ("Course/home.md", "home"),
        ("Course/syllabus.md", "syllabus"),
        ("Course/Files/a.pdf", "file"),
        ("Course/Media/v.mp4", "file"),
        ("Course/Discussion/0001-x.md", "ed"),
        ("Course/Lessons/1-x.md", "lessons"),
        ("Course/Unknown/x.md", "other"),
        ("Exam prep/notes.md", "document"),
        ("board.canvas", "canvas"),
        ("w/p.notebook", "notebook"),
        ("scan.pdf", "upload"),
        (".oculus/agents/x.md", "other"),
    ];
    for (inner, category) in cases {
        assert_eq!(TARGET.category_of(inner), category, "{inner}");
    }
}
