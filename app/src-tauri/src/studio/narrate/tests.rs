use std::cell::{Cell, RefCell};
use std::path::Path;

use serde_json::json;

use super::*;
use crate::studio::audio::decode_wav;
use crate::studio::libraries::libraries_at;
use crate::studio::test_wav::tone;
use crate::studio::timing::{Gap, WordTime};
use crate::test_support::Scratch;

/// Words spread evenly over the clip; `Cauchy` comes out unsure, and a
/// script starting `Um` gets a gap before its first word.
struct Stub {
    ready: bool,
    calls: Cell<usize>,
}

impl Stub {
    fn new() -> Self {
        Self {
            ready: true,
            calls: Cell::new(0),
        }
    }
}

impl Aligner for Stub {
    fn ready(&self) -> Result<(), NarrationError> {
        if self.ready {
            Ok(())
        } else {
            Err(NarrationError::ModelMissing)
        }
    }

    fn align(&self, pcm: &[f32], words: &[String]) -> Result<Alignment, NarrationError> {
        self.calls.set(self.calls.get() + 1);
        let step = pcm.len() as f64 / 16_000.0 / words.len() as f64;
        let words = words
            .iter()
            .enumerate()
            .map(|(i, w)| WordTime {
                index: i,
                text: w.clone(),
                start: i as f64 * step,
                end: (i + 1) as f64 * step,
                confidence: if w.starts_with("Cauchy") { -11.0 } else { -0.5 },
            })
            .collect::<Vec<_>>();
        let gaps = if words[0].text.starts_with("Um") {
            vec![Gap {
                after: None,
                start: 0.0,
                end: 0.1,
                heard: "UM".into(),
            }]
        } else {
            Vec::new()
        };
        Ok(Alignment { words, gaps })
    }
}

/// A clip of 0.1 s per word of text; a text with `broken` in it fails.
/// `texts` is every text it was asked to speak, in order.
struct FakeSpeaker {
    ready: bool,
    texts: RefCell<Vec<String>>,
}

impl FakeSpeaker {
    fn new() -> Self {
        Self {
            ready: true,
            texts: RefCell::new(Vec::new()),
        }
    }

    fn spoken(&self) -> Vec<String> {
        self.texts.borrow().clone()
    }
}

impl Speaker for FakeSpeaker {
    fn ready(&self) -> Result<(), String> {
        if self.ready {
            Ok(())
        } else {
            Err("the voice is not downloaded".into())
        }
    }

    fn key(&self, text: &str) -> Result<Vec<String>, String> {
        if text.contains("unkeyable") {
            return Err("no phonemes for \"unkeyable\"".into());
        }
        Ok(server_key(&voice()))
    }

    fn speak(&self, text: &str) -> Result<Speech, String> {
        self.texts.borrow_mut().push(text.to_owned());
        if text.contains("broken") {
            return Err("the speaker failed: engine exploded".into());
        }
        let wav = tone(0.1 * text.split_whitespace().count() as f64);
        let decoded = decode_wav(&wav).map_err(String::from)?;
        Ok(Speech { wav, decoded })
    }
}

/// The voice the fake speaker keys clips on.
fn voice() -> ServerVoice {
    ServerVoice {
        base_url: "http://127.0.0.1:8880".into(),
        model: "kokoro".into(),
        voice: "af_heart".into(),
        speed: 1.0,
    }
}

