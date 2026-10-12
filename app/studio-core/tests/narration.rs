//! Narration: script.md and the manifest set narrated scenes' lengths, stale
//! or missing narration fails renders (not the load), and `word()` /
//! `wordEnd()` cue elements to the narration.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::fixture;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use studio_core::gallery::{self, GalleryOptions};
use studio_core::{Project, RenderOptions, Renderer, StripOptions, StudioError, Time, strip};

/// A project folder in the temp dir, removed on drop.
struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Temp {
        let dir =
            std::env::temp_dir().join(format!("studio-narration-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("components")).unwrap();
        std::fs::create_dir_all(dir.join("narration")).unwrap();
        Temp(dir)
    }

    fn file(&self, rel: &str, content: &str) -> &Temp {
        std::fs::write(self.0.join(rel), content).unwrap();
        self
    }

    fn video(&self, scenes: Value) -> &Temp {
        let video =
            json!({ "size": { "width": 64, "height": 36 }, "theme": "default", "scenes": scenes });
        self.file("video.json", &video.to_string())
    }

    fn manifest(&self, scenes: Value) -> &Temp {
        self.file(
            "narration/manifest.json",
            &json!({ "version": 1, "scenes": scenes }).to_string(),
        )
    }

    fn load(&self) -> Project {
        Project::load(&self.0).unwrap()
    }

    fn load_err(&self) -> String {
        Project::load(&self.0).err().unwrap().to_string()
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// A manifest entry: `words` are `(text, start, end)` at confidence -0.5.
fn clip(text: &str, duration: f64, words: &[(&str, f64, f64)]) -> Value {
    let words: Vec<Value> = words
        .iter()
        .map(|(t, s, e)| json!({ "text": t, "start": s, "end": e, "confidence": -0.5 }))
        .collect();
    json!({ "key": "k", "text": text, "audio": "k.wav", "duration": duration, "words": words })
}

const BLANK: &str = "export const props = {};\nexport default () => <Box />;\n";

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// `n` frames at the default 30 fps, in seconds.
fn frames(n: u32) -> f64 {
    f64::from(n) / 30.0
}

#[test]
fn the_fixture_is_narrated_then_silent() {
    let p = Project::load(fixture("narrated")).unwrap();
    // lead + clip + tail is 0.25 + 3.0 + 1.0 = 4.25 s, 127.5 frames: 128.
    assert!(near(p.scene_length(0).unwrap(), frames(128)));
    assert!(near(p.scene_length(1).unwrap(), 1.5));
    let tl = p.timeline().unwrap();
    assert!(near(tl.length(), frames(173)));
    assert!(near(tl.start(1), frames(128)));
    assert!(near(p.length().unwrap(), frames(173)));
    assert_eq!(p.scene_frames(1).unwrap(), 128..173);
    assert_eq!(
        p.video.voice.as_ref().unwrap().voice.as_deref(),
        Some("af_heart")
    );
    assert_eq!(
        p.section(0).unwrap().text,
        "We follow the gradient downhill, one small step at a time."
    );
    assert!(p.narration(0).unwrap().is_some());
    assert!(p.narration(1).unwrap().is_none() && p.section(1).is_none());
    assert!(p.word_cues(0).unwrap().is_some());
    assert!(p.word_cues(1).unwrap().is_none());
}

#[test]
fn the_card_enters_at_its_word() {
    // `word("gradient")` is 0.62 s into the clip plus the 0.25 s lead.
    let p = Project::load(fixture("narrated")).unwrap();
    let mut r = Renderer::new(&p, RenderOptions::default()).unwrap();
    let mut at = |t: f64| r.frame(Time::Scene { index: 0, t }).unwrap().rgba;
    let start = at(0.0);
    assert!(at(0.86) == start, "nothing has entered before the cue");
    assert!(at(0.9) != start, "the card is entering just after it");
}

#[test]
fn hold_is_a_minimum_for_narrated_scenes() {
    let t = Temp::new("hold");
    t.video(json!([
        { "id": "a", "component": "Blank", "hold": 1.0 },
        { "id": "b", "component": "Blank", "hold": 5.0 },
        { "id": "c", "component": "Blank", "hold": 2.0 }
    ]))
    .file("components/Blank.tsx", BLANK)
    .file("script.md", "## a\nFirst.\n\n## b\nSecond.\n")
    .manifest(json!({
        "a": clip("First.", 2.0, &[("First.", 0.0, 2.0)]),
        "b": clip("Second.", 1.0, &[("Second.", 0.0, 1.0)]),
    }));
    let p = t.load();
    let lengths: Vec<f64> = (0..3).map(|i| p.scene_length(i).unwrap()).collect();
    assert!(
        near(lengths[0], frames(98)),
        "the clip outlasts the hold: {lengths:?}"
    );
    assert!(
        near(lengths[1], 5.0),
        "the hold outlasts the clip: {lengths:?}"
    );
    assert!(
        near(lengths[2], 2.0),
        "a silent scene is its hold: {lengths:?}"
    );
    assert!(near(p.length().unwrap(), frames(98 + 150 + 60)));
    let e = p.scene_length(3).unwrap_err();
    assert!(matches!(e, StudioError::SceneOutOfRange { .. }), "{e}");
}

#[test]
fn missing_or_stale_narration_fails_renders_not_the_load() {
    let t = Temp::new("stale");
    t.video(json!([{ "id": "update", "component": "Blank" }]))
        .file("components/Blank.tsx", BLANK)
        .file("script.md", "## update\nThe *new* text.\n");
    let check = |why: &str| {
        let p = t.load();
        let e = p.timeline().unwrap_err();
        assert_eq!(e.kind(), "narration");
        let message = e.to_string();
        let want = format!(
            "scene `update`: no narration for its current text ({why}); run `oculus studio narrate {}`",
            t.path().display()
        );
        assert_eq!(message, want);
        let e = Renderer::new(&p, RenderOptions::default()).err().unwrap();
        assert_eq!(e.to_string(), want);
        let e = strip(&p, StripOptions::default()).err().unwrap();
        assert_eq!(e.to_string(), want);
        assert!(p.word_cues(0).is_err());
        let report = p.lint().unwrap();
        assert_eq!(report.issues.len(), 1, "{report}");
        assert_eq!(report.issues[0].error.kind(), "narration");
        assert_eq!(report.issues[0].id.as_deref(), Some("update"));
    };
    check("there is no narration/manifest.json");
    t.manifest(json!({ "other": clip("The new text.", 1.0, &[]) }));
    check("narration/manifest.json has no clip for it");
    t.manifest(json!({ "update": clip("The old text.", 1.0, &[]) }));
    check("its clip in narration/manifest.json is for older text");
    // Emphasis and line breaks are not part of the text it must match.
    t.manifest(json!({ "update": clip("The new text.", 1.0, &[]) }));
    assert!(near(t.load().length().unwrap(), frames(68)));
}

#[test]
fn a_silent_scene_needs_a_hold() {
    let t = Temp::new("silent");
    t.video(json!([
        { "id": "a", "component": "Blank", "hold": 1.0 },
        { "id": "b", "component": "Blank" }
    ]))
    .file("components/Blank.tsx", BLANK)
    .file("script.md", "## a\nHello.\n")
    .manifest(json!({ "a": clip("Hello.", 1.0, &[]) }));
    let p = t.load();
    let e = p.timeline().unwrap_err().to_string();
    assert!(
        e.contains("scenes[1] has no hold and no narration") && e.contains("`## b` section"),
        "{e}"
    );
    let report = p.lint().unwrap();
    // Scene `a` also warns that nothing is on screen.
    assert_eq!(report.errors(), 1, "{report}");
    assert_eq!(report.issues[0].scene, Some(1));
}

#[test]
fn script_ids_and_manifest_errors_fail_the_load() {
    let t = Temp::new("load");
    t.file("components/Blank.tsx", BLANK);
    let scenes = json!([{ "id": "a", "component": "Blank", "hold": 1 }]);
    t.video(scenes.clone())
        .file("script.md", "Intro.\n## a\nHello.\n");
    let e = t.load_err();
    assert!(
        e.contains("script.md") && e.contains("line 1: text before the first `## <id>` heading"),
        "{e}"
    );
    t.file("script.md", "## a\nHello.\n## a\nAgain.\n");
    assert!(
        t.load_err()
            .contains("line 3: `## a` repeats the section at line 1")
    );
    t.file("script.md", "## a\nHello.\n");

    t.video(json!([{ "component": "Blank", "hold": 1 }]));
    assert!(t.load_err().contains("scenes[0] has no id"));
    t.video(json!([{ "id": "Intro", "component": "Blank", "hold": 1 }]));
    assert!(
        t.load_err()
            .contains("scenes[0].id `Intro` must be lowercase")
    );
    t.video(json!([
        { "id": "a", "component": "Blank", "hold": 1 },
        { "id": "a", "component": "Blank", "hold": 1 }
    ]));
    assert!(
        t.load_err()
            .contains("scenes[1].id `a` is also scenes[0].id")
    );

    t.video(scenes.clone());
    t.file(
        "narration/manifest.json",
        r#"{ "version": 2, "scenes": {} }"#,
    );
    let e = t.load_err();
    assert!(
        e.contains("manifest.json") && e.contains("version 2"),
        "{e}"
    );
    t.file(
        "narration/manifest.json",
        r#"{ "version": 1, "scenes": {}, "extra": 1 }"#,
    );
    assert!(t.load_err().contains("unknown field `extra`"));
    t.manifest(
        json!({ "a": { "key": "k", "text": "Hello.", "audio": "k.wav", "duration": 1.0,
        "words": [{ "text": "Hello.", "start": 0.6, "end": 0.2, "confidence": 0 }] } }),
    );
    assert!(t.load_err().contains("scenes.a.words[0]"));
    t.manifest(
        json!({ "a": { "key": "k", "text": "Hello.", "audio": "k.wav", "duration": 1.0,
        "words": [{ "text": "Hello.", "start": 0.0, "end": 0.5, "confidence": 0 }],
        "gaps": [{ "after": 1, "start": 0.5, "end": 0.7, "heard": "uh" }] } }),
    );
    assert!(
        t.load_err()
            .contains("scenes.a.gaps[0] follows word 1 of 1")
    );
    t.manifest(
        json!({ "a": { "key": "k", "text": "Hello.", "audio": "k.wav", "duration": 1.0,
        "words": [{ "text": "Hello.", "start": 0.3, "end": 0.5, "confidence": 0 }],
        "gaps": [{ "after": null, "start": 0.0, "end": 0.2, "heard": "uh" }] } }),
    );
    let gap = &t.load().manifest.unwrap().scenes["a"].gaps[0];
    assert_eq!(gap.after, None, "null is speech before the first word");
    t.manifest(json!({}));

    let video = |voice: Value| {
        json!({ "size": { "width": 64, "height": 36 }, "theme": "default", "voice": voice, "scenes": scenes })
            .to_string()
    };
    t.file("video.json", &video(json!({ "speed": 0 })));
    assert!(t.load_err().contains("voice.speed is 0"));
    t.file("video.json", &video(json!({ "pitch": 1 })));
    assert!(t.load_err().contains("unknown field `pitch`"));
    t.file(
        "video.json",
        &video(json!({ "model": "kokoro", "voice": "af_heart", "speed": 1.1 })),
    );
    let voice = t.load().video.voice.unwrap();
    assert_eq!(voice.model.as_deref(), Some("kokoro"));
    assert_eq!(voice.speed, Some(1.1));
    assert_eq!(
        voice.engine, None,
        "a block without `engine` leaves it unset"
    );
    t.file(
        "video.json",
        &video(json!({ "engine": "kokoro", "voice": "am_michael" })),
    );
    let voice = t.load().video.voice.unwrap();
    assert_eq!(voice.engine, Some(studio_core::VoiceEngine::Kokoro));
    t.file("video.json", &video(json!({ "engine": "server" })));
    assert_eq!(
        t.load().video.voice.unwrap().engine,
        Some(studio_core::VoiceEngine::Server)
    );
    t.file("video.json", &video(json!({ "engine": "piper" })));
    assert!(
        t.load_err().contains("unknown variant `piper`"),
        "{}",
        t.load_err()
    );
}

/// A project whose two narrated scenes call `Probe`, which runs `body`.
fn probe(name: &str, body: &str) -> Temp {
    let t = Temp::new(name);
    t.video(json!([
        { "id": "a", "component": "Probe" },
        { "id": "b", "component": "Probe" },
        { "id": "quiet", "component": "Probe", "hold": 1, "transition": { "kind": "cut" } }
    ]))
    .file(
        "components/Probe.tsx",
        &format!(
            "export const props = {{}};\nexport default () => {{\n{body}\n  return <Box />;\n}};\n"
        ),
    )
    .file(
        "script.md",
        "## a\nThe gradient, the *small* step. It's time.\n\n## b\nHello there.\n",
    )
    .manifest(json!({
        "a": clip("The gradient, the small step. It's time.", 2.1, &[
            ("The", 0.0, 0.2), ("gradient,", 0.2, 0.7), ("the", 0.8, 0.9), ("small", 0.9, 1.2),
            ("step.", 1.2, 1.5), ("It\u{2019}s", 1.6, 1.8), ("time.", 1.8, 2.1),
        ]),
        "b": clip("Hello there.", 1.0, &[("Hello", 0.1, 0.5), ("there.", 0.5, 1.0)]),
    }));
    t
}

fn logs(p: &Project, at: Time) -> Vec<String> {
    let mut r = Renderer::new(p, RenderOptions::default()).unwrap();
    r.frame(at).unwrap().logs
}

#[test]
fn word_cues_are_scene_seconds_with_the_lead() {
    let t = probe(
        "cues",
        r#"  console.log(JSON.stringify([word("gradient"), wordEnd("gradient"), word("small step"),
    wordEnd("small  STEP"), word("the", 2), word("THE"), word("it's"), wordEnd("time")]));"#,
    );
    let p = t.load();
    let got: Vec<f64> =
        serde_json::from_str(&logs(&p, Time::Scene { index: 0, t: 1.0 })[0]).unwrap();
    let want = [0.45, 0.95, 1.15, 1.75, 1.05, 0.25, 1.85, 2.35];
    assert_eq!(got.len(), want.len());
    for (g, w) in got.iter().zip(want) {
        assert!(near(*g, w), "{got:?} != {want:?}");
    }
}

