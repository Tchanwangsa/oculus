use crate::library::layout::{own_dir_rel, own_file_ok, own_kind_of, own_path_ok, OwnKind};

#[test]
fn only_files_below_an_own_folder_pass() {
    for rel in [
        "courses/COMP30026/uploads/notes.pdf",
        "courses/COMP30026/uploads/deeper/scan.png",
        "courses/COMP30026/documents/week-3.md",
        "courses/COMP30026_2026_SM2/documents/Untitled-2.md",
        "courses/X/canvases/Board.canvas",
        "courses/X/notebooks/Week 3.notebook",
        "courses/X/UPLOADS/odd.pdf",
        "courses/X/uploads/U\u{308}bung.pdf",
        "courses/数学/documents/笔记.md",
    ] {
        assert!(own_path_ok(rel), "{rel}");
    }
    for rel in [
        // The folders themselves are not the student's to rename or delete.
        "courses/X/uploads",
        "courses/X/documents",
        // Everything else belongs to a sync or to the app.
        "courses/X/files/lecture.pdf",
        "courses/X/home.md",
        "courses/X/agents/INSTRUCTIONS.md",
        "lectures/abc/source1.mp4",
        "agents/memories/x.md",
        // Traversal, however it is spelled.
        "courses/../oculus.db",
        "courses/X/uploads/../../../oculus.db",
        "courses//uploads/x",
        r"courses/X/documents/..\secret.md",
        r"courses/X/documents/C:\secret.md",
        "/courses/X/uploads/a.pdf",
        // Dot-names: staging files and hidden folders.
        "courses/X/uploads/.DS_Store",
        "courses/X/documents/.week-3.md.123-456.tmp",
        "courses/X/uploads/.hidden/a.pdf",
        // A Windows stream.
        "courses/X/uploads/file.txt:stream",
    ] {
        assert!(!own_path_ok(rel), "{rel}");
    }
}

#[test]
fn the_folder_decides_the_kind() {
    assert_eq!(own_kind_of("courses/X/uploads/a.md"), Some(OwnKind::Upload));
    assert_eq!(
        own_kind_of("courses/X/documents/a.md"),
        Some(OwnKind::Document)
    );
    assert_eq!(
        own_kind_of("courses/X/canvases/a.canvas"),
        Some(OwnKind::Canvas)
    );
    assert_eq!(
        own_kind_of("courses/X/notebooks/a.notebook"),
        Some(OwnKind::Notebook)
    );
    assert_eq!(own_kind_of("courses/X/files/a.pdf"), None);
}

#[test]
fn a_kinds_files_carry_its_extension() {
    let doc = OwnKind::Document;
    assert!(own_file_ok("courses/X/documents/week-3.md", doc));
    assert!(own_file_ok("courses/X/documents/drafts/x.md", doc));
    assert!(!own_file_ok("courses/X/documents/x.pdf", doc));
    assert!(!own_file_ok("courses/X/documents/.md", doc));
    assert!(!own_file_ok("courses/X/documents/assets/x.md", doc));
    assert!(!own_file_ok("courses/X/uploads/x.md", doc));
    assert!(!own_file_ok("lectures/abc/documents/x.md", doc));

    let canvas = OwnKind::Canvas;
    assert!(own_file_ok("courses/X/canvases/Week 3.canvas", canvas));
    assert!(!own_file_ok("courses/X/canvases/.canvas", canvas));
    assert!(!own_file_ok("courses/X/canvases/Week 3.notebook", canvas));
    assert!(!own_file_ok("courses/X/notebooks/Week 3.canvas", canvas));
    assert!(own_file_ok(
        "courses/X/notebooks/Week 3.notebook",
        OwnKind::Notebook
    ));

    assert!(own_file_ok("courses/X/uploads/anything", OwnKind::Upload));
}

#[test]
fn new_files_land_in_their_kinds_folder() {
    assert_eq!(
        own_dir_rel("COMP 30026", OwnKind::Upload),
        "courses/COMP_30026/uploads"
    );
    assert_eq!(own_dir_rel("X", OwnKind::Document), "courses/X/documents");
    assert_eq!(own_dir_rel("X", OwnKind::Canvas), "courses/X/canvases");
    assert_eq!(own_dir_rel("X", OwnKind::Notebook), "courses/X/notebooks");
}
