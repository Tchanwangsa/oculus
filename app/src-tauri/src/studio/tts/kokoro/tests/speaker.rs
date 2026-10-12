use std::path::Path;

use super::super::model::{PACK, VOICE_BYTES};
use super::super::{KokoroSpeaker, LexiconFiles, Packs};
use crate::runtime::onnx::Pack;
use crate::studio::narrate::Speaker;
use crate::studio::voice::g2p::guess;
use crate::studio::voice::KokoroVoice;
use crate::test_support::Scratch;

/// Every file of `pack` in `dir` at its pinned size (sparse, unread).
fn fake_pack(dir: &Path, pack: &Pack) {
    for f in pack.files {
        std::fs::File::create(dir.join(f.file))
            .unwrap()
            .set_len(f.bytes)
            .unwrap();
    }
}

/// `voice`'s style file in `dir` at a voice file's size (sparse, unread).
fn fake_voice(dir: &Path, voice: &str) {
    std::fs::File::create(dir.join(format!("{voice}.bin")))
        .unwrap()
        .set_len(VOICE_BYTES)
        .unwrap();
}

fn speaker(dir: &Path, voice: &str) -> KokoroSpeaker {
    KokoroSpeaker::new(
        KokoroVoice {
            voice: voice.into(),
            speed: 1.0,
        },
        Packs {
            voice: dir.to_owned(),
            g2p: dir.to_owned(),
        },
        LexiconFiles::default(),
    )
}

#[test]
fn ready_needs_the_model_the_g2p_pack_and_the_chosen_voice() {
    let dir = Scratch::new("kokoro-ready");
    let e = speaker(&dir, "af_heart").ready().unwrap_err();
    assert!(
        e.contains("model (voice-kokoro)") && e.contains("Settings -> Studio -> Narration"),
        "{e}"
    );
    fake_pack(&dir, &PACK);
    let e = speaker(&dir, "af_heart").ready().unwrap_err();
    assert!(e.contains("G2P pack"), "{e}");
    fake_pack(&dir, &guess::PACK);
    let e = speaker(&dir, "af_heart").ready().unwrap_err();
    assert!(
        e.contains("`af_heart`") && e.contains("voices download af_heart"),
        "the model and G2P are not enough without the voice: {e}"
    );
    fake_voice(&dir, "af_heart");
    speaker(&dir, "af_heart").ready().unwrap();
    let e = speaker(&dir, "am_michael").ready().unwrap_err();
    assert!(e.contains("voices download am_michael"), "{e}");
    let e = speaker(&dir, "bf_emma").ready().unwrap_err();
    assert!(
        e.contains("`bf_emma`") && e.contains("not one of Kokoro's US voices"),
        "{e}"
    );
    fake_voice(&dir, "bf_emma");
    speaker(&dir, "bf_emma").ready().unwrap();
}

#[test]
fn keying_needs_the_g2p_pack() {
    let dir = Scratch::new("kokoro-key");
    let e = speaker(&dir, "af_heart").key("Hello there.").unwrap_err();
    assert!(
        e.contains("G2P pack") && e.contains("Settings -> Studio -> Narration"),
        "{e}"
    );
}

#[test]
fn a_dollar_in_the_text_is_refused_before_anything_loads() {
    let dir = Scratch::new("kokoro-pad");
    let s = speaker(&dir, "af_heart");
    for e in [
        s.key("It costs $5.").unwrap_err(),
        s.speak("It costs $5.").err().unwrap(),
    ] {
        assert!(e.contains("`$`") && e.contains("pad"), "{e}");
    }
}
