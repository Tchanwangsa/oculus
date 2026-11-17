use crate::library::layout::{
    own_dir_rel, own_file_ok, own_files, own_kind_of, own_path_ok, own_top_level, subjects_on_disk,
    OwnKind,
};
use crate::test_support::Scratch;

#[test]
fn own_files_sit_beside_course_at_any_depth() {
    for rel in [
        "courses/COMP30026/notes.pdf",
        "courses/COMP30026/Exam prep/deeper/scan.png",
        "courses/COMP30026_2026_SM2/Untitled-2.md",
        "courses/X/Board.canvas",
        "courses/X/Week 3/Week 3.notebook",
        "courses/X/Courses/a.pdf",
        "courses/X/U\u{308}bung.pdf",
        "courses/数学/笔记.md",
    ] {
        assert!(own_path_ok(rel), "{rel}");
    }
    for rel in [
        // The subject folder itself is not the student's to rename or delete.
        "courses/X",
        // Everything synced belongs to the sync, whatever its case.
        "courses/X/Course",
        "courses/X/Course/Files/lecture.pdf",
        "courses/X/course/home.md",
        "courses/X/Course/Recordings/2026-08-04-x/source1.mp4",
        // App state.
        "courses/X/.oculus/agents/INSTRUCTIONS.md",
        "lectures/abc/source1.mp4",
        "agents/memories/x.md",
        // Traversal, however it is spelled.
        "courses/../oculus.db",
        "courses/X/Exam prep/../../../oculus.db",
        "courses//Exam prep/x",
        r"courses/X/..\secret.md",
        r"courses/X/C:\secret.md",
        "/courses/X/a.pdf",
        // Dot-names: staging files, a note's pictures and hidden folders.
        "courses/X/.DS_Store",
        "courses/X/.week-3.md.123-456.tmp",
        "courses/X/.assets/p.png",
        "courses/X/Exam prep/.hidden/a.pdf",
        // A Windows stream.
        "courses/X/file.txt:stream",
    ] {
        assert!(!own_path_ok(rel), "{rel}");
    }
}

#[test]
fn the_extension_decides_the_kind() {
    assert_eq!(
        own_kind_of("courses/X/Exam prep/notes.md"),
        Some(OwnKind::Document)
    );
    assert_eq!(own_kind_of("courses/X/b.canvas"), Some(OwnKind::Canvas));
    assert_eq!(
        own_kind_of("courses/X/w/p.notebook"),
        Some(OwnKind::Notebook)
    );
    assert_eq!(own_kind_of("courses/X/scan.png"), Some(OwnKind::Upload));
    assert_eq!(own_kind_of("courses/X/Course/Files/a.pdf"), None);
}

#[test]
fn a_kinds_files_carry_its_extension() {
    let doc = OwnKind::Document;
    assert!(own_file_ok("courses/X/week-3.md", doc));
    assert!(own_file_ok("courses/X/drafts/x.md", doc));
    assert!(!own_file_ok("courses/X/x.pdf", doc));
    assert!(!own_file_ok("courses/X/.md", doc));
    assert!(!own_file_ok("courses/X/.assets/x.md", doc));
    assert!(!own_file_ok("courses/X/Course/Pages/x.md", doc));
    assert!(!own_file_ok("lectures/abc/x.md", doc));

    let canvas = OwnKind::Canvas;
    assert!(own_file_ok("courses/X/Week 3.canvas", canvas));
    assert!(!own_file_ok("courses/X/.canvas", canvas));
    assert!(!own_file_ok("courses/X/Week 3.notebook", canvas));
    assert!(own_file_ok("courses/X/Week 3.notebook", OwnKind::Notebook));

    assert!(own_file_ok("courses/X/anything", OwnKind::Upload));
}

#[test]
fn new_files_land_at_the_subjects_root() {
    assert_eq!(
        own_dir_rel("COMP 30026", OwnKind::Upload),
        "courses/COMP_30026"
    );
    for kind in [OwnKind::Document, OwnKind::Canvas, OwnKind::Notebook] {
        assert_eq!(own_dir_rel("X", kind), "courses/X");
    }
}

#[test]
fn the_walk_lists_own_files_only() {
    let dir = Scratch::new("layout-own-walk");
    for rel in [
        "courses/X/a.md",
        "courses/X/Exam prep/b.pdf",
        "courses/X/Exam prep/deep/c.canvas",
        "courses/X/Course/Files/synced.pdf",
        "courses/X/.oculus/agents/INSTRUCTIONS.md",
        "courses/X/.assets/p.png",
        "courses/X/.DS_Store",
    ] {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "x").unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink("../../agents/AGENTS.md", dir.join("courses/X/AGENTS.md")).unwrap();
    let found: Vec<String> = own_files(&dir, "X")
        .into_iter()
        .map(|(rel, _)| rel)
        .collect();
    assert_eq!(
        found,
        [
            "courses/X/Exam prep/b.pdf",
            "courses/X/Exam prep/deep/c.canvas",
            "courses/X/a.md",
        ]
    );
}

#[test]
fn the_top_level_own_entries_of_every_subject_leave_out_course_and_oculus() {
    let dir = Scratch::new("layout-own-top");
    for rel in [
        "courses/X/a.md",
        "courses/X/Exam prep/b.pdf",
        "courses/X/Course/Files/synced.pdf",
        "courses/X/.oculus/agents/INSTRUCTIONS.md",
        "courses/X/.assets/p.png",
        "courses/Y/c.canvas",
    ] {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "x").unwrap();
    }
    std::fs::create_dir_all(dir.join("courses/.trash")).unwrap();
    std::fs::write(dir.join("courses/loose.pdf"), "x").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("../../agents/AGENTS.md", dir.join("courses/X/AGENTS.md")).unwrap();

    assert_eq!(subjects_on_disk(&dir), ["X", "Y"]);
    let found: Vec<String> = own_top_level(&dir)
        .iter()
        .map(|p| crate::library::paths::slash_path(p.strip_prefix(&*dir).unwrap()))
        .collect();
    assert_eq!(
        found,
        [
            "courses/X/.assets",
            "courses/X/Exam prep",
            "courses/X/a.md",
            "courses/Y/c.canvas",
        ]
    );
}
