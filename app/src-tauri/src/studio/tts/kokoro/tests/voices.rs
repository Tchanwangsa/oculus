use std::collections::BTreeSet;

use super::super::model::{PACK, VOICE_BYTES};
use super::super::voices::{self, Accent, Gender, VOICES};
use crate::runtime::onnx::Status;
use crate::test_support::Scratch;

/// A voice's file in `dir` at its pinned size (sparse, unread).
fn fake_voice(dir: &std::path::Path, id: &str) {
    std::fs::File::create(dir.join(format!("{id}.bin")))
        .unwrap()
        .set_len(VOICE_BYTES)
        .unwrap();
}

#[test]
fn the_catalogue_is_the_twenty_us_voices() {
    assert_eq!(VOICES.len(), 20);
    let women = VOICES.iter().filter(|v| v.gender == Gender::Female).count();
    assert_eq!((women, VOICES.len() - women), (11, 9));
    assert!(VOICES.iter().all(|v| v.accent == Accent::American));
    for v in &VOICES {
        let prefix = match v.gender {
            Gender::Female => "af_",
            Gender::Male => "am_",
        };
        assert!(v.id.starts_with(prefix), "{} is {:?}", v.id, v.gender);
        assert!(
            v.id[3..].bytes().all(|b| b.is_ascii_lowercase()),
            "{}",
            v.id
        );
        assert_eq!(v.name.to_lowercase(), v.id[3..], "{}", v.id);
        assert!(
            matches!(
                v.grade.as_bytes(),
                [b'A'..=b'F'] | [b'A'..=b'F', b'+' | b'-']
            ),
            "{}: {}",
            v.id,
            v.grade
        );
    }
}

#[test]
fn ids_files_and_pack_ids_are_unique() {
    let ids: BTreeSet<_> = VOICES.iter().map(|v| v.id).collect();
    assert_eq!(ids.len(), VOICES.len());
    let files: BTreeSet<_> = VOICES.iter().map(|v| v.pack.files[0].file).collect();
    assert_eq!(files.len(), VOICES.len());
    let mut packs: BTreeSet<_> = VOICES.iter().map(|v| v.pack.id).collect();
    assert_eq!(packs.len(), VOICES.len());
    assert!(packs.insert(PACK.id), "a voice's pack id is the model's");
}

#[test]
fn every_voice_is_one_file_of_the_pinned_size_hash_and_commit() {
    let model_commit = commit(PACK.files[0].url);
    let hashes: BTreeSet<_> = VOICES.iter().map(|v| v.pack.files[0].sha256).collect();
    assert_eq!(hashes.len(), VOICES.len(), "two voices share a hash");
    for v in &VOICES {
        assert_eq!(v.pack.files.len(), 1, "{}", v.id);
        let f = &v.pack.files[0];
        assert_eq!(f.file, format!("{}.bin", v.id));
        assert_eq!(f.bytes, 522_240, "{}", v.id);
        assert_eq!(v.bytes(), 522_240);
        assert_eq!(f.sha256.len(), 64, "{}", v.id);
        assert!(
            f.sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "{}: {}",
            v.id,
            f.sha256
        );
        assert!(
            f.url.ends_with(&format!("/voices/{}.bin", v.id)),
            "{}",
            f.url
        );
        assert_eq!(commit(f.url), model_commit, "{} is pinned elsewhere", v.id);
    }
}

fn commit(url: &str) -> &str {
    url.split('/')
        .find(|part| part.len() == 40 && part.bytes().all(|b| b.is_ascii_hexdigit()))
        .unwrap_or_else(|| panic!("{url} is not pinned"))
}

#[test]
fn the_two_starters_keep_the_hashes_they_were_pinned_with() {
    let hash = |id| voices::find(id).unwrap().pack.files[0].sha256;
    assert_eq!(
        hash("af_heart"),
        "d583ccff3cdca2f7fae535cb998ac07e9fcb90f09737b9a41fa2734ec44a8f0b"
    );
    assert_eq!(
        hash("am_michael"),
        "1d1f21dd8da39c30705cd4c75d039d265e9bc4a2a93ed09bc9e1b1225eb95ba1"
    );
}

#[test]
fn find_knows_the_catalogue_and_nothing_else() {
    assert_eq!(voices::find("am_puck").unwrap().name, "Puck");
    assert!(voices::find("bf_emma").is_none());
    assert!(voices::find("../af_heart").is_none());
}

#[test]
fn a_voice_is_present_only_at_its_size_in_its_own_file() {
    let dir = Scratch::new("kokoro-voice-status");
    let heart = voices::find("af_heart").unwrap();
    let michael = voices::find("am_michael").unwrap();
    assert_eq!(voices::status(&dir, heart), Status::Missing);

    fake_voice(&dir, "af_heart");
    assert_eq!(voices::status(&dir, heart), Status::Present);
    assert_eq!(voices::status(&dir, michael), Status::Missing);
    assert!(voices::file(&dir, "af_heart").is_some());
    assert!(voices::file(&dir, "am_michael").is_none());

    std::fs::write(dir.join("am_michael.bin"), b"cut short").unwrap();
    assert_eq!(voices::status(&dir, michael), Status::Missing);
    assert!(voices::file(&dir, "am_michael").is_none());
}

#[test]
fn a_voice_put_in_the_folder_by_hand_has_a_file_but_no_catalogue_entry() {
    let dir = Scratch::new("kokoro-voice-extra");
    fake_voice(&dir, "bf_emma");
    assert!(voices::file(&dir, "bf_emma").is_some());
    assert!(voices::find("bf_emma").is_none());
}

#[test]
fn deleting_a_voice_frees_its_file_and_part_and_leaves_the_others() {
    let dir = Scratch::new("kokoro-voice-delete");
    fake_voice(&dir, "af_heart");
    fake_voice(&dir, "am_michael");
    std::fs::write(dir.join("af_heart.bin.part"), [0u8; 10]).unwrap();
    let heart = voices::find("af_heart").unwrap();
    assert!(!voices::cancel(heart), "nothing is downloading");
    assert_eq!(voices::delete(&dir, heart).unwrap(), VOICE_BYTES + 10);
    assert_eq!(voices::status(&dir, heart), Status::Missing);
    assert!(!dir.join("af_heart.bin.part").exists());
    assert!(voices::file(&dir, "am_michael").is_some());
    assert_eq!(voices::delete(&dir, heart).unwrap(), 0);
}

/// cargo test --lib studio::tts::kokoro::tests::voices -- --ignored --nocapture
#[test]
#[ignore = "downloads all 20 voices (10 MB) from Hugging Face"]
fn every_pinned_voice_downloads_and_verifies() {
    let dir = Scratch::new("kokoro-voices-e2e");
    for v in &VOICES {
        voices::download(&dir, v, |_, _| {}, || false).unwrap_or_else(|e| panic!("{}: {e}", v.id));
        assert_eq!(voices::status(&dir, v), Status::Present, "{}", v.id);
    }
}

#[test]
fn downloading_a_voice_already_on_disk_needs_no_network() {
    let dir = Scratch::new("kokoro-voice-download");
    fake_voice(&dir, "af_sky");
    let sky = voices::find("af_sky").unwrap();
    let seen = std::sync::Mutex::new(Vec::new());
    voices::download(
        &dir,
        sky,
        |r, t| seen.lock().unwrap().push((r, t)),
        || false,
    )
    .unwrap();
    assert_eq!(
        seen.lock().unwrap().last(),
        Some(&(VOICE_BYTES, VOICE_BYTES))
    );
    assert_eq!(voices::status(&dir, sky), Status::Present);
}
