use std::cell::Cell;
use std::path::Path;

use serde_json::json;

use super::*;
use crate::studio::test_wav::tone;
use crate::studio::timing::{Gap, WordTime};
use crate::test_support::{FakeServer, Reply, Scratch};

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

/// A TTS server whose clip lasts 0.1 s per word of input; an input with
/// `broken` in it gets a 500.
fn tts() -> FakeServer {
    FakeServer::start(|hit| {
        let input = hit.json()["input"].as_str().unwrap_or("").to_owned();
        if input.contains("broken") {
            return Reply::status(500, json!({ "detail": "engine exploded" }));
        }
        Reply::bytes(tone(0.1 * input.split_whitespace().count() as f64))
    })
}

fn voice(server: &FakeServer) -> Voice {
    Voice {
        base_url: server.origin(),
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

fn run(dir: &Path, server: &FakeServer, aligner: &dyn Aligner, options: &Options) -> Report {
    narrate(dir, &voice(server), aligner, options, &mut |_| {}).unwrap()
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
    let server = tts();
    let stub = Stub::new();
    let report = run(&dir, &server, &stub, &Options::default());
    assert_eq!(
        outcomes(&report),
        [("a", Outcome::Narrated), ("b", Outcome::Narrated)]
    );
    assert_eq!(server.hits().len(), 2);

    let m = manifest(&dir);
    let a = &m.scenes["a"];
    assert_eq!(a.text, "We follow the gradient.");
    assert_eq!(a.audio, format!("{}.wav", a.key));
    assert!((a.duration - 0.4).abs() < 1e-9, "{}", a.duration);
    assert_eq!(a.words.len(), 4);
    assert_eq!(
        std::fs::read(dir.join("narration").join(&a.audio)).unwrap(),
        tone(0.4),
        "the server's bytes as sent"
    );
    let project = studio_core::Project::load(&*dir).unwrap();
    assert!(
        project.scene_length(0).unwrap() > 0.4,
        "studio-core reads it"
    );

    let again = run(&dir, &server, &stub, &Options::default());
    assert_eq!(
        outcomes(&again),
        [("a", Outcome::Cached), ("b", Outcome::Cached)]
    );
    assert_eq!(server.hits().len(), 2, "nothing requested");
    assert_eq!(stub.calls.get(), 2);
    assert_eq!(again.scenes[1].words, 6);
    assert_eq!(manifest(&dir), m);

    let forced = run(
        &dir,
        &server,
        &stub,
        &Options {
            scene: Some("b".into()),
            force: true,
        },
    );
    assert_eq!(outcomes(&forced), [("b", Outcome::Narrated)]);
    assert_eq!(server.hits().len(), 3);
}

#[test]
fn editing_one_paragraph_renarrates_that_scene_only() {
    let dir = Scratch::new("narrate-edit");
    video(&dir, SCRIPT);
    let server = tts();
    let stub = Stub::new();
    run(&dir, &server, &stub, &Options::default());
    let before = manifest(&dir);

    video(
        &dir,
        "## a\nWe follow the gradient.\n\n## b\nOne smaller step at a time.\n",
    );
    let report = run(&dir, &server, &stub, &Options::default());
    assert_eq!(
        outcomes(&report),
        [("a", Outcome::Cached), ("b", Outcome::Narrated)]
    );
    let hits = server.hits();
    assert_eq!(hits.len(), 3);
    assert_eq!(hits[2].json()["input"], "One smaller step at a time.");
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
    let server = tts();
    let mut seen_before_b = None;
    let error = narrate(
        &dir,
        &voice(&server),
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
    assert!(
        error
            .to_string()
            .starts_with("scene `b`: the TTS server answered 500")
            && error.message.contains("engine exploded"),
        "{error}"
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
    let server = tts();
    let stub = Stub::new();
    run(&dir, &server, &stub, &Options::default());
    let gone = manifest(&dir).scenes["b"].audio.clone();
    let clips = dir.join("narration");
    std::fs::write(clips.join("stray.wav"), b"x").unwrap();
    std::fs::write(clips.join("half.wav.part"), b"x").unwrap();
    std::fs::write(clips.join("notes.txt"), b"keep").unwrap();

    video(&dir, "## a\nWe follow the gradient.\n");
    let report = run(&dir, &server, &stub, &Options::default());
    assert_eq!(outcomes(&report), [("a", Outcome::Cached)]);
    assert_eq!(report.dropped, ["b"]);
    let mut want = vec![gone, "half.wav.part".into(), "stray.wav".into()];
    want.sort();
    assert_eq!(report.deleted, want);
    assert_eq!(manifest(&dir).scenes.keys().collect::<Vec<_>>(), ["a"]);
    assert!(clips.join("notes.txt").is_file());
    assert_eq!(server.hits().len(), 2);
}

#[test]
fn a_missing_aligner_fails_before_any_tts_request() {
    let dir = Scratch::new("narrate-no-model");
    video(&dir, SCRIPT);
    let server = tts();
    let stub = Stub {
        ready: false,
        calls: Cell::new(0),
    };
    let error = narrate(
        &dir,
        &voice(&server),
        &stub,
        &Options::default(),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(
        error.message.contains("oculus studio aligner download"),
        "{error}"
    );
    assert!(server.hits().is_empty());
}

#[test]
fn reports_low_confidence_words_and_leading_gaps() {
    let dir = Scratch::new("narrate-warn");
    video(&dir, "## a\nUm, Cauchy said so.\n\n## b\nOne small step.\n");
    let server = tts();
    let report = run(&dir, &server, &Stub::new(), &Options::default());
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
    let server = tts();
    let options = Options {
        scene: Some("quiet".into()),
        force: false,
    };
    let e = narrate(&dir, &voice(&server), &Stub::new(), &options, &mut |_| {}).unwrap_err();
    assert!(e.message.contains("`a`, `b`"), "{e}");
    std::fs::remove_file(dir.join("script.md")).unwrap();
    let e = narrate(
        &dir,
        &voice(&server),
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
    let server = tts();
    let mut reset = None;
    narrate(
        &dir,
        &voice(&server),
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
    let base = Voice {
        base_url: "http://127.0.0.1:8880".into(),
        model: "kokoro".into(),
        voice: "af_heart".into(),
        speed: 1.0,
    };
    let key = cache_key("Hello there.", &base);
    assert_eq!(key.len(), 64);
    assert_eq!(key, cache_key("Hello there.", &base.clone()));
    let mut variants = vec![cache_key("Hello there!", &base)];
    for change in [
        |v: &mut Voice| v.base_url = "http://127.0.0.1:8881".into(),
        |v: &mut Voice| v.model = "tts-1".into(),
        |v: &mut Voice| v.voice = "bf_emma".into(),
        |v: &mut Voice| v.speed = 1.1,
    ] {
        let mut v = base.clone();
        change(&mut v);
        variants.push(cache_key("Hello there.", &v));
    }
    let ambiguous = Voice {
        model: "kokoroaf".into(),
        voice: "_heart".into(),
        ..base.clone()
    };
    variants.push(cache_key("Hello there.", &ambiguous));
    for v in &variants {
        assert_ne!(v, &key);
    }
    let distinct: BTreeSet<_> = variants.iter().collect();
    assert_eq!(distinct.len(), variants.len());
}
