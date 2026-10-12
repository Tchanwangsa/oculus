use super::super::{delete, model_file, status, Status, PACK};
use crate::test_support::Scratch;

#[test]
fn the_model_is_pinned_to_a_commit_with_its_size_and_checksum() {
    let file = &PACK.files[0];
    assert!(file
        .url
        .contains("/resolve/a19f851b3d42865797e410752b4c570c871e4825/onnx/model.onnx"));
    assert_eq!(file.bytes, 377_887_594);
    assert_eq!(file.sha256.len(), 64);
}

#[test]
fn status_reads_the_directory_and_a_wrong_size_file_is_missing() {
    let dir = Scratch::new("align-model-status");
    assert_eq!(status(&dir), Status::Missing);
    assert_eq!(model_file(&dir), None);
    let path = dir.join(PACK.files[0].file);
    std::fs::write(&path, b"truncated").unwrap();
    assert_eq!(status(&dir), Status::Missing);
    // A file of the pinned size reads as present: the checksum was checked
    // before the download gave it its name.
    std::fs::File::create(&path)
        .unwrap()
        .set_len(PACK.files[0].bytes)
        .unwrap();
    assert_eq!(status(&dir), Status::Present);
    assert_eq!(model_file(&dir), Some(path));
    assert_eq!(delete(&dir).unwrap(), PACK.files[0].bytes);
    assert_eq!(status(&dir), Status::Missing);
}