#[test]
fn cued_words_are_the_times_word_returns() {
    let t = probe(
        "cued",
        r#"  console.log(JSON.stringify([word("gradient"), wordEnd("gradient")]));"#,
    );
    let p = t.load();
    let got: Vec<f64> =
        serde_json::from_str(&logs(&p, Time::Scene { index: 0, t: 1.0 })[0]).unwrap();
    let words = p.cued_words(0).unwrap().unwrap();
    let gradient = words.iter().find(|w| w.text == "gradient,").unwrap();
    assert!(near(gradient.start, got[0]) && near(gradient.end, got[1]));
    assert_eq!(words.len(), 7);
    assert!(p.cued_words(2).unwrap().is_none(), "`quiet` is silent");
}

#[test]
fn word_errors_name_the_call_its_line_and_the_closest_word() {
    let cases = [
        (
            "  word(\"gradiant\");",
            "`word(\"gradiant\")`: no such word in scene `a`; closest: gradient",
        ),
        (
            "  wordEnd(\"small stop\");",
            "`wordEnd(\"small stop\")`: no such word in scene `a`; closest: small step",
        ),
        (
            "  word(\"gradient\", 2);",
            "`word(\"gradient\", 2)`: scene `a` says \"gradient\" once, so there is no occurrence 2",
        ),
        (
            "  word(\"...\");",
            "`word(\"...\")`: the phrase has no words",
        ),
    ];
    for (i, (body, want)) in cases.iter().enumerate() {
        let t = probe(&format!("err{i}"), body);
        let p = t.load();
        let mut r = Renderer::new(&p, RenderOptions::default()).unwrap();
        let e = r.frame(Time::Scene { index: 0, t: 0.0 }).unwrap_err();
        assert_eq!(e.kind(), "component");
        let loc = e.loc().unwrap_or_else(|| panic!("no TSX line: {e}"));
        assert_eq!(loc.line, 3, "{e}");
        assert!(e.to_string().contains(want), "{e}\nwanted: {want}");
    }
    let t = probe("silent", "  word(\"hello\");");
    let p = t.load();
    let mut r = Renderer::new(&p, RenderOptions::default()).unwrap();
    let e = r.frame(Time::Scene { index: 2, t: 0.0 }).unwrap_err();
    assert!(
        e.to_string()
            .contains("`word(\"hello\")`: this scene is not narrated"),
        "{e}"
    );
    assert_eq!(e.loc().unwrap().line, 3);
}

