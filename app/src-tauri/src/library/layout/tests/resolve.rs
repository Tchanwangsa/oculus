use crate::library::layout::{resolve, resolve_absolute};
use crate::test_support::Scratch;

#[test]
fn a_library_file_resolves() {
    let dir = Scratch::new("layout-resolve");
    std::fs::create_dir_all(dir.join("courses/X/files")).unwrap();
    std::fs::write(dir.join("courses/X/files/a.md"), "a").unwrap();
    std::fs::create_dir_all(dir.join("agents")).unwrap();
    std::fs::write(dir.join("agents/notes.md"), "n").unwrap();
    assert!(resolve(&dir, "courses/X/files/a.md").is_ok());
    assert!(resolve(&dir, "agents/notes.md").is_ok());
    assert!(resolve(&dir, "courses/X/files/missing.md").is_err());
    assert!(resolve(&dir, "../outside").is_err());
    assert!(resolve(&dir, "/etc/hosts").is_err());
}

#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_data_dir_is_refused() {
    let dir = Scratch::new("layout-resolve-link");
    let outside = Scratch::new("layout-resolve-outside");
    std::fs::write(outside.join("secret.txt"), "s").unwrap();
    std::fs::create_dir_all(dir.join("courses/X/uploads")).unwrap();
    std::os::unix::fs::symlink(
        outside.join("secret.txt"),
        dir.join("courses/X/uploads/link.txt"),
    )
    .unwrap();
    // A link that stays inside (a subject's memories) is fine.
    std::fs::create_dir_all(dir.join("agents/memories/X")).unwrap();
    std::fs::write(dir.join("agents/memories/X/m.md"), "m").unwrap();
    std::fs::create_dir_all(dir.join("courses/X/agents")).unwrap();
    std::os::unix::fs::symlink(
        "../../../agents/memories/X",
        dir.join("courses/X/agents/memories"),
    )
    .unwrap();

    assert!(resolve(&dir, "courses/X/uploads/link.txt").is_err());
    assert!(resolve(&dir, "courses/X/agents/memories/m.md").is_ok());
    assert!(resolve_absolute(&dir, &dir.join("courses/X/uploads/link.txt")).is_err());
}

#[test]
fn a_stored_absolute_path_resolves_only_inside() {
    let dir = Scratch::new("layout-resolve-abs");
    std::fs::create_dir_all(dir.join("lectures/abc")).unwrap();
    std::fs::write(dir.join("lectures/abc/transcript.vtt"), "WEBVTT").unwrap();
    let outside = Scratch::new("layout-resolve-abs-outside");
    std::fs::write(outside.join("x.vtt"), "WEBVTT").unwrap();
    assert!(resolve_absolute(&dir, &dir.join("lectures/abc/transcript.vtt")).is_ok());
    assert!(resolve_absolute(&dir, &outside.join("x.vtt")).is_err());
    assert!(resolve_absolute(&dir, &dir).is_err());
    assert!(resolve_absolute(&dir, std::path::Path::new("lectures/abc/transcript.vtt")).is_err());
}
