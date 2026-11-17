use std::path::Path;

use crate::library::layout::{recording_dir, recording_rel};

#[test]
fn a_recording_is_its_media_ids_folder() {
    assert_eq!(recording_rel("abc-123"), "lectures/abc-123");
    assert_eq!(
        recording_dir(Path::new("/d"), "abc-123").unwrap(),
        Path::new("/d/lectures/abc-123")
    );
}

#[test]
fn an_id_is_one_folder_name() {
    for id in ["", "..", "../x", "a/b", r"a\b", ".hidden", "C:x", "a\0"] {
        assert!(recording_dir(Path::new("/d"), id).is_err(), "{id:?}");
    }
}