#[test]
fn transition_calls_get_their_own_scenes_words() {
    let t = probe(
        "transition",
        "  let w = [];\n  try { w.push(word(\"hello\")); } catch (e) { w.push(word(\"gradient\")); }\n  console.log(String(w[0]));",
    );
    let p = t.load();
    let start = p.timeline().unwrap().start(1);
    // 0.2 s into the default 0.7 s dissolve into scene b.
    let got = logs(&p, Time::Global(start + 0.2));
    assert_eq!(got, ["scene 0: 0.45", "0.35"]);
}

/// A project whose narrated scene `a` says "We get them back at
/// ninety-one." and calls `Probe`, which runs `body`.
fn hyphens(name: &str, body: &str) -> Temp {
    let t = Temp::new(name);
    t.video(json!([{ "id": "a", "component": "Probe" }]))
        .file(
            "components/Probe.tsx",
            &format!(
                "export const props = {{}};\nexport default () => {{\n{body}\n  return <Box />;\n}};\n"
            ),
        )
        .file("script.md", "## a\nWe get them back at ninety-one.\n")
        .manifest(json!({
            "a": clip("We get them back at ninety-one.", 2.0, &[
                ("We", 0.0, 0.2), ("get", 0.2, 0.4), ("them", 0.4, 0.6), ("back", 0.6, 0.9),
                ("at", 0.9, 1.0), ("ninety-one.", 1.0, 1.8),
            ]),
        }));
    t
}

