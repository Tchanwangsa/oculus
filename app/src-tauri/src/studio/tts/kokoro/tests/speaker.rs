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
fn ready_needs_both_packs_and_the_voice() {
    let dir = Scratch::new("kokoro-ready");
    let e = speaker(&dir, "af_heart").ready().unwrap_err();
    assert!(
        e.contains("voice pack") && e.contains("Settings -> Studio -> Narration"),
        "{e}"
    );
    fake_pack(&dir, &PACK);
    let e = speaker(&dir, "af_heart").ready().unwrap_err();
    assert!(e.contains("G2P pack"), "{e}");
    fake_pack(&dir, &guess::PACK);
    speaker(&dir, "af_heart").ready().unwrap();
    speaker(&dir, "am_michael").ready().unwrap();
    let e = speaker(&dir, "bf_emma").ready().unwrap_err();
    assert!(
        e.contains("`bf_emma`") && e.contains("af_heart and am_michael"),
        "{e}"
    );
    std::fs::File::create(dir.join("bf_emma.bin"))
        .unwrap()
        .set_len(VOICE_BYTES)
        .unwrap();
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
