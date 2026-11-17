use crate::library::layout::{zone_of, Zone};

#[test]
fn every_path_shape_has_its_zone() {
    let cases = [
        ("courses/X/files/a.pdf", Zone::Synced),
        ("courses/X/files/.hidden", Zone::Synced),
        ("courses/X/home.md", Zone::Synced),
        ("courses/X/lessons/1-l/03-q1a/q1a.py", Zone::Synced),
        ("courses/X/something-new/a.md", Zone::Synced),
        ("courses", Zone::Synced),
        ("courses/X", Zone::Synced),
        ("lectures/abc/source1.mp4", Zone::Synced),
        ("lectures", Zone::Synced),
        ("courses/X/uploads/a.pdf", Zone::Own),
        ("courses/X/uploads", Zone::Own),
        ("courses/X/documents/deep/er/a.md", Zone::Own),
        ("courses/X/canvases/a.canvas", Zone::Own),
        ("courses/X/notebooks/a.notebook", Zone::Own),
        ("courses/X/agents/INSTRUCTIONS.md", Zone::Hidden),
        ("courses/X/agents/memories/m.md", Zone::Hidden),
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
    assert_eq!(zone_of("courses/X/Uploads/a.pdf"), Zone::Own);
    assert_eq!(zone_of("Courses/X/DOCUMENTS/a.md"), Zone::Own);
    assert_eq!(zone_of("courses/X/Agents/x.md"), Zone::Hidden);
    assert_eq!(zone_of("LECTURES/abc/source1.mp4"), Zone::Synced);
}

#[test]
fn any_spelling_that_could_leave_the_data_dir_is_outside() {
    for rel in [
        "",
        "/etc/passwd",
        "..",
        "../oculus.db",
        "./courses/X/files/a.pdf",
        "courses/../oculus.db",
        "courses/X/uploads/../../../oculus.db",
        "courses/X/uploads/..",
        "courses//uploads/x",
        "courses/X/files/",
        r"courses\X\files\a.pdf",
        r"courses/X/uploads/..\..\oculus.db",
        "C:/Windows/win.ini",
        "C:x",
        "courses/X/uploads/a\0.pdf",
    ] {
        assert_eq!(zone_of(rel), Zone::Outside, "{rel:?}");
    }
}

#[test]
fn unicode_names_are_ordinary_names() {
    // NFC and NFD spellings of the same name; neither is normalised away.
    assert_eq!(zone_of("courses/X/uploads/Übung.pdf"), Zone::Own);
    assert_eq!(zone_of("courses/X/uploads/U\u{308}bung.pdf"), Zone::Own);
    assert_eq!(zone_of("courses/数学/files/讲义.pdf"), Zone::Synced);
}
