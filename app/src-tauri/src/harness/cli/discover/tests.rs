use super::oculus_bin::{dev_checkout_src, newest_rs_mtime};
use crate::test_support::Scratch;
use std::path::Path;

fn scratch(name: &str) -> Scratch {
    Scratch::new(&format!("discover-{name}"))
}

#[test]
fn only_a_checkout_has_sources_to_be_behind() {
    let root = scratch("shapes");
    std::fs::create_dir_all(root.join("src")).unwrap();
    for profile in ["debug", "release"] {
        std::fs::create_dir_all(root.join("target").join(profile)).unwrap();
        let bin = root.join("target").join(profile).join("oculus");
        assert_eq!(dev_checkout_src(&bin), Some(root.join("src")));
    }

    // A bundle's sidecar, and a cargo layout with no sources beside it.
    assert_eq!(
        dev_checkout_src(Path::new("/Applications/Oculus.app/Contents/MacOS/oculus")),
        None
    );
    let bare = scratch("bare");
    std::fs::create_dir_all(bare.join("target/debug")).unwrap();
    assert_eq!(dev_checkout_src(&bare.join("target/debug/oculus")), None);
}

#[test]
fn a_touched_source_is_newer_than_the_binary() {
    let root = scratch("mtime");
    std::fs::create_dir_all(root.join("src/harness")).unwrap();
    std::fs::create_dir_all(root.join("target/debug")).unwrap();
    let bin = root.join("target/debug/oculus");
    std::fs::write(&bin, b"binary").unwrap();
    let built = std::fs::metadata(&bin).unwrap().modified().unwrap();

    // Nested, so the walk must recurse, and written after the binary.
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(root.join("src/harness/discover.rs"), b"fn main() {}").unwrap();
    let newest = newest_rs_mtime(&root.join("src")).unwrap();
    assert!(newest > built, "an edit after the build reads as newer");

    // Non-Rust files are not what cargo rebuilds from.
    std::fs::remove_file(root.join("src/harness/discover.rs")).unwrap();
    std::fs::write(root.join("src/notes.md"), b"# not a rebuild").unwrap();
    assert_eq!(newest_rs_mtime(&root.join("src")), None);
}
