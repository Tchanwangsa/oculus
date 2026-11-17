use super::*;
const DOCUMENT: &str = r#"{"type":"oculus-canvas","version":1,"elements":[],"viewport":{"x":0,"y":0,"zoom":1},"background":"dots"}"#;
const NOTEBOOK: &str = r##"{"type":"oculus-canvas","version":1,"elements":[],"viewport":{"x":0,"y":0,"zoom":1},"background":"#e4e5ea","paper":{"pages":[{"id":"p1","width":595,"height":842,"template":"grid"}]}}"##;

#[test]
fn canvas_lifecycle_is_atomic_and_scoped() {
    let root = crate::test_support::Scratch::new("canvas-files");
    let first = create_at(&root, DrawingKind::Canvas, "TEST", "Ideas", DOCUMENT).unwrap();
    let second = create_at(&root, DrawingKind::Canvas, "TEST", "Ideas", DOCUMENT).unwrap();
    assert_ne!(first.relative_path, second.relative_path);
    assert_eq!(
        list_at(&root, DrawingKind::Canvas, "TEST").unwrap().len(),
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
        std::fs::read_dir(root.join("courses/TEST/canvases"))
            .unwrap()
            .count(),
        2
    );
    for rel in [
        "/tmp/x.canvas",
        "courses/../canvases/x.canvas",
        "courses/X/documents/x.canvas",
        "courses/X/canvases/../x.canvas",
        "courses/X/canvases/.canvas",
        r"courses/X/canvases/..\x.canvas",
    ] {
        assert!(!is_canvas_rel(rel), "{rel}");
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
fn canvas_files_and_folders_cannot_be_symlinks() {
    use std::os::unix::fs::symlink;
    let root = crate::test_support::Scratch::new("canvas-links");
    let outside = crate::test_support::Scratch::new("canvas-links-outside");
    std::fs::create_dir_all(root.join("courses/TEST")).unwrap();
    symlink(&*outside, root.join("courses/TEST/canvases")).unwrap();
    assert!(create_at(&root, DrawingKind::Canvas, "TEST", "Escape", DOCUMENT).is_err());
    assert!(list_at(&root, DrawingKind::Canvas, "TEST").is_err());
    std::fs::remove_file(root.join("courses/TEST/canvases")).unwrap();
    let own = create_at(&root, DrawingKind::Canvas, "TEST", "Own", DOCUMENT).unwrap();
    std::fs::write(outside.join("private.canvas"), DOCUMENT).unwrap();
    symlink(
        outside.join("private.canvas"),
        root.join("courses/TEST/canvases/alias.canvas"),
    )
    .unwrap();
    assert!(write_at(&root, "courses/TEST/canvases/alias.canvas", DOCUMENT).is_err());
    assert!(rename_at(&root, "courses/TEST/canvases/alias.canvas", "Escape").is_err());
    assert!(write_at(&root, &own.relative_path, DOCUMENT).is_ok());
    // A folder deeper down is checked the same way.
    std::fs::create_dir_all(outside.join("deep")).unwrap();
    std::fs::write(outside.join("deep/inner.canvas"), DOCUMENT).unwrap();
    symlink(outside.join("deep"), root.join("courses/TEST/canvases/sub")).unwrap();
    assert!(write_at(&root, "courses/TEST/canvases/sub/inner.canvas", DOCUMENT).is_err());
    assert_eq!(std::fs::read_dir(&*outside).unwrap().count(), 2);
}

#[test]
fn a_notebook_lives_in_its_own_folder_and_keeps_its_kind() {
    let root = crate::test_support::Scratch::new("notebook-files");
    let notebook = create_at(&root, DrawingKind::Notebook, "TEST", "Week 3", NOTEBOOK).unwrap();
    assert_eq!(
        notebook.relative_path,
        "courses/TEST/notebooks/Week_3.notebook"
    );
    assert_eq!(notebook.file_type, "notebook");
    let again = create_at(&root, DrawingKind::Notebook, "TEST", "Week 3", NOTEBOOK).unwrap();
    assert_eq!(
        again.relative_path,
        "courses/TEST/notebooks/Week_3-2.notebook"
    );
    let canvas = create_at(&root, DrawingKind::Canvas, "TEST", "Board", DOCUMENT).unwrap();
    assert_eq!(canvas.file_type, "canvas");

    let notebooks = list_at(&root, DrawingKind::Notebook, "TEST").unwrap();
    assert_eq!(notebooks.len(), 2);
    assert!(notebooks.iter().all(|f| f.file_type == "notebook"));
    let canvases = list_at(&root, DrawingKind::Canvas, "TEST").unwrap();
    assert_eq!(
        canvases
            .iter()
            .map(|f| f.relative_path.as_str())
            .collect::<Vec<_>>(),
        ["courses/TEST/canvases/Board.canvas"]
    );

    write_at(&root, &notebook.relative_path, NOTEBOOK).unwrap();
    let renamed = rename_at(&root, &notebook.relative_path, "Plan.canvas").unwrap();
    assert_eq!(
        renamed.relative_path,
        "courses/TEST/notebooks/Plan.canvas.notebook"
    );
    assert_eq!(renamed.file_type, "notebook");
    let renamed = rename_at(&root, &renamed.relative_path, "Plan.notebook").unwrap();
    assert_eq!(
        renamed.relative_path,
        "courses/TEST/notebooks/Plan.notebook"
    );
    assert!(read_dir_names(&root, "courses/TEST/notebooks").contains(&"Plan.notebook".into()));
    let board = rename_at(&root, &canvas.relative_path, "Plan").unwrap();
    assert_eq!(board.relative_path, "courses/TEST/canvases/Plan.canvas");
}

#[test]
fn a_notebook_needs_pages_and_a_canvas_does_not() {
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
    // `paper` in a canvas is just a field: the file is a canvas, saved as written.
    let canvas = create_at(&root, DrawingKind::Canvas, "TEST", "With paper", NOTEBOOK).unwrap();
    assert_eq!(
        (canvas.relative_path.as_str(), canvas.file_type.as_str()),
        ("courses/TEST/canvases/With_paper.canvas", "canvas")
    );
    assert_eq!(
        std::fs::read_to_string(root.join(&canvas.relative_path)).unwrap(),
        NOTEBOOK
    );
    write_at(&root, &canvas.relative_path, DOCUMENT).unwrap();
}

#[test]
fn each_kind_only_takes_its_own_folder_and_extension() {
    let root = crate::test_support::Scratch::new("notebook-paths");
    assert_eq!(
        DrawingKind::of("courses/X/notebooks/a.notebook"),
        Some(DrawingKind::Notebook)
    );
    assert_eq!(
        DrawingKind::of("courses/X/canvases/a.canvas"),
        Some(DrawingKind::Canvas)
    );
    // Any depth inside its own folder.
    assert_eq!(
        DrawingKind::of("courses/X/notebooks/sub/a.notebook"),
        Some(DrawingKind::Notebook)
    );
    for rel in [
        "courses/X/canvases/a.notebook",
        "courses/X/notebooks/a.canvas",
        "courses/X/notebooks/.notebook",
        "courses/X/notebooks/.hidden.notebook",
        "courses/X/notebooks/../a.notebook",
        "courses/../notebooks/a.notebook",
        "courses/X/documents/a.notebook",
        r"courses/X/notebooks/..\a.notebook",
        "courses/X/notebooks/.sub/a.notebook",
    ] {
        assert_eq!(DrawingKind::of(rel), None, "{rel}");
        assert!(write_at(&root, rel, NOTEBOOK).is_err(), "{rel}");
    }
    std::fs::create_dir_all(root.join("courses/X/notebooks")).unwrap();
    std::fs::write(root.join("courses/X/notebooks/stray.canvas"), DOCUMENT).unwrap();
    std::fs::create_dir_all(root.join("courses/X/canvases")).unwrap();
    std::fs::write(root.join("courses/X/canvases/stray.notebook"), NOTEBOOK).unwrap();
    assert!(list_at(&root, DrawingKind::Notebook, "X")
        .unwrap()
        .is_empty());
    assert!(list_at(&root, DrawingKind::Canvas, "X").unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn notebook_files_and_folders_cannot_be_symlinks() {
    use std::os::unix::fs::symlink;
    let root = crate::test_support::Scratch::new("notebook-links");
    let outside = crate::test_support::Scratch::new("notebook-links-outside");
    std::fs::create_dir_all(root.join("courses/TEST")).unwrap();
    symlink(&*outside, root.join("courses/TEST/notebooks")).unwrap();
    assert!(create_at(&root, DrawingKind::Notebook, "TEST", "Escape", NOTEBOOK).is_err());
    assert!(list_at(&root, DrawingKind::Notebook, "TEST").is_err());
    std::fs::remove_file(root.join("courses/TEST/notebooks")).unwrap();
    let own = create_at(&root, DrawingKind::Notebook, "TEST", "Own", NOTEBOOK).unwrap();
    std::fs::write(outside.join("private.notebook"), NOTEBOOK).unwrap();
    symlink(
        outside.join("private.notebook"),
        root.join("courses/TEST/notebooks/alias.notebook"),
    )
    .unwrap();
    assert!(write_at(&root, "courses/TEST/notebooks/alias.notebook", NOTEBOOK).is_err());
    assert!(rename_at(&root, "courses/TEST/notebooks/alias.notebook", "Escape").is_err());
    assert!(write_at(&root, &own.relative_path, NOTEBOOK).is_ok());
    assert_eq!(std::fs::read_dir(&*outside).unwrap().count(), 1);
}

fn read_dir_names(root: &std::path::Path, rel: &str) -> Vec<String> {
    std::fs::read_dir(root.join(rel))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}
