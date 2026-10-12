use super::super::model::{MODEL_FILE, PACK, STYLE_ROWS, STYLE_WIDTH, VOICES, VOICE_BYTES};

#[test]
fn every_file_is_pinned_to_one_commit() {
    let commits: std::collections::BTreeSet<&str> = PACK
        .files
        .iter()
        .map(|f| {
            assert_eq!(f.sha256.len(), 64, "{}", f.file);
            f.url
                .split('/')
                .find(|part| part.len() == 40 && part.bytes().all(|b| b.is_ascii_hexdigit()))
                .unwrap_or_else(|| panic!("{} is not pinned", f.url))
        })
        .collect();
    assert_eq!(commits.len(), 1);
}

#[test]
fn the_pack_holds_the_model_and_every_voice() {
    assert!(PACK.files.iter().any(|f| f.file == MODEL_FILE));
    for voice in VOICES {
        let file = PACK
            .files
            .iter()
            .find(|f| f.file == format!("{voice}.bin"))
            .unwrap_or_else(|| panic!("{voice} is not in the pack"));
        assert_eq!(file.bytes, VOICE_BYTES);
    }
    assert_eq!(VOICE_BYTES, 522_240);
    assert_eq!(STYLE_ROWS * STYLE_WIDTH * 4, 522_240);
}
