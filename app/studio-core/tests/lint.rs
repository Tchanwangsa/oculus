//! `lint` collects every problem in a project, tagged with scene and
//! component, without stopping at the first.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::fixture;
use studio_core::{Project, StudioError};

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
    ] {
        let report = Project::load(fixture(name)).unwrap().lint().unwrap();
        assert!(report.is_clean(), "{name}: {report}");
    }
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
        r#"{ "size": { "width": 100, "height": 100 }, "theme": "default", "scenes": [], "fps": 30 }"#,
    );
    let e = Project::load(&dir).err().unwrap().to_string();
    assert!(e.contains("unknown field `fps`"), "{e}");
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
