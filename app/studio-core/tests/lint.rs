//! `lint` collects every problem in a project, tagged with scene and
//! component, without stopping at the first.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::fixture;
use serde_json::json;
use studio_core::{LOW_CONFIDENCE, LintLevel, Project, StudioError};

#[test]
fn clean_projects_lint_clean() {
    for name in [
        "title-card",
        "layout",
        "text-motion",
        "reveal",
        "stroke-draw",
        "counter",
        "enter",
        "camera",
        "transitions",
        "morph",
        "magic",
        "dark-gradient",
        "narrated",
    ] {
        let report = Project::load(fixture(name)).unwrap().lint().unwrap();
        // The renderer fixtures paint literal colours on purpose (their
        // goldens pin those pixels); the token rules are checked below.
        // Warnings are about how a scene looks, which some fixtures
        // stretch on purpose (a background alone).
        let other: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.level == LintLevel::Error && !common::component_rule(&i.to_string()))
            .collect();
        assert!(other.is_empty(), "{name}: {report}");
    }
}

#[test]
fn token_use_is_reported_where_a_fixture_uses_literals() {
    let report = Project::load(fixture("title-card"))
        .unwrap()
        .lint()
        .unwrap();
    let token: Vec<String> = report
        .issues
        .iter()
        .map(ToString::to_string)
        .filter(|l| common::component_rule(l))
        .collect();
    assert!(
        token
            .iter()
            .any(|l| l
                .contains("TitleCard.tsx:31:49: colour `#ffffff` is a literal; use theme.color.")),
        "{token:#?}"
    );
    assert!(
        token
            .iter()
            .any(|l| l.contains("style value 4 equals theme.space.xxs")),
        "{token:#?}"
    );
}

#[test]
fn broken_project_reports_everything() {
    let report = Project::load(fixture("broken")).unwrap().lint().unwrap();
    let lines: Vec<String> = report.issues.iter().map(ToString::to_string).collect();
    let text = lines.join("\n");
    assert_eq!(lines.len(), 5, "{text}");
    let by_scene = |i: usize| {
        report
            .issues
            .iter()
            .find(|x| x.scene == Some(i))
            .unwrap_or_else(|| panic!("no issue for scene {i}: {text}"))
    };
    assert!(
        matches!(by_scene(1).error, StudioError::ComponentNotFound { .. }),
        "{text}"
    );
    assert!(
        by_scene(1)
            .error
            .to_string()
            .contains("available: Fine, LateThrow, Typo"),
        "{text}"
    );
    assert!(
        matches!(by_scene(2).error, StudioError::Props { .. }),
        "{text}"
    );
    assert!(
        matches!(by_scene(3).error, StudioError::Style { .. }),
        "{text}"
    );
    // Fails only at the end of the scene: lint renders both ends.
    assert!(by_scene(4).error.to_string().contains("halfway"), "{text}");
    let example = report.issues.iter().find(|x| x.example.is_some()).unwrap();
    assert_eq!(example.example.as_deref(), Some("bad"));
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("scene 3 (Typo): Typo.tsx:4:5: Box > Box[0]: unknown style key")),
        "{text}"
    );
}

