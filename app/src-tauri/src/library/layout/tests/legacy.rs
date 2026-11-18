//! The layout before the Drive tree (synced kinds at the subject root, one
//! folder per own kind, `lectures/<media id>/`), which the one-shot move
//! recognises old paths by; checked with the same rules as the current one.

use crate::library::layout::{OwnKind, Zone, LEGACY};

#[test]
fn synced_folders_sit_at_the_root_and_own_kinds_have_folders() {
    let cases = [
        ("courses/X/files/a.pdf", Zone::Synced),
        ("courses/X/home.md", Zone::Synced),
        ("courses/X/lessons/1-l/03-q1a/q1a.py", Zone::Synced),
        ("courses/X/something-new/a.md", Zone::Synced),
        ("courses/X", Zone::Synced),
        ("lectures/abc/source1.mp4", Zone::Synced),
        ("LECTURES/abc/source1.mp4", Zone::Synced),
        ("courses/X/uploads/a.pdf", Zone::Own),
        ("courses/X/Uploads/a.pdf", Zone::Own),
        ("courses/X/documents/deep/er/a.md", Zone::Own),
        ("courses/X/canvases/a.canvas", Zone::Own),
        ("courses/X/notebooks/a.notebook", Zone::Own),
        ("courses/X/agents/INSTRUCTIONS.md", Zone::Hidden),
        ("courses/X/Agents/x.md", Zone::Hidden),
        ("agents/memories/x.md", Zone::Hidden),
        ("courses/X/uploads/../../../oculus.db", Zone::Outside),
    ];
    for (rel, zone) in cases {
        assert_eq!(LEGACY.zone_of(rel), zone, "{rel}");
    }
}

#[test]
fn own_files_go_by_their_folder() {
    assert!(LEGACY.own_path_ok("courses/X/uploads/deeper/scan.png"));
    assert!(!LEGACY.own_path_ok("courses/X/uploads"));
    assert!(!LEGACY.own_path_ok("courses/X/files/lecture.pdf"));
    assert!(!LEGACY.own_path_ok("courses/X/uploads/.DS_Store"));
    assert_eq!(
        LEGACY.own_kind_of("courses/X/uploads/a.md"),
        Some(OwnKind::Upload)
    );
    assert_eq!(
        LEGACY.own_kind_of("courses/X/documents/a.md"),
        Some(OwnKind::Document)
    );
    assert_eq!(
        LEGACY.own_kind_of("courses/X/notebooks/a.notebook"),
        Some(OwnKind::Notebook)
    );
    assert!(LEGACY.own_file_ok("courses/X/documents/drafts/x.md", OwnKind::Document));
    assert!(!LEGACY.own_file_ok("courses/X/documents/assets/x.md", OwnKind::Document));
    assert!(!LEGACY.own_file_ok("courses/X/notebooks/Week 3.whiteboard", OwnKind::Whiteboard));
}

#[test]
fn categories_follow_the_first_folder() {
    let cases = [
        ("home.md", "home"),
        ("files/a.pdf", "file"),
        ("files/v.mp4", "file"),
        ("ed/0001-x.md", "ed"),
        ("lessons/1-x.md", "lessons"),
        ("uploads/notes.md", "upload"),
        ("documents/week-3.md", "document"),
        ("canvases/b.canvas", "whiteboard"),
        ("notebooks/p.notebook", "notebook"),
        ("Uploads/odd-case.pdf", "other"),
        ("uploads", "other"),
        ("agents/x.md", "other"),
    ];
    for (inner, category) in cases {
        assert_eq!(LEGACY.category_of(inner), category, "{inner}");
    }
}

#[test]
fn a_recording_is_its_media_ids_folder() {
    assert_eq!(LEGACY.recordings_rel("X"), "lectures");
    assert_eq!(
        LEGACY
            .recording_of("lectures/abc-123/frames/0001.jpg")
            .as_deref(),
        Some("lectures/abc-123")
    );
    assert_eq!(LEGACY.recording_of("lectures").as_deref(), None);
    assert_eq!(LEGACY.recording_of("courses/X/files/a.pdf"), None);
}