/// A video with narrated scenes `a` and `b` and a silent `quiet`.
fn video(dir: &Path, script: &str) {
    std::fs::write(
        dir.join("video.json"),
        json!({
            "size": { "width": 64, "height": 36 }, "theme": "default",
            "scenes": [
                { "id": "a", "component": "Card" },
                { "id": "b", "component": "Card" },
                { "id": "quiet", "component": "Card", "hold": 1 }
            ]
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(dir.join("script.md"), script).unwrap();
}

const SCRIPT: &str = "## a\nWe follow the gradient.\n\n## b\nOne small step at a time.\n";

fn run(dir: &Path, speaker: &FakeSpeaker, aligner: &dyn Aligner, options: &Options) -> Report {
    narrate(
        dir,
        &Library::default(),
        speaker,
        aligner,
        options,
        &mut |_| {},
    )
    .unwrap()
}

fn manifest(dir: &Path) -> Manifest {
    Manifest::read(dir).unwrap().unwrap()
}

fn outcomes(report: &Report) -> Vec<(&str, Outcome)> {
    report
        .scenes
        .iter()
        .map(|s| (s.id.as_str(), s.outcome))
        .collect()
}

#[test]
fn narrates_each_scene_then_keeps_the_cached_clips() {
    let dir = Scratch::new("narrate-cache");
    video(&dir, SCRIPT);
    let speaker = FakeSpeaker::new();
    let stub = Stub::new();
    let report = run(&dir, &speaker, &stub, &Options::default());
    assert_eq!(
        outcomes(&report),
        [("a", Outcome::Narrated), ("b", Outcome::Narrated)]
    );
    assert_eq!(speaker.spoken().len(), 2);

    let m = manifest(&dir);
    let a = &m.scenes["a"];
    assert_eq!(a.text, "We follow the gradient.");
    assert_eq!(a.audio, format!("{}.wav", a.key));
    assert!((a.duration - 0.4).abs() < 1e-9, "{}", a.duration);
    assert_eq!(a.words.len(), 4);
    assert_eq!(
        std::fs::read(dir.join("narration").join(&a.audio)).unwrap(),
        tone(0.4),
        "the speaker's bytes as sent"
    );
    let project = studio_core::Project::load(&*dir).unwrap();
    assert!(
        project.scene_length(0).unwrap() > 0.4,
        "studio-core reads it"
    );

    let again = run(&dir, &speaker, &stub, &Options::default());
    assert_eq!(
        outcomes(&again),
        [("a", Outcome::Cached), ("b", Outcome::Cached)]
    );
    assert_eq!(speaker.spoken().len(), 2, "nothing requested");
    assert_eq!(stub.calls.get(), 2);
    assert_eq!(again.scenes[1].words, 6);
    assert_eq!(manifest(&dir), m);

    let forced = run(
        &dir,
        &speaker,
        &stub,
        &Options {
            scene: Some("b".into()),
            force: true,
        },
    );
    assert_eq!(outcomes(&forced), [("b", Outcome::Narrated)]);
    assert_eq!(speaker.spoken().len(), 3);
}

#[test]
fn editing_one_paragraph_renarrates_that_scene_only() {
    let dir = Scratch::new("narrate-edit");
    video(&dir, SCRIPT);
    let speaker = FakeSpeaker::new();
    let stub = Stub::new();
    run(&dir, &speaker, &stub, &Options::default());
    let before = manifest(&dir);

    video(
        &dir,
        "## a\nWe follow the gradient.\n\n## b\nOne smaller step at a time.\n",
    );
    let report = run(&dir, &speaker, &stub, &Options::default());
    assert_eq!(
        outcomes(&report),
        [("a", Outcome::Cached), ("b", Outcome::Narrated)]
    );
    let spoken = speaker.spoken();
    assert_eq!(spoken.len(), 3);
    assert_eq!(spoken[2], "One smaller step at a time.");
    let after = manifest(&dir);
    assert_eq!(after.scenes["a"], before.scenes["a"]);
    assert_ne!(after.scenes["b"].key, before.scenes["b"].key);
    assert_eq!(
        report.deleted,
        [before.scenes["b"].audio.clone()],
        "the old clip goes"
    );
}

#[test]
fn a_failure_keeps_the_scenes_before_it() {
    let dir = Scratch::new("narrate-fail");
    video(
        &dir,
        "## a\nWe follow the gradient.\n\n## b\nThis one is broken.\n",
    );
    let speaker = FakeSpeaker::new();
    let mut seen_before_b = None;
    let error = narrate(
        &dir,
        &Library::default(),
        &speaker,
        &Stub::new(),
        &Options::default(),
        &mut |p| {
            if let Progress::Start { id: "b", .. } = p {
                seen_before_b = Some(manifest(&dir));
            }
        },
    )
    .unwrap_err();
    assert_eq!(error.scene.as_deref(), Some("b"));
    assert_eq!(
        error.to_string(),
        "scene `b`: the speaker failed: engine exploded"
    );
    let written = seen_before_b.expect("scene b started");
    assert!(
        written.scenes.contains_key("a"),
        "the manifest is written after each scene"
    );
    let m = manifest(&dir);
    assert_eq!(m.scenes.keys().collect::<Vec<_>>(), ["a"]);
    assert!(dir.join("narration").join(&m.scenes["a"].audio).is_file());
}

#[test]
fn stale_entries_and_orphan_clips_are_pruned() {
    let dir = Scratch::new("narrate-prune");
    video(
        &dir,
        "## a\nWe follow the gradient.\n\n## b\nOne small step.\n",
    );
    let speaker = FakeSpeaker::new();
    let stub = Stub::new();
    run(&dir, &speaker, &stub, &Options::default());
    let gone = manifest(&dir).scenes["b"].audio.clone();
    let clips = dir.join("narration");
    std::fs::write(clips.join("stray.wav"), b"x").unwrap();
    std::fs::write(clips.join("half.wav.part"), b"x").unwrap();
    std::fs::write(clips.join("notes.txt"), b"keep").unwrap();

    video(&dir, "## a\nWe follow the gradient.\n");
    let report = run(&dir, &speaker, &stub, &Options::default());
    assert_eq!(outcomes(&report), [("a", Outcome::Cached)]);
    assert_eq!(report.dropped, ["b"]);
    let mut want = vec![gone, "half.wav.part".into(), "stray.wav".into()];
    want.sort();
    assert_eq!(report.deleted, want);
    assert_eq!(manifest(&dir).scenes.keys().collect::<Vec<_>>(), ["a"]);
    assert!(clips.join("notes.txt").is_file());
    assert_eq!(speaker.spoken().len(), 2);
}

#[test]
fn a_missing_aligner_fails_before_any_tts_request() {
    let dir = Scratch::new("narrate-no-model");
    video(&dir, SCRIPT);
    let speaker = FakeSpeaker::new();
    let stub = Stub {
        ready: false,
        calls: Cell::new(0),
    };
    let error = narrate(
        &dir,
        &Library::default(),
        &speaker,
        &stub,
        &Options::default(),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(
        error.message.contains("oculus studio aligner download"),
        "{error}"
    );
    assert!(speaker.spoken().is_empty());
}

#[test]
fn a_speaker_that_is_not_ready_fails_before_any_clip() {
    let dir = Scratch::new("narrate-no-voice");
    video(&dir, SCRIPT);
    let speaker = FakeSpeaker {
        ready: false,
        texts: RefCell::new(Vec::new()),
    };
    let stub = Stub::new();
    let error = narrate(
        &dir,
        &Library::default(),
        &speaker,
        &stub,
        &Options::default(),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(error.scene, None);
    assert_eq!(error.message, "the voice is not downloaded");
    assert!(speaker.spoken().is_empty());
    assert_eq!(stub.calls.get(), 0);
    assert_eq!(Manifest::read(&dir).unwrap(), None, "nothing written");
}

#[test]
fn reports_low_confidence_words_and_leading_gaps() {
    let dir = Scratch::new("narrate-warn");
    video(&dir, "## a\nUm, Cauchy said so.\n\n## b\nOne small step.\n");
    let speaker = FakeSpeaker::new();
    let report = run(&dir, &speaker, &Stub::new(), &Options::default());
    let a = &report.scenes[0];
    assert_eq!(
        a.low_confidence
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>(),
        ["Cauchy"]
    );
    assert_eq!(a.gaps.len(), 1);
    assert_eq!(manifest(&dir).scenes["a"].gaps[0].after, None);
    let lint = studio_core::Project::load(&*dir).unwrap();
    assert!(lint.manifest.is_some());
}

#[test]
fn an_unknown_scene_or_no_script_is_an_error() {
    let dir = Scratch::new("narrate-args");
    video(&dir, SCRIPT);
    let speaker = FakeSpeaker::new();
    let options = Options {
        scene: Some("quiet".into()),
        force: false,
    };
    let e = narrate(
        &dir,
        &Library::default(),
        &speaker,
        &Stub::new(),
        &options,
        &mut |_| {},
    )
    .unwrap_err();
    assert!(e.message.contains("`a`, `b`"), "{e}");
    std::fs::remove_file(dir.join("script.md")).unwrap();
    let e = narrate(
        &dir,
        &Library::default(),
        &speaker,
        &Stub::new(),
        &Options::default(),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(e.message.contains("no script.md"), "{e}");
}

#[test]
fn an_unreadable_manifest_is_rebuilt() {
    let dir = Scratch::new("narrate-reset");
    video(&dir, SCRIPT);
    std::fs::create_dir_all(dir.join("narration")).unwrap();
    std::fs::write(dir.join(MANIFEST_PATH), r#"{ "version": 2, "scenes": {} }"#).unwrap();
    let speaker = FakeSpeaker::new();
    let mut reset = None;
    narrate(
        &dir,
        &Library::default(),
        &speaker,
        &Stub::new(),
        &Options::default(),
        &mut |p| {
            if let Progress::Reset(why) = p {
                reset = Some(why.to_owned());
            }
        },
    )
    .unwrap();
    assert!(reset.unwrap().contains("version 2"));
    assert_eq!(manifest(&dir).scenes.len(), 2);
}

#[test]
fn the_cache_key_changes_with_every_input() {
    let base = voice();
    let key = cache_key("Hello there.", &server_key(&base));
    assert_eq!(key.len(), 64);
    assert_eq!(key, cache_key("Hello there.", &server_key(&base.clone())));
    let mut variants = vec![cache_key("Hello there!", &server_key(&base))];
    for change in [
        |v: &mut ServerVoice| v.base_url = "http://127.0.0.1:8881".into(),
        |v: &mut ServerVoice| v.model = "tts-1".into(),
        |v: &mut ServerVoice| v.voice = "bf_emma".into(),
        |v: &mut ServerVoice| v.speed = 1.1,
    ] {
        let mut v = base.clone();
        change(&mut v);
        variants.push(cache_key("Hello there.", &server_key(&v)));
    }
    let ambiguous = ServerVoice {
        model: "kokoroaf".into(),
        voice: "_heart".into(),
        ..base.clone()
    };
    variants.push(cache_key("Hello there.", &server_key(&ambiguous)));
    let kokoro = |voice: &str, speed: &str, phonemes: &str| {
        ["kokoro", "kokoro-model", voice, speed, phonemes].map(String::from)
    };
    let k = cache_key("Hello there.", &kokoro("af_heart", "1.0", "həlˈO ðˈɛɹ."));
    assert_eq!(
        k,
        cache_key("Hello there.", &kokoro("af_heart", "1.0", "həlˈO ðˈɛɹ.")),
        "stable"
    );
    variants.push(k);
    for (voice, speed, phonemes) in [
        ("am_michael", "1.0", "həlˈO ðˈɛɹ."),
        ("af_heart", "1.1", "həlˈO ðˈɛɹ."),
        ("af_heart", "1.0", "hɛlˈO ðˈɛɹ."),
    ] {
        variants.push(cache_key("Hello there.", &kokoro(voice, speed, phonemes)));
    }
    for v in &variants {
        assert_ne!(v, &key);
    }
    let distinct: BTreeSet<_> = variants.iter().collect();
    assert_eq!(distinct.len(), variants.len());
}

#[test]
fn a_scene_the_speaker_cannot_key_fails_naming_it() {
    let dir = Scratch::new("narrate-unkeyable");
    video(
        &dir,
        "## a\nWe follow the gradient.\n\n## b\nAn unkeyable word.\n",
    );
    let speaker = FakeSpeaker::new();
    let error = narrate(
        &dir,
        &Library::default(),
        &speaker,
        &Stub::new(),
        &Options::default(),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(error.scene.as_deref(), Some("b"));
    assert!(error.message.contains("unkeyable"), "{}", error.message);
    assert_eq!(speaker.spoken(), ["We follow the gradient."]);
}

/// A video whose theme lives in a library (the starter `warm`) narrates:
/// the narrator loads it over the libraries, as every other command does.
#[test]
fn a_library_theme_narrates() {
    let root = Scratch::new("narrate-library");
    let dir = root.join("video");
    std::fs::create_dir_all(&dir).unwrap();
    video(&dir, SCRIPT);
    let json = std::fs::read_to_string(dir.join("video.json"))
        .unwrap()
        .replace("\"default\"", "\"warm\"");
    std::fs::write(dir.join("video.json"), json).unwrap();
    let library = libraries_at(&root.join("library"), &root.join("subjects"), false).unwrap();
    let speaker = FakeSpeaker::new();
    let options = Options::default();
    let bare = narrate(
        &dir,
        &Library::default(),
        &speaker,
        &Stub::new(),
        &options,
        &mut |_| {},
    );
    assert!(
        bare.unwrap_err()
            .message
            .contains("no bundled theme `warm`"),
        "without the libraries `warm` does not resolve"
    );
    let report = narrate(
        &dir,
        &library,
        &speaker,
        &Stub::new(),
        &options,
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(report.scenes.len(), 2);
    let project = studio_core::Project::load_with(&dir, &library).unwrap();
    assert!(project.manifest.is_some());
    assert_eq!(project.video.theme, "warm");
}

#[test]
fn a_section_without_pauses_keeps_its_key() {
    let dir = Scratch::new("narrate-key");
    video(&dir, SCRIPT);
    let speaker = FakeSpeaker::new();
    run(&dir, &speaker, &Stub::new(), &Options::default());
    let text = "We follow the gradient.";
    let want = cache_key(text, &speaker.key(text).unwrap());
    assert_eq!(manifest(&dir).scenes["a"].key, want);
}

#[test]
fn pauses_splice_exact_silence_into_the_clip_said_whole() {
    let dir = Scratch::new("narrate-pause");
    video(
        &dir,
        "## a\nWe follow. [pause]\nUm, the gradient. [pause 0.5]\n\n## b\nOne small step.\n",
    );
    let speaker = FakeSpeaker::new();
    let stub = Stub::new();
    let report = run(&dir, &speaker, &stub, &Options::default());
    assert_eq!(report.scenes[0].pauses, 2);
    assert_eq!(
        speaker.spoken(),
        ["We follow. Um, the gradient.", "One small step."],
        "said in one go, so the pace is the voice's own"
    );
    assert_eq!(stub.calls.get(), 2, "timed in one pass, before the splice");

    let m = manifest(&dir);
    let a = &m.scenes["a"];
    assert_eq!(a.text, "We follow. [pause 1] Um, the gradient. [pause 0.5]");
    assert!((a.duration - 2.0).abs() < 1e-9, "{}", a.duration);
    let times: Vec<(&str, f64, f64)> = a
        .words
        .iter()
        .map(|w| (w.text.as_str(), w.start, w.end))
        .collect();
    let want = [
        ("We", 0.0, 0.1),
        ("follow.", 0.1, 0.2),
        ("Um,", 1.2, 1.3),
        ("the", 1.3, 1.4),
        ("gradient.", 1.4, 1.5),
    ];
    for ((t, s, e), (wt, ws, we)) in times.iter().zip(want) {
        assert!(
            *t == wt && (s - ws).abs() < 1e-9 && (e - we).abs() < 1e-9,
            "{times:?}"
        );
    }
    let pauses: Vec<(Option<usize>, f64, f64)> =
        a.pauses.iter().map(|p| (p.after, p.start, p.end)).collect();
    assert_eq!(pauses.len(), 2);
    assert_eq!((pauses[0].0, pauses[1].0), (Some(1), Some(4)));
    assert!((pauses[0].1 - 0.2).abs() < 1e-9 && (pauses[0].2 - 1.2).abs() < 1e-9);
    assert!((pauses[1].1 - 1.5).abs() < 1e-9 && (pauses[1].2 - 2.0).abs() < 1e-9);

    let clip = decode_wav(&std::fs::read(dir.join("narration").join(&a.audio)).unwrap()).unwrap();
    assert!((clip.duration - a.duration).abs() < 1e-9);
    let (from, to) = (24_000 / 5, 24_000 * 6 / 5);
    assert!(clip.samples[from..to].iter().all(|&s| s == 0.0));
    let said = decode_wav(&tone(0.5)).unwrap();
    assert_eq!(
        clip.samples[..from],
        said.samples[..from],
        "speech is kept as made"
    );
    assert_eq!(
        clip.samples[to..to + 7_200],
        said.samples[from..],
        "and resumes after"
    );

    let p = studio_core::Project::load(&*dir).unwrap();
    assert_eq!(p.cued_pauses(0).unwrap().len(), 2, "studio-core reads it");

    video(
        &dir,
        "## a\nWe follow. [pause 2]\nUm, the gradient. [pause 0.5]\n\n## b\nOne small step.\n",
    );
    let again = run(&dir, &speaker, &stub, &Options::default());
    assert_eq!(
        outcomes(&again),
        [("a", Outcome::Narrated), ("b", Outcome::Cached)],
        "a pause's length is in the key"
    );
}

#[test]
fn a_pause_is_cut_into_the_silence_between_its_words() {
    let rate = 24_000;
    let tone = |n: usize| (0..n).map(|i| ((i as f32) * 0.05).sin() * 0.3);
    let samples: Vec<f32> = tone(12_000)
        .chain(std::iter::repeat_n(0.0, 7_200))
        .chain(tone(9_600))
        .collect();
    let word = |index: usize, start: f64, end: f64| WordTime {
        index,
        text: "w".into(),
        start,
        end,
        confidence: -0.5,
    };
    let words = [word(0, 0.0, 0.45), word(1, 0.75, 1.2)];
    let c = clip::cut(&samples, rate, &words, 1);
    assert!((12_000..19_200).contains(&c), "in the silence: {c}");
    assert!(c.abs_diff(14_400) <= 60, "its middle, of equal quiet: {c}");
    assert_eq!(clip::cut(&samples, rate, &words, 0), 0);
    assert_eq!(clip::cut(&samples, rate, &words, 2), samples.len());
}
