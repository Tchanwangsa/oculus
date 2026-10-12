use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::super::kokoro::{packs_status, sample, SAMPLE_TEXT};
use crate::runtime::onnx::{Pack, Status};
use crate::studio::audio::decode_wav;
use crate::studio::tts::kokoro::{self, LexiconFiles, Packs, PACK};
use crate::studio::voice::g2p::guess;
use crate::test_support::Scratch;

/// `kokoro::download`'s in-flight flag is process-wide: the tests that read
/// or set it take turns.
static DOWNLOAD_FLAG: Mutex<()> = Mutex::new(());

/// Every file of `pack` in `dir` at its pinned size (sparse, unread).
fn fake_pack(dir: &Path, pack: &Pack) {
    for f in pack.files {
        std::fs::File::create(dir.join(f.file))
            .unwrap()
            .set_len(f.bytes)
            .unwrap();
    }
}

fn packs(dir: &Path) -> Packs {
    Packs {
        voice: dir.to_owned(),
        g2p: dir.to_owned(),
    }
}

#[test]
fn the_status_reports_both_packs_and_the_voices_on_disk() {
    let _turn = DOWNLOAD_FLAG.lock().unwrap_or_else(|e| e.into_inner());
    let dir = Scratch::new("voice-packs-status");
    let missing = packs_status(&dir);
    assert_eq!(missing.status, Status::Missing);
    assert_eq!(missing.bytes, PACK.bytes() + guess::PACK.bytes());
    let ids: Vec<_> = missing.packs.iter().map(|p| (p.id, p.status)).collect();
    assert_eq!(
        ids,
        [
            ("voice-kokoro", Status::Missing),
            ("g2p-en-us", Status::Missing)
        ]
    );
    let voices: Vec<_> = missing
        .voices
        .iter()
        .map(|v| (v.id.as_str(), v.downloaded))
        .collect();
    assert_eq!(voices, [("af_heart", false), ("am_michael", false)]);

    fake_pack(&dir, &guess::PACK);
    let half = packs_status(&dir);
    assert_eq!(half.status, Status::Missing, "one pack is not enough");
    assert_eq!(half.packs[1].status, Status::Present);

    fake_pack(&dir, &PACK);
    std::fs::File::create(dir.join("bf_emma.bin"))
        .unwrap()
        .set_len(kokoro::VOICE_BYTES)
        .unwrap();
    let all = packs_status(&dir);
    assert_eq!(all.status, Status::Present);
    let json = serde_json::to_value(&all).unwrap();
    assert_eq!(json["status"], "present");
    assert_eq!(json["packs"][0]["id"], "voice-kokoro");
    assert_eq!(
        json["voices"],
        serde_json::json!([
            { "id": "af_heart", "downloaded": true },
            { "id": "am_michael", "downloaded": true },
            { "id": "bf_emma", "downloaded": true },
        ])
    );
}

#[test]
fn a_download_of_packs_on_disk_needs_no_network_and_frees_its_flag() {
    let _turn = DOWNLOAD_FLAG.lock().unwrap_or_else(|e| e.into_inner());
    let dir = Scratch::new("voice-packs-download");
    fake_pack(&dir, &PACK);
    fake_pack(&dir, &guess::PACK);
    assert!(!kokoro::cancel(), "nothing is downloading");
    let seen = Mutex::new(Vec::new());
    kokoro::download(&dir, |r, t| seen.lock().unwrap().push((r, t)), || false).unwrap();
    let total = PACK.bytes() + guess::PACK.bytes();
    assert_eq!(seen.lock().unwrap().last(), Some(&(total, total)));
    assert!(!kokoro::downloading());
    assert!(!kokoro::cancel());

    let freed = kokoro::delete(&dir).unwrap();
    assert_eq!(freed, total);
    assert_eq!(packs_status(&dir).status, Status::Missing);
}

#[test]
fn a_sample_refuses_a_bad_voice_or_speed_and_missing_packs() {
    let dir = Scratch::new("voice-sample");
    let lexicon = LexiconFiles::default();
    let e = sample(" ", 1.0, None, packs(&dir), lexicon.clone()).unwrap_err();
    assert!(e.contains("choose a voice"), "{e}");
    let e = sample("../af_heart", 1.0, None, packs(&dir), lexicon.clone()).unwrap_err();
    assert!(e.contains("not a voice id"), "{e}");
    for speed in [0.0, -1.0, f64::NAN] {
        let e = sample("af_heart", speed, None, packs(&dir), lexicon.clone()).unwrap_err();
        assert!(e.contains("positive"), "{e}");
    }
    let e = sample("af_heart", 1.0, None, packs(&dir), lexicon.clone()).unwrap_err();
    assert!(
        e.contains("voice pack") && e.contains("Settings -> Studio -> Narration"),
        "{e}"
    );
    fake_pack(&dir, &PACK);
    fake_pack(&dir, &guess::PACK);
    let e = sample("bf_emma", 1.0, None, packs(&dir), lexicon).unwrap_err();
    assert!(e.contains("`bf_emma`"), "{e}");
}

#[test]
fn the_sample_sentence_is_spoken_form() {
    assert!(SAMPLE_TEXT.chars().all(|c| !c.is_ascii_digit() && c != '$'));
}

fn env_dir(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var(name).unwrap_or_else(|_| panic!("set {name}")));
    assert!(path.exists(), "{name}: {} does not exist", path.display());
    path
}

/// STUDIO_KOKORO=<voice-kokoro folder> STUDIO_G2P_GUESSER=<g2p-en-us folder>
/// cargo test --lib studio::commands::tests::kokoro -- --ignored --nocapture
#[test]
#[ignore = "needs STUDIO_KOKORO and STUDIO_G2P_GUESSER"]
fn a_sample_is_a_24khz_wav_of_the_sentence() {
    let packs = Packs {
        voice: env_dir("STUDIO_KOKORO"),
        g2p: env_dir("STUDIO_G2P_GUESSER"),
    };
    for (voice, speed) in [("af_heart", 1.0), ("am_michael", 1.25)] {
        let started = std::time::Instant::now();
        let wav = sample(voice, speed, None, packs.clone(), LexiconFiles::default()).unwrap();
        let decoded = decode_wav(&wav).unwrap();
        println!(
            "{voice} at {speed}×: {:.2} s of audio in {:.2} s, {} KB",
            decoded.duration,
            started.elapsed().as_secs_f64(),
            wav.len() / 1024
        );
        assert_eq!(decoded.sample_rate, kokoro::SAMPLE_RATE);
        assert!(decoded.duration > 3.0, "{}", decoded.duration);
    }
}
