use super::*;
const DOCUMENT: &str = r#"{"type":"oculus-whiteboard","version":1,"elements":[],"viewport":{"x":0,"y":0,"zoom":1},"background":"dots"}"#;
const NOTEBOOK: &str = r##"{"type":"oculus-whiteboard","version":1,"elements":[],"viewport":{"x":0,"y":0,"zoom":1},"background":"#e4e5ea","paper":{"pages":[{"id":"p1","width":595,"height":842,"template":"grid"}]}}"##;

#[test]
fn whiteboard_lifecycle_is_atomic_and_scoped() {
    let root = crate::test_support::Scratch::new("whiteboard-files");
    let first = create_at(&root, DrawingKind::Whiteboard, "TEST", "Ideas", DOCUMENT).unwrap();
    let second = create_at(&root, DrawingKind::Whiteboard, "TEST", "Ideas", DOCUMENT).unwrap();
    assert_ne!(first.relative_path, second.relative_path);
    assert_eq!(
        list_at(&root, DrawingKind::Whiteboard, "TEST")
            .unwrap()
            .len(),
        2
    );
    write_at(&root, &first.relative_path, DOCUMENT).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join(&first.relative_path)).unwrap(),
        DOCUMENT
    );
    let renamed = rename_at(&root, &first.relative_path, "Plan").unwrap();
    assert!(write_at(&root, &first.relative_path, DOCUMENT).is_err());
    assert!(root.join(renamed.relative_path).exists());
    assert_eq!(
        std::fs::read_dir(root.join("courses/TEST"))
            .unwrap()
            .count(),
        2
    );
    for rel in [
        "/tmp/x.whiteboard",
        "courses/../x.whiteboard",
        "courses/X/Course/x.whiteboard",
        "courses/X/../x.whiteboard",
        "courses/X/.whiteboard",
        r"courses/X/..\x.whiteboard",
    ] {
        assert!(!is_whiteboard_rel(rel), "{rel}");
        assert!(write_at(&root, rel, DOCUMENT).is_err());
    }
    assert!(write_at(&root, &second.relative_path, "{}").is_err());
    assert_eq!(
        std::fs::read_to_string(root.join(&second.relative_path)).unwrap(),
        DOCUMENT
    );
}

#[cfg(unix)]
#[test]
fn whiteboard_files_and_folders_cannot_be_symlinks() {
    use std::os::unix::fs::symlink;
    let root = crate::test_support::Scratch::new("whiteboard-links");
    let outside = crate::test_support::Scratch::new("whiteboard-links-outside");
    std::fs::create_dir_all(root.join("courses")).unwrap();
    std::fs::write(outside.join("listed.whiteboard"), DOCUMENT).unwrap();
    symlink(&*outside, root.join("courses/TEST")).unwrap();
    assert!(create_at(&root, DrawingKind::Whiteboard, "TEST", "Escape", DOCUMENT).is_err());
    // A drawing behind the link is never listed.
    assert!(list_at(&root, DrawingKind::Whiteboard, "TEST")
        .unwrap()
        .is_empty());
    std::fs::remove_file(outside.join("listed.whiteboard")).unwrap();
    std::fs::remove_file(root.join("courses/TEST")).unwrap();
    let own = create_at(&root, DrawingKind::Whiteboard, "TEST", "Own", DOCUMENT).unwrap();
    std::fs::write(outside.join("private.whiteboard"), DOCUMENT).unwrap();
    symlink(
        outside.join("private.whiteboard"),
        root.join("courses/TEST/alias.whiteboard"),
    )
    .unwrap();
    assert!(write_at(&root, "courses/TEST/alias.whiteboard", DOCUMENT).is_err());
    assert!(rename_at(&root, "courses/TEST/alias.whiteboard", "Escape").is_err());
    assert!(write_at(&root, &own.relative_path, DOCUMENT).is_ok());
    // A folder deeper down is checked the same way.
    std::fs::create_dir_all(outside.join("deep")).unwrap();
    std::fs::write(outside.join("deep/inner.whiteboard"), DOCUMENT).unwrap();
    symlink(outside.join("deep"), root.join("courses/TEST/sub")).unwrap();
    assert!(write_at(&root, "courses/TEST/sub/inner.whiteboard", DOCUMENT).is_err());
    assert_eq!(std::fs::read_dir(&*outside).unwrap().count(), 2);
}

