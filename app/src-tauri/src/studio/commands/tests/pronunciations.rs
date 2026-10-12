use std::path::PathBuf;

use super::super::pronunciations::{sample_record, Which};
use crate::studio::audio::decode_wav;
use crate::studio::tts::kokoro::{LexiconFiles, Packs};
use crate::studio::voice::review::{Record, Status};
use crate::studio::voice::VoiceSettings;
use crate::test_support::Scratch;

fn record() -> Record {
    Record {
        word: "Nesterov".into(),
        reading: "NEST-er-off".into(),
        original: "nˈɛstəɹɔv".into(),
        guessed: true,
        subject: None,
        video: "momentum".into(),
        sentence: "Nesterov adds momentum to the step.".into(),
        scene: "intro".into(),
        line: 2,
        status: Status::Pending,
        note: None,
        created: "2026-10-12T00:00:00".into(),
    }
}

fn settings(voice: Option<&str>) -> VoiceSettings {
    VoiceSettings {
        voice: voice.map(str::to_owned),
        ..VoiceSettings::default()
    }
}

#[test]
fn a_sample_needs_a_voice_in_settings_and_the_packs() {
    let dir = Scratch::new("pronunciation-sample");
    let packs = Packs {
        voice: dir.to_path_buf(),
        g2p: dir.to_path_buf(),
    };
    let e = sample_record(
        &record(),
        Which::Original,
        &settings(None),
        packs.clone(),
        LexiconFiles::default(),
    )
    .unwrap_err();
    assert!(e.contains("choose a voice"), "{e}");
    let e = sample_record(
        &record(),
        Which::Suggestion,
        &settings(Some("af_heart")),
        packs,
        LexiconFiles::default(),
    )
    .unwrap_err();
    assert!(e.contains("Settings -> Studio -> Pronunciation"), "{e}");
}

#[test]
fn the_verdicts_and_sample_choices_parse_from_the_frontend() {
    use crate::studio::voice::review::Decision;
    assert_eq!(
        serde_json::from_str::<Which>("\"original\"").unwrap(),
        Which::Original
    );
    assert_eq!(
        serde_json::from_str::<Decision>("\"fine\"").unwrap(),
        Decision::Fine
    );
    assert!(serde_json::from_str::<Decision>("\"wrong\"").is_err());
}

fn env_dir(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var(name).unwrap_or_else(|_| panic!("set {name}")));
    assert!(path.exists(), "{name}: {} does not exist", path.display());
    path
}

/// STUDIO_KOKORO=<voice-kokoro folder> STUDIO_G2P_GUESSER=<g2p-en-us folder>
/// cargo test --lib studio::commands::tests::pronunciations -- --ignored
#[test]
#[ignore = "needs STUDIO_KOKORO and STUDIO_G2P_GUESSER"]
fn the_two_samples_read_the_word_two_ways() {
    let packs = Packs {
        voice: env_dir("STUDIO_KOKORO"),
        g2p: env_dir("STUDIO_G2P_GUESSER"),
    };
    let wav = |which| {
        let bytes = sample_record(
            &record(),
            which,
            &settings(Some("af_heart")),
            packs.clone(),
            LexiconFiles::default(),
        )
        .unwrap();
        decode_wav(&bytes).unwrap()
    };
    let (original, suggestion) = (wav(Which::Original), wav(Which::Suggestion));
    assert!(original.duration > 1.0, "{}", original.duration);
    assert_ne!(original.samples, suggestion.samples);
}
