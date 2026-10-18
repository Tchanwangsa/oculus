use super::*;
const DOCUMENT: &str = r#"{"type":"oculus-canvas","version":1,"elements":[],"viewport":{"x":0,"y":0,"zoom":1},"background":"dots"}"#;

#[test]
fn canvas_lifecycle_is_atomic_and_scoped() {
    let root = crate::test_support::Scratch::new("canvas-files");
    let first = create_at(&root, "TEST", "Ideas", DOCUMENT).unwrap();
    let second = create_at(&root, "TEST", "Ideas", DOCUMENT).unwrap();
    assert_ne!(first.relative_path, second.relative_path);
    assert_eq!(list_at(&root, "TEST").unwrap().len(), 2);
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
    assert!(create_at(&root, "TEST", "Escape", DOCUMENT).is_err());
    assert!(list_at(&root, "TEST").is_err());
    std::fs::remove_file(root.join("courses/TEST/canvases")).unwrap();
    let own = create_at(&root, "TEST", "Own", DOCUMENT).unwrap();
    std::fs::write(outside.join("private.canvas"), DOCUMENT).unwrap();
    symlink(
        outside.join("private.canvas"),
        root.join("courses/TEST/canvases/alias.canvas"),
    )
    .unwrap();
    assert!(write_at(&root, "courses/TEST/canvases/alias.canvas", DOCUMENT).is_err());
    assert!(rename_at(&root, "courses/TEST/canvases/alias.canvas", "Escape").is_err());
    assert!(write_at(&root, &own.relative_path, DOCUMENT).is_ok());
    assert_eq!(std::fs::read_dir(&*outside).unwrap().count(), 1);
}