#[test]
fn a_notebook_lands_at_the_subjects_root_and_keeps_its_kind() {
    let root = crate::test_support::Scratch::new("notebook-files");
    let notebook = create_at(&root, DrawingKind::Notebook, "TEST", "Week 3", NOTEBOOK).unwrap();
    assert_eq!(notebook.relative_path, "courses/TEST/Week_3.notebook");
    assert_eq!(notebook.file_type, "notebook");
    let again = create_at(&root, DrawingKind::Notebook, "TEST", "Week 3", NOTEBOOK).unwrap();
    assert_eq!(again.relative_path, "courses/TEST/Week_3-2.notebook");
    let whiteboard = create_at(&root, DrawingKind::Whiteboard, "TEST", "Board", DOCUMENT).unwrap();
    assert_eq!(whiteboard.file_type, "whiteboard");

    let notebooks = list_at(&root, DrawingKind::Notebook, "TEST").unwrap();
    assert_eq!(notebooks.len(), 2);
    assert!(notebooks.iter().all(|f| f.file_type == "notebook"));
    let whiteboards = list_at(&root, DrawingKind::Whiteboard, "TEST").unwrap();
    assert_eq!(
        whiteboards
            .iter()
            .map(|f| f.relative_path.as_str())
            .collect::<Vec<_>>(),
        ["courses/TEST/Board.whiteboard"]
    );

    write_at(&root, &notebook.relative_path, NOTEBOOK).unwrap();
    let renamed = rename_at(&root, &notebook.relative_path, "Plan.whiteboard").unwrap();
    assert_eq!(
        renamed.relative_path,
        "courses/TEST/Plan.whiteboard.notebook"
    );
    assert_eq!(renamed.file_type, "notebook");
    let renamed = rename_at(&root, &renamed.relative_path, "Plan.notebook").unwrap();
    assert_eq!(renamed.relative_path, "courses/TEST/Plan.notebook");
    assert!(read_dir_names(&root, "courses/TEST").contains(&"Plan.notebook".into()));
    let board = rename_at(&root, &whiteboard.relative_path, "Plan").unwrap();
    assert_eq!(board.relative_path, "courses/TEST/Plan.whiteboard");
}

#[test]
fn a_notebook_needs_pages_and_a_whiteboard_does_not() {
    let root = crate::test_support::Scratch::new("notebook-paper");
    assert!(create_at(&root, DrawingKind::Notebook, "TEST", "Bare", DOCUMENT).is_err());
    assert!(list_at(&root, DrawingKind::Notebook, "TEST")
        .unwrap()
        .is_empty());
    let empty_paper = NOTEBOOK.replace(
        r#"[{"id":"p1","width":595,"height":842,"template":"grid"}]"#,
        "[]",
    );
    assert!(create_at(&root, DrawingKind::Notebook, "TEST", "Empty", &empty_paper).is_err());
    let notebook = create_at(&root, DrawingKind::Notebook, "TEST", "Paper", NOTEBOOK).unwrap();
    assert!(write_at(&root, &notebook.relative_path, DOCUMENT).is_err());
    assert_eq!(
        std::fs::read_to_string(root.join(&notebook.relative_path)).unwrap(),
        NOTEBOOK
    );
    // `paper` in a whiteboard is just a field: the file is a whiteboard, saved as written.
    let whiteboard = create_at(
        &root,
        DrawingKind::Whiteboard,
        "TEST",
        "With paper",
        NOTEBOOK,
    )
    .unwrap();
    assert_eq!(
        (
            whiteboard.relative_path.as_str(),
            whiteboard.file_type.as_str()
        ),
        ("courses/TEST/With_paper.whiteboard", "whiteboard")
    );
    assert_eq!(
        std::fs::read_to_string(root.join(&whiteboard.relative_path)).unwrap(),
        NOTEBOOK
    );
    write_at(&root, &whiteboard.relative_path, DOCUMENT).unwrap();
}

