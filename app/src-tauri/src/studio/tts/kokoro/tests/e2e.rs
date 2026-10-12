//! A two-scene video narrated through `narrate()` with Kokoro and the
//! aligner, then again from the cache, then after a lexicon edit that
//! changes one scene's phonemes. Ignored by default; run with the packs:
//!
//! STUDIO_KOKORO=<voice-kokoro folder> STUDIO_G2P_GUESSER=<g2p-en-us folder>
//! STUDIO_ALIGN_MODEL=<wav2vec2 .onnx> cargo test --lib
//! studio::tts::kokoro::tests::e2e -- --ignored --nocapture
//!
//! It prints each scene's real-time factor, the alignment's confidences,
//! the guessed words and the process's peak RSS.

use std::path::{Path, PathBuf};

use serde_json::json;
use studio_core::{Library, Manifest};

use super::super::{KokoroSpeaker, LexiconFiles, Packs};
use crate::studio::narrate::{narrate, ModelAligner, Options, Outcome, Report};
use crate::studio::voice::g2p::Source;
use crate::studio::voice::KokoroVoice;
use crate::test_support::Scratch;

const SCRIPT: &str = "## descent\nGradient descent takes a small step downhill, again and again, until the loss stops falling. Each step follows the slope we measured a moment ago.\n\n## momentum\nNext time we meet Adam, Nesterov and RMSProp, three optimisers that adapt their step size as they go. None of them needs a schedule tuned by hand.\n";

fn env_dir(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var(name).unwrap_or_else(|_| panic!("set {name}")));
    assert!(path.exists(), "{name}: {} does not exist", path.display());
    path
}

fn video(dir: &Path) {
    std::fs::write(
        dir.join("video.json"),
        json!({
            "size": { "width": 64, "height": 36 }, "theme": "default",
            "scenes": [
                { "id": "descent", "component": "Card" },
                { "id": "momentum", "component": "Card" }
            ]
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(dir.join("script.md"), SCRIPT).unwrap();
}

fn speaker(packs: &Packs, lexicon: &Path) -> KokoroSpeaker {
    KokoroSpeaker::new(
        KokoroVoice {
            voice: "af_heart".into(),
            speed: 1.0,
        },
        packs.clone(),
        LexiconFiles {
            library: Some(lexicon.to_owned()),
            subject: None,
        },
    )
}

fn run(dir: &Path, speaker: &KokoroSpeaker) -> Report {
    narrate(
        dir,
        &Library::default(),
        speaker,
        &ModelAligner,
        &Options::default(),
        &mut |_| {},
    )
    .unwrap()
}

/// The process's peak resident set so far, in bytes.
fn peak_rss() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage fills the struct it is given.
    let usage = unsafe {
        libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr());
        usage.assume_init()
    };
    let max = usage.ru_maxrss as u64;
    if cfg!(target_os = "macos") {
        max
    } else {
        max * 1024
    }
}

fn outcomes(report: &Report) -> Vec<(&str, Outcome)> {
    report
        .scenes
        .iter()
        .map(|s| (s.id.as_str(), s.outcome))
        .collect()
}

#[test]
#[ignore = "needs STUDIO_KOKORO, STUDIO_G2P_GUESSER and STUDIO_ALIGN_MODEL"]
fn narrates_a_video_with_kokoro() {
    let packs = Packs {
        voice: env_dir("STUDIO_KOKORO"),
        g2p: env_dir("STUDIO_G2P_GUESSER"),
    };
    env_dir("STUDIO_ALIGN_MODEL");
    let dir = Scratch::new("kokoro-e2e");
    video(&dir);
    let lexicon = dir.join("lexicon.json");
    std::fs::write(&lexicon, r#"{"RMSProp": "/ˌɑɹˌɛmˈɛs pɹˈɑp/"}"#).unwrap();

    let first = speaker(&packs, &lexicon);
    let started = std::time::Instant::now();
    let report = run(&dir, &first);
    let wall = started.elapsed().as_secs_f64();
    assert_eq!(
        outcomes(&report),
        [
            ("descent", Outcome::Narrated),
            ("momentum", Outcome::Narrated)
        ]
    );
    let manifest = Manifest::read(&dir).unwrap().unwrap();
    let (mut audio, mut synth, mut align) = (0.0, 0.0, 0.0);
    for scene in &report.scenes {
        let t = scene.timings.unwrap();
        let entry = &manifest.scenes[&scene.id];
        let confidences: Vec<f64> = entry.words.iter().map(|w| w.confidence).collect();
        let mean = confidences.iter().sum::<f64>() / confidences.len() as f64;
        let min = confidences.iter().copied().fold(f64::INFINITY, f64::min);
        println!(
            "{}: {:.2} s of audio, synth {:.2} s (RTF {:.3}), align {:.2} s, {} words, confidence mean {mean:.2} min {min:.2}, {} unsure, {} gaps",
            scene.id,
            scene.duration,
            t.synth_ms as f64 / 1000.0,
            t.synth_ms as f64 / 1000.0 / scene.duration,
            t.align_ms as f64 / 1000.0,
            scene.words,
            scene.low_confidence.len(),
            scene.gaps.len()
        );
        assert_eq!(
            entry.words.len(),
            entry.text.split_whitespace().count(),
            "every word is timed"
        );
        assert!(scene.duration > 3.0, "{}", scene.duration);
        audio += scene.duration;
        synth += t.synth_ms as f64 / 1000.0;
        align += t.align_ms as f64 / 1000.0;
        let read = first.phonemized(&entry.text).unwrap();
        let guessed: Vec<String> = read
            .words
            .iter()
            .filter(|w| w.source == Source::Guess)
            .map(|w| format!("{} /{}/", w.word, w.phonemes))
            .collect();
        println!("  guessed: {guessed:?}");
        let rms = read
            .words
            .iter()
            .find(|w| w.word == "RMSProp")
            .map(|w| w.source);
        if scene.id == "momentum" {
            assert_eq!(rms, Some(Source::Lexicon));
        }
    }
    println!(
        "total: {audio:.2} s of audio, synth {synth:.2} s (RTF {:.3}), align {align:.2} s, wall {wall:.2} s, peak RSS {:.2} GB",
        synth / audio,
        peak_rss() as f64 / 1e9
    );
    drop(first);

    let again = speaker(&packs, &lexicon);
    let report = run(&dir, &again);
    assert_eq!(
        outcomes(&report),
        [("descent", Outcome::Cached), ("momentum", Outcome::Cached)],
        "the same phonemes key the same clips"
    );

    std::fs::write(
        &lexicon,
        r#"{"RMSProp": "/ˌɑɹˌɛmˈɛs pɹˈɑp/", "downhill": "DOWN-hill"}"#,
    )
    .unwrap();
    let edited = speaker(&packs, &lexicon);
    let report = run(&dir, &edited);
    assert_eq!(
        outcomes(&report),
        [
            ("descent", Outcome::Narrated),
            ("momentum", Outcome::Cached)
        ],
        "a lexicon edit re-narrates only the scene it changes"
    );
    println!(
        "after the lexicon edit: peak RSS {:.2} GB",
        peak_rss() as f64 / 1e9
    );
}