#[test]
fn bad_video_json_is_a_project_error() {
    let dir = std::env::temp_dir().join(format!("studio-bad-video-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let write = |s: &str| std::fs::write(dir.join("video.json"), s).unwrap();
    write(
        r#"{ "size": { "width": 100, "height": 100 }, "theme": "default", "scenes": [], "rate": 30 }"#,
    );
    let e = Project::load(&dir).err().unwrap().to_string();
    assert!(e.contains("unknown field `rate`"), "{e}");
    write(
        r#"{ "size": { "width": 100, "height": 100 }, "theme": "default", "fps": 29.97, "scenes": [{ "component": "A", "hold": 1 }] }"#,
    );
    let e = Project::load(&dir).err().unwrap().to_string();
    assert!(e.contains("fps is 29.97; it must be one of"), "{e}");
    write(
        r#"{ "size": { "width": 100, "height": 100 }, "theme": "default", "fps": 48, "scenes": [{ "component": "A", "hold": 1 }] }"#,
    );
    let e = Project::load(&dir).err().unwrap().to_string();
    assert!(
        e.contains("fps is 48; it must be one of 24, 25, 30, 50, 60"),
        "{e}"
    );
    write(r#"{ "size": { "width": 100, "height": 100 }, "theme": "default", "scenes": [] }"#);
    let e = Project::load(&dir).err().unwrap().to_string();
    assert!(e.contains("at least one scene"), "{e}");
    write(
        r#"{ "size": { "width": 100, "height": 100 }, "theme": "neon", "scenes": [{ "component": "A", "hold": 1 }] }"#,
    );
    let e = Project::load(&dir).err().unwrap().to_string();
    assert!(e.contains("bundled themes: default"), "{e}");
    write(
        r#"{ "size": { "width": 100, "height": 100 }, "theme": "default", "scenes": [{ "component": "A", "hold": 0 }] }"#,
    );
    let e = Project::load(&dir).err().unwrap().to_string();
    assert!(e.contains("scenes[0].hold"), "{e}");
    let theme = std::fs::read_to_string(fixture("../../themes/default.json")).unwrap();
    std::fs::write(
        dir.join("mine.json"),
        theme.replace("\"bg\": \"#f4f4f6\"", "\"bg\": \"#000000\""),
    )
    .unwrap();
    write(
        r#"{ "size": { "width": 100, "height": 100 }, "theme": "mine.json", "scenes": [{ "component": "A", "hold": 1 }] }"#,
    );
    let project = Project::load(&dir).unwrap();
    assert_eq!(project.theme.background.r, 0.0);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn narration_lint_reports_the_script_confidence_and_stale_scenes() {
    let dir = std::env::temp_dir().join(format!("studio-lint-narration-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(dir.join("components")).unwrap();
    std::fs::create_dir_all(dir.join("narration")).unwrap();
    let write = |rel: &str, s: &str| std::fs::write(dir.join(rel), s).unwrap();
    write(
        "video.json",
        &json!({ "size": { "width": 64, "height": 36 }, "theme": "default", "scenes": [
            { "id": "a", "component": "Say" },
            { "id": "b", "component": "Say" },
            { "id": "c", "component": "Quiet", "hold": 1 }
        ] })
        .to_string(),
    );
    write(
        "components/Say.tsx",
        "export const props = {};\nexport default () =>\n  <Box style={{ opacity: word(\"helo\") }} />;\n",
    );
    write(
        "components/Quiet.tsx",
        "export const props = {};\nexport default () => <Box />;\n",
    );
    write(
        "script.md",
        "## a\nHello, a rate of 0.01, e.g. here.\n\n## b\nNew words.\n\n## outro\nBye.\n",
    );
    let word = |t: &str, c: f64| json!({ "text": t, "start": 0.1, "end": 0.4, "confidence": c });
    write(
        "narration/manifest.json",
        &json!({ "version": 1, "scenes": {
            "a": { "key": "k", "text": "Hello, a rate of 0.01, e.g. here.", "audio": "k.wav",
                   "duration": 6.0,
                   "words": [word("Hello,", -0.5), word("rate", LOW_CONFIDENCE - 1.0)],
                   "gaps": [{ "after": null, "start": 0.0, "end": 0.1, "heard": "ah" },
                            { "after": 0, "start": 0.4, "end": 0.6, "heard": "um" }] },
            "b": { "key": "k2", "text": "Old words.", "audio": "k2.wav", "duration": 1.0, "words": [] }
        } })
        .to_string(),
    );
    let report = Project::load(&dir).unwrap().lint().unwrap();
    std::fs::remove_dir_all(&dir).ok();
    let lines: Vec<String> = report.issues.iter().map(ToString::to_string).collect();
    let text = lines.join("\n");
    let has = |want: &str| {
        assert!(
            lines.iter().any(|l| l.starts_with(want)),
            "no line starting {want:?}:\n{text}"
        );
    };
    has("scene 0 `a` (Say): script.md:2: `0.01` is not spoken form: write numbers in words");
    has(
        "scene 0 `a` (Say): script.md:2: `e.g.` is not spoken form: write `for example`, not `e.g.`",
    );
    has("script.md:7: `## outro` matches no scene");
    has("note: scene 0 `a` (Say): scene `a`: the aligner is unsure of `rate` at 0.35 s");
    has("warning: scene 2 `c` (Quiet): nothing is ever on screen");
    has("scene 1 `b` (Say): scene `b`: no narration for its current text");
    has(
        "scene 0 `a` (Say): scene `a`: the audio has unscripted speech after `Hello,` (heard \"um\")",
    );
    has(
        "scene 0 `a` (Say): scene `a`: the audio has unscripted speech before the first word (heard \"ah\")",
    );
    has("scene 0 `a` (Say): Say.tsx:3:");
    assert!(
        text.contains("`word(\"helo\")`: no such word in scene `a`; closest: hello"),
        "{text}"
    );
    assert_eq!(lines.len(), 9, "{text}");
    assert_eq!(
        report.errors(),
        7,
        "the note and the warning do not count: {text}"
    );
    let note = report
        .issues
        .iter()
        .find(|i| i.level == LintLevel::Note)
        .unwrap();
    assert_eq!(
        note.group,
        Some(("unsure word", "\"rate\" 0.3 s".to_owned()))
    );
    let orphan = report.issues.iter().find(|i| i.scene.is_none()).unwrap();
    assert_eq!(
        (orphan.id.as_deref(), orphan.component.as_deref()),
        (Some("outro"), None)
    );
    assert_eq!(orphan.error.kind(), "script");
}