#[test]
fn each_kind_takes_its_extension_anywhere_in_the_own_files() {
    let root = crate::test_support::Scratch::new("notebook-paths");
    assert_eq!(
        DrawingKind::of("courses/X/a.notebook"),
        Some(DrawingKind::Notebook)
    );
    assert_eq!(
        DrawingKind::of("courses/X/a.whiteboard"),
        Some(DrawingKind::Whiteboard)
    );
    // At any depth among the student's own files.
    assert_eq!(
        DrawingKind::of("courses/X/sub/a.notebook"),
        Some(DrawingKind::Notebook)
    );
    for rel in [
        "courses/X/Course/Files/a.notebook",
        "courses/X/course/a.whiteboard",
        "courses/X/.oculus/a.notebook",
        "courses/X/.notebook",
        "courses/X/.hidden.notebook",
        "courses/X/../a.notebook",
        "courses/../a.notebook",
        "courses/X",
        r"courses/X/..\a.notebook",
        "courses/X/.sub/a.notebook",
    ] {
        assert_eq!(DrawingKind::of(rel), None, "{rel}");
        assert!(write_at(&root, rel, NOTEBOOK).is_err(), "{rel}");
    }
    std::fs::create_dir_all(root.join("courses/X/Course/Files")).unwrap();
    std::fs::write(
        root.join("courses/X/Course/Files/stray.whiteboard"),
        DOCUMENT,
    )
    .unwrap();
    std::fs::create_dir_all(root.join("courses/X/.oculus")).unwrap();
    std::fs::write(root.join("courses/X/.oculus/stray.notebook"), NOTEBOOK).unwrap();
    assert!(list_at(&root, DrawingKind::Notebook, "X")
        .unwrap()
        .is_empty());
    assert!(list_at(&root, DrawingKind::Whiteboard, "X")
        .unwrap()
        .is_empty());
}

#[cfg(unix)]
#[test]
fn notebook_files_and_folders_cannot_be_symlinks() {
    use std::os::unix::fs::symlink;
    let root = crate::test_support::Scratch::new("notebook-links");
    let outside = crate::test_support::Scratch::new("notebook-links-outside");
    std::fs::create_dir_all(root.join("courses")).unwrap();
    std::fs::write(outside.join("listed.notebook"), NOTEBOOK).unwrap();
    symlink(&*outside, root.join("courses/TEST")).unwrap();
    assert!(create_at(&root, DrawingKind::Notebook, "TEST", "Escape", NOTEBOOK).is_err());
    // A drawing behind the link is never listed.
    assert!(list_at(&root, DrawingKind::Notebook, "TEST")
        .unwrap()
        .is_empty());
    std::fs::remove_file(outside.join("listed.notebook")).unwrap();
    std::fs::remove_file(root.join("courses/TEST")).unwrap();
    let own = create_at(&root, DrawingKind::Notebook, "TEST", "Own", NOTEBOOK).unwrap();
    std::fs::write(outside.join("private.notebook"), NOTEBOOK).unwrap();
    symlink(
        outside.join("private.notebook"),
        root.join("courses/TEST/alias.notebook"),
    )
    .unwrap();
    assert!(write_at(&root, "courses/TEST/alias.notebook", NOTEBOOK).is_err());
    assert!(rename_at(&root, "courses/TEST/alias.notebook", "Escape").is_err());
    assert!(write_at(&root, &own.relative_path, NOTEBOOK).is_ok());
    assert_eq!(std::fs::read_dir(&*outside).unwrap().count(), 1);
}

fn read_dir_names(root: &std::path::Path, rel: &str) -> Vec<String> {
    std::fs::read_dir(root.join(rel))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}
