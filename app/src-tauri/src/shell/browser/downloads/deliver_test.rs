use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

use super::deliver::{deliver, move_file, remove_staged};
use super::state::Target;
use super::SavedTo;
use crate::test_support::Scratch;

/// A staged download under `<data>/downloads/.incoming/<folder>/`.
fn stage(data: &Path, folder: &str, name: &str, bytes: &[u8]) -> PathBuf {
    let dir = data.join("downloads").join(".incoming").join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    let staged = dir.join(name);
    std::fs::write(&staged, bytes).unwrap();
    staged
}

fn oculus(code: &str) -> Target {
    Target::Oculus {
        subject_code: code.into(),
    }
}

#[test]
fn any_file_lands_in_the_subjects_uploads_and_the_staged_copy_goes() {
    let data = Scratch::new("download-oculus");
    let staged = stage(&data, "1-1", "data set.zip", b"PK zip");
    let saved = deliver(&data, &staged, &oculus("COMP10001")).unwrap();
    let SavedTo::Oculus {
        subject_code,
        outcome,
    } = saved
    else {
        panic!("not an upload");
    };
    assert_eq!(subject_code, "COMP10001");
    let file = outcome.file.expect("landed");
    assert_eq!(file.relative_path, "courses/COMP10001/uploads/data_set.zip");
    assert_eq!(file.file_type, "zip");
    assert_eq!(
        outcome.error, None,
        "not an Office file, nothing to convert"
    );
    assert_eq!(
        std::fs::read(data.join(&file.relative_path)).unwrap(),
        b"PK zip"
    );
    assert!(!staged.parent().unwrap().exists(), "staging folder removed");
}

#[test]
fn uploads_rules_apply_a_taken_name_steps_aside_and_same_bytes_dedupe() {
    let data = Scratch::new("download-oculus-names");
    let rel = |saved: SavedTo| match saved {
        SavedTo::Oculus { outcome, .. } => outcome.file.unwrap().relative_path,
        SavedTo::Path { .. } => panic!("not an upload"),
    };
    let first = stage(&data, "1-1", "notes.txt", b"one");
    let again = stage(&data, "1-2", "notes.txt", b"one");
    let other = stage(&data, "1-3", "notes.txt", b"two");
    let code = oculus("MAST20004");
    assert_eq!(
        rel(deliver(&data, &first, &code).unwrap()),
        "courses/MAST20004/uploads/notes.txt"
    );
    assert_eq!(
        rel(deliver(&data, &again, &code).unwrap()),
        "courses/MAST20004/uploads/notes.txt"
    );
    assert_eq!(
        rel(deliver(&data, &other, &code).unwrap()),
        "courses/MAST20004/uploads/notes-2.txt"
    );
}

#[test]
fn a_path_delivery_moves_the_file_and_never_replaces_unasked() {
    let data = Scratch::new("download-path");
    let mac = Scratch::new("download-path-mac");
    let staged = stage(&data, "1-1", "a.pdf", b"new");
    let to = mac.join("a.pdf");
    let saved = deliver(
        &data,
        &staged,
        &Target::Path {
            path: to.clone(),
            may_replace: false,
        },
    )
    .unwrap();
    assert!(matches!(saved, SavedTo::Path { path } if path == to.to_string_lossy()));
    assert_eq!(std::fs::read(&to).unwrap(), b"new");
    assert!(!staged.parent().unwrap().exists());

    let staged = stage(&data, "1-2", "a.pdf", b"newer");
    let refused = deliver(
        &data,
        &staged,
        &Target::Path {
            path: to.clone(),
            may_replace: false,
        },
    );
    assert!(refused.unwrap_err().contains("already exists"));
    assert_eq!(std::fs::read(&to).unwrap(), b"new", "untouched");
    assert!(staged.exists(), "kept for another choice");

    deliver(
        &data,
        &staged,
        &Target::Path {
            path: to.clone(),
            may_replace: true,
        },
    )
    .unwrap();
    assert_eq!(
        std::fs::read(&to).unwrap(),
        b"newer",
        "the panel confirmed it"
    );
}

#[test]
fn a_folder_or_link_is_never_replaced() {
    let mac = Scratch::new("download-path-kinds");
    let from = mac.join("staged");
    std::fs::write(&from, b"x").unwrap();
    std::fs::create_dir(mac.join("folder")).unwrap();
    let rename = |a: &Path, b: &Path| std::fs::rename(a, b);
    assert!(move_file(&from, &mac.join("folder"), true, rename).is_err());
    #[cfg(unix)]
    {
        std::fs::write(mac.join("real"), b"mine").unwrap();
        std::os::unix::fs::symlink(mac.join("real"), mac.join("link")).unwrap();
        assert!(move_file(&from, &mac.join("link"), true, rename).is_err());
        assert_eq!(std::fs::read(mac.join("real")).unwrap(), b"mine");
    }
    assert!(from.exists());
}

#[test]
fn across_volumes_it_copies_then_deletes() {
    let mac = Scratch::new("download-exdev");
    let from = mac.join("staged.bin");
    std::fs::write(&from, b"bytes").unwrap();
    let to = mac.join("out.bin");
    let other_volume = |_: &Path, _: &Path| Err(Error::from(ErrorKind::CrossesDevices));
    move_file(&from, &to, false, other_volume).unwrap();
    assert_eq!(std::fs::read(&to).unwrap(), b"bytes");
    assert!(!from.exists(), "the original goes");
    let names: Vec<_> = std::fs::read_dir(&*mac)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["out.bin"], "no partial copy left behind");

    std::fs::write(&from, b"bytes").unwrap();
    let denied = |_: &Path, _: &Path| Err(Error::from(ErrorKind::PermissionDenied));
    assert!(move_file(&from, &mac.join("other.bin"), false, denied).is_err());
    assert!(from.exists(), "any other failure keeps the staged file");
}

#[test]
fn only_a_staging_folder_is_ever_removed() {
    let data = Scratch::new("download-remove");
    let outside = data.join("courses").join("X").join("keep.pdf");
    std::fs::create_dir_all(outside.parent().unwrap()).unwrap();
    std::fs::write(&outside, b"keep").unwrap();
    remove_staged(&outside);
    assert!(outside.exists());

    let staged = stage(&data, "1-1", "a.pdf", b"x");
    remove_staged(&staged);
    assert!(!staged.parent().unwrap().exists());
    assert!(data.join("downloads").join(".incoming").exists());
}