fn first_error(t: &Temp) -> String {
    let p = t.load();
    let mut r = Renderer::new(&p, RenderOptions::default()).unwrap();
    r.frame(Time::Scene { index: 0, t: 0.0 })
        .unwrap_err()
        .to_string()
}

#[test]
fn a_hyphenated_word_matches_its_parts_spaced_or_hyphenated() {
    let t = hyphens(
        "hyphen",
        r#"  console.log(JSON.stringify([word("ninety one"), word("Ninety-One"), wordEnd("ninety one"),
    word("at ninety one")]));"#,
    );
    let p = t.load();
    let got: Vec<f64> =
        serde_json::from_str(&logs(&p, Time::Scene { index: 0, t: 0.0 })[0]).unwrap();
    let want = [1.25, 1.25, 2.05, 1.15];
    for (g, w) in got.iter().zip(want) {
        assert!(near(*g, w), "{got:?} != {want:?}");
    }
}

#[test]
fn every_miss_names_the_closest_phrase_said() {
    let cases = [
        (
            r#"  word("getting them back");"#,
            "; closest: get them back",
        ),
        (r#"  word("ninetyone");"#, "; closest: ninety-one"),
        // A phrase ending inside a word has no time to give.
        (
            r#"  wordEnd("back at ninety");"#,
            "; closest: back at ninety-one",
        ),
        (r#"  word("zebra");"#, "; closest: "),
    ];
    for (i, (body, want)) in cases.iter().enumerate() {
        let e = first_error(&hyphens(&format!("miss{i}"), body));
        assert!(e.contains("no such word in scene `a`"), "{e}");
        assert!(e.contains(want), "{e}\nwanted: {want}");
    }
}

/// A narrated video with no manifest: `title` uses `Card`, `question`
/// another component, and `Card` runs `body` in its call.
fn unnarrated(name: &str, body: &str) -> Temp {
    let t = Temp::new(name);
    t.video(json!([
        { "id": "title", "component": "Card" },
        { "id": "question", "component": "Other" }
    ]))
    .file(
        "components/Card.tsx",
        &format!(
            "export const props = {{ label: str() }};\nexport const examples = {{ one: {{ label: \"a\" }}, two: {{ label: \"b\" }} }};\nexport default ({{ label }}) => {{\n{body}\n  return <Box><Text>{{label}}</Text></Box>;\n}};\n"
        ),
    )
    .file("components/Other.tsx", BLANK)
    .file("script.md", "## title\nHello there.\n\n## question\nWhy?\n");
    t
}

#[test]
fn a_gallery_renders_a_narrated_video_before_narrate() {
    let t = unnarrated("gallery", "");
    assert!(!t.path().join("narration/manifest.json").exists());
    let p = Project::load_unnarrated(t.path()).unwrap();
    let g = gallery::render(&p, "Card", None, &GalleryOptions::default()).unwrap();
    let names: Vec<&str> = g.stills.iter().map(|s| s.example.as_str()).collect();
    assert_eq!(names, ["one", "two"]);
    // A narrated scene's length needs its narration: the preview's middle.
    assert_eq!((g.moment.t, g.moment.duration), (3.0, gallery::PREVIEW));
    assert!(gallery::render(&p, "Card", Some(5.5), &GalleryOptions::default()).is_ok());
}

#[test]
fn word_in_a_gallery_says_a_gallery_has_no_narration() {
    let t = unnarrated("gallery-word", "  word(\"hello\");");
    let p = Project::load_unnarrated(t.path()).unwrap();
    let e = gallery::render(&p, "Card", None, &GalleryOptions::default())
        .unwrap_err()
        .to_string();
    assert!(e.contains("example `one`"), "{e}");
    assert!(
        e.contains("`word(\"hello\")`: a gallery renders without narration"),
        "{e}"
    );
}

/// A project whose narrated scene `a` has two pauses: a bare one after
/// "lands." and a 1.5 s one at the end, and whose `Probe` runs `body`.
fn paused(name: &str, body: &str) -> Temp {
    let t = Temp::new(name);
    t.video(json!([{ "id": "a", "component": "Probe" }]))
        .file(
            "components/Probe.tsx",
            &format!(
                "export const props = {{}};\nexport default () => {{\n{body}\n  return <Box />;\n}};\n"
            ),
        )
        .file(
            "script.md",
            "## a\nIt *lands.* [pause]\nThen it stays. [pause 1.5]\n",
        );
    let mut a = clip(
        "It lands. [pause 1] Then it stays. [pause 1.5]",
        5.0,
        &[
            ("It", 0.0, 0.2),
            ("lands.", 0.2, 0.8),
            ("Then", 2.2, 2.5),
            ("it", 2.5, 2.6),
            ("stays.", 2.6, 3.2),
        ],
    );
    a["pauses"] = json!([
        { "after": 1, "start": 1.0, "end": 2.0 },
        { "after": 4, "start": 3.5, "end": 5.0 }
    ]);
    t.manifest(json!({ "a": a }));
    t
}

#[test]
fn pauses_are_cut_from_the_text_and_cue_with_the_lead() {
    let t = paused(
        "pauses",
        "  console.log(JSON.stringify([pause(), pause(2), word(\"then\")]));",
    );
    let p = t.load();
    let s = p.section(0).unwrap();
    assert_eq!(s.text, "It lands. Then it stays.");
    assert_eq!(
        s.clip_text(1.0),
        "It lands. [pause 1] Then it stays. [pause 1.5]"
    );
    let got: Vec<f64> =
        serde_json::from_str(&logs(&p, Time::Scene { index: 0, t: 0.5 })[0]).unwrap();
    assert_eq!(got, vec![1.25, 3.75, 2.45]);
    let pauses = p.cued_pauses(0).unwrap();
    assert_eq!(pauses.len(), 2);
    assert_eq!(pauses[0].after, Some(1));
    // lead + clip (its pauses included) + tail: 0.25 + 5.0 + 1.0 = 6.25 s,
    // 187.5 frames: 188.
    assert!(near(p.scene_length(0).unwrap(), frames(188)));
}

#[test]
fn a_pause_is_part_of_the_text_its_clip_must_match() {
    let t = paused("pause-stale", "");
    t.file(
        "script.md",
        "## a\nIt lands. [pause 2]\nThen it stays. [pause 1.5]\n",
    );
    let e = t.load().timeline().unwrap_err().to_string();
    assert!(e.contains("is for older text"), "{e}");
    t.file("script.md", "## a\nIt lands.\nThen it stays.\n");
    let e = t.load().timeline().unwrap_err().to_string();
    assert!(e.contains("is for older text"), "{e}");
}

#[test]
fn pause_errors_name_the_call() {
    let cases = [
        (
            "  pause(3);",
            "`pause(3)`: scene `a` has 2 pauses, so there is no pause 3",
        ),
        (
            "  pause(0);",
            "`pause(0)`: nth must be a whole number from 1",
        ),
    ];
    for (i, (body, want)) in cases.iter().enumerate() {
        let t = paused(&format!("pause-err{i}"), body);
        let p = t.load();
        let mut r = Renderer::new(&p, RenderOptions::default()).unwrap();
        let e = r.frame(Time::Scene { index: 0, t: 0.0 }).unwrap_err();
        assert!(e.to_string().contains(want), "{e}\nwanted: {want}");
    }
    let t = probe("no-pause", "  pause();");
    let p = t.load();
    let mut r = Renderer::new(&p, RenderOptions::default()).unwrap();
    let e = r.frame(Time::Scene { index: 0, t: 0.0 }).unwrap_err();
    assert!(
        e.to_string()
            .contains("`pause()`: scene `a` has no `[pause]` in script.md"),
        "{e}"
    );
}

#[test]
fn a_manifest_checks_its_pauses_and_writes_none_when_there_are_none() {
    let t = paused("pause-check", "");
    let mut a = clip("It lands. [pause 1] Then it stays. [pause 1.5]", 5.0, &[]);
    a["pauses"] = json!([{ "after": 0, "start": 1.0, "end": 2.0 }]);
    t.manifest(json!({ "a": a }));
    assert!(
        t.load_err()
            .contains("scenes.a.pauses[0] follows word 0 of 0")
    );
    let m: studio_core::Manifest = serde_json::from_value(json!({
        "version": 1,
        "scenes": { "a": clip("Hello.", 1.0, &[("Hello.", 0.0, 0.5)]) }
    }))
    .unwrap();
    assert!(m.scenes["a"].pauses.is_empty());
    let back = serde_json::to_value(&m).unwrap();
    assert!(back["scenes"]["a"].get("pauses").is_none());
}

#[test]
fn a_fingerprint_holds_pauses_only_when_there_are_some() {
    let t = paused("pause-print", "");
    let bytes = String::from_utf8(t.load().scene_fingerprint(0).unwrap()).unwrap();
    assert!(
        bytes.contains("\"pauses\":[[1.0,2.0],[3.5,5.0]]"),
        "{bytes}"
    );
    let p = Project::load(fixture("narrated")).unwrap();
    let bytes = String::from_utf8(p.scene_fingerprint(0).unwrap()).unwrap();
    assert!(!bytes.contains("pauses"), "{bytes}");
}
