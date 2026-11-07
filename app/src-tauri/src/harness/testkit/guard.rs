use std::path::{Component, Path, PathBuf};

fn live_dir() -> PathBuf {
    dirs::home_dir()
        .expect("home for forbidden-path guard")
        .join("Library/Application Support/com.tchan.oculus")
}

fn allowed(path: &Path) {
    assert!(
        !path.starts_with(live_dir()),
        "replay path reaches live data: {}",
        path.display()
    );
    assert!(
        !path.starts_with("/Users/tanatchanwangsa/Documents/gh/oculus"),
        "replay path reaches main checkout"
    );
}

/// Resolve symlinks and nonexistent tails without opening any target file.
/// Check each prefix before looking up a link, including links followed by `..`.
pub(super) fn assert_scratch(path: &Path, scratch: &Path) -> PathBuf {
    let resolved = resolve(path, 0);
    let root = resolve(scratch, 0);
    assert!(
        resolved.starts_with(&root),
        "path escapes replay scratch: {}",
        resolved.display()
    );
    resolved
}

fn resolve(path: &Path, depth: usize) -> PathBuf {
    assert!(depth < 64, "replay symlink loop");
    assert!(path.is_absolute(), "replay paths must be absolute");
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
            _ => resolved.push(component.as_os_str()),
        }
        allowed(&resolved);
        if let Ok(target) = std::fs::read_link(&resolved) {
            let target = if target.is_absolute() {
                target
            } else {
                resolved.parent().unwrap().join(target)
            };
            resolved = resolve(&target, depth + 1);
        }
    }
    allowed(&resolved);
    resolved
}

pub(super) fn scratch(name: &str) -> crate::test_support::Scratch {
    let temp = std::env::temp_dir();
    assert_scratch(&temp, &temp);
    let scratch = crate::test_support::Scratch::new(name);
    assert_scratch(&scratch, &scratch);
    scratch
}

#[test]
fn scratch_guard_rejects_live_and_escaped_paths_without_opening_them() {
    let scratch = scratch("replay-guard");
    let forbidden = live_dir().join("oculus.db");
    for path in [
        forbidden.clone(),
        scratch.join("../outside/oculus.db"),
        PathBuf::from("/Users/tanatchanwangsa/Documents/gh/oculus/app"),
    ] {
        assert!(std::panic::catch_unwind(|| assert_scratch(&path, &scratch)).is_err());
    }
    #[cfg(unix)]
    {
        let link = scratch.join("link");
        std::os::unix::fs::symlink(&forbidden, &link).unwrap();
        assert!(std::panic::catch_unwind(|| assert_scratch(&link, &scratch)).is_err());
        let nested = scratch.join("nested");
        std::os::unix::fs::symlink(&scratch, &nested).unwrap();
        assert_eq!(
            assert_scratch(&nested.join("new.db"), &scratch),
            resolve(&scratch, 0).join("new.db")
        );
    }
    assert_eq!(
        assert_scratch(&scratch.join("oculus.db"), &scratch),
        resolve(&scratch, 0).join("oculus.db")
    );
}
