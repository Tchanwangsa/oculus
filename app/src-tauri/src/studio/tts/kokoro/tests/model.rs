use super::super::model::{MODEL_FILE, PACK, STYLE_ROWS, STYLE_WIDTH, VOICE_BYTES};

#[test]
fn the_pack_is_the_model_pinned_to_a_commit() {
    assert_eq!(PACK.files.len(), 1);
    let file = &PACK.files[0];
    assert_eq!(file.file, MODEL_FILE);
    assert_eq!(file.sha256.len(), 64);
    assert!(
        file.url
            .split('/')
            .any(|part| part.len() == 40 && part.bytes().all(|b| b.is_ascii_hexdigit())),
        "{} is not pinned",
        file.url
    );
}

#[test]
fn a_voice_file_is_a_style_row_per_input_length() {
    assert_eq!(VOICE_BYTES, 522_240);
    assert_eq!(STYLE_ROWS * STYLE_WIDTH * 4, 522_240);
}
