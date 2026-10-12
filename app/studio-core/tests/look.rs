//! What a scene shows over time: lint's warnings (off-frame, still,
//! half-frame, fragment), their level, `lint_scene`, and `beats`.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::fixture;
use serde_json::{Value, json};
use std::path::PathBuf;
use studio_core::{LintIssue, LintLevel, Project};

/// A project folder in the temp dir, 800x400, removed on drop.
struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Temp {
        let dir = std::env::temp_dir().join(format!("studio-look-{name}-{}", std::process::id()));
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
        let video = json!({ "size": { "width": 800, "height": 400 }, "theme": "default", "scenes": scenes });
        self.file("video.json", &video.to_string())
    }

    /// One component `A` returning `body` (JSX, `t` in scope) in a scene.
    fn scene(&self, body: &str, scene: Value) -> Project {
        self.file(
            "components/A.tsx",
            &format!("export const props = {{}};\nexport default ({{}}, {{ t }}) =>\n  {body};\n"),
        )
        .video(json!([scene]));
        Project::load(&self.0).unwrap()
    }

    /// Scene `a` narrated with `text`, one word a second from 0.
    fn narrate(&self, text: &str) -> &Temp {
        let words: Vec<Value> = text
            .split(' ')
            .enumerate()
            .map(|(i, w)| {
                let s = i as f64;
                json!({ "text": w, "start": s, "end": s + 0.8, "confidence": -0.5 })
            })
            .collect();
        let n = words.len() as f64;
        self.file("script.md", &format!("## a\n{text}\n"));
        self.file(
            "narration/manifest.json",
            &json!({ "version": 1, "scenes": { "a": {
                "key": "k", "text": text, "audio": "k.wav", "duration": n, "words": words } } })
            .to_string(),
        )
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn warnings(p: &Project) -> Vec<(&'static str, String)> {
    let look = p.look(&[0]).pop().unwrap().unwrap();
    look.warnings()
        .into_iter()
        .map(|w| (w.kind, w.message))
        .collect()
}

fn kinds(p: &Project) -> Vec<&'static str> {
    warnings(p).into_iter().map(|(k, _)| k).collect()
}

/// An absolutely placed box, `left` and `top` in px.
fn boxed(key: &str, left: &str, top: &str, size: u32) -> String {
    format!(
        "<Box key=\"{key}\" style={{{{ position: \"absolute\", left: {left}, top: {top}, width: {size}, height: {size}, fill: \"#5e6ad2\" }}}} />"
    )
}

/// Boxes spread across the frame, so only the check under test fires.
fn spread(extra: &str) -> String {
    format!(
        "<Box style={{{{ flexGrow: 1 }}}}>{}{}{extra}</Box>",
        boxed("l", "40", "150", 100),
        boxed("r", "660", "150", 100)
    )
}

#[test]
fn a_node_left_past_the_edge_is_off_frame_but_one_sliding_in_is_not() {
    let t = Temp::new("off");
    let past = boxed("label", "750", "40", 100);
    let p = t.scene(&spread(&past), json!({ "component": "A", "hold": 2 }));
    let w = warnings(&p);
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].0, "off_frame");
    assert!(
        w[0].1
            .starts_with("`label` is 50 px past the right edge from 0.0 s to 2.0 s"),
        "{}",
        w[0].1
    );
    // Moving every sample through the edge: never settled.
    let sliding = boxed("slide", "-100 + t * 300", "40", 100);
    let p = t.scene(&spread(&sliding), json!({ "component": "A", "hold": 2 }));
    assert_eq!(kinds(&p), Vec::<&str>::new());
}

#[test]
fn a_camera_zoom_crops_on_purpose() {
    let p = Project::load(fixture("camera")).unwrap();
    assert!(!kinds(&p).contains(&"off_frame"), "{:?}", warnings(&p));
}

#[test]
fn narration_over_a_frame_that_does_not_change_is_still() {
    let t = Temp::new("still");
    t.narrate("one two three four five six");
    let p = t.scene(&spread(""), json!({ "id": "a", "component": "A" }));
    let w = warnings(&p);
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].0, "still");
    assert!(
        w[0].1.starts_with("nothing on screen changes from 0.2 s to 6.2 s while the narration says \"one two three four five six\""),
        "{}",
        w[0].1
    );
    // A box that moves the whole time is never still.
    let p = t.scene(
        &spread(&boxed("m", "300", "40 + t * 20", 50)),
        json!({ "id": "a", "component": "A" }),
    );
    assert_eq!(kinds(&p), Vec::<&str>::new());
}

#[test]
fn an_empty_opening_is_still() {
    let t = Temp::new("opening");
    let late = "<Box style={{ flexGrow: 1, opacity: t > 1.5 ? 1 : 0 }}>{SPREAD}</Box>";
    let p = t.scene(
        &late.replace("{SPREAD}", &spread("")),
        json!({ "component": "A", "hold": 3 }),
    );
    let w = warnings(&p);
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(
        w[0].1.starts_with("nothing is on screen until 1.6 s"),
        "{}",
        w[0].1
    );
}

#[test]
fn content_kept_to_one_half_is_half_frame() {
    let t = Temp::new("half");
    let left = format!(
        "<Box style={{{{ flexGrow: 1 }}}}>{}{}</Box>",
        boxed("a", "40", "50", 100),
        boxed("b", "200", "200", 100)
    );
    let p = t.scene(&left, json!({ "component": "A", "hold": 4 }));
    let w = warnings(&p);
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].0, "half_frame");
    assert!(
        w[0].1.starts_with("everything on screen sits in the left half (x 40-300 of 800 px) from 0.0 s to 4.0 s; the right half is empty"),
        "{}",
        w[0].1
    );
    // The same for under 3 s is fine.
    let p = t.scene(&left, json!({ "component": "A", "hold": 2.5 }));
    assert_eq!(kinds(&p), Vec::<&str>::new());
}

#[test]
fn a_math_left_with_a_piece_of_a_rewrite_is_a_fragment() {
    let t = Temp::new("fragment");
    let math = r#"<Write at={{ a: 0, b: 99 }}><Math key="eq" tex="\k{a}{=} \k{b}{1 + 2 + 3 + 4 + 5}" style={{ fontSize: 40 }} /></Write>"#;
    let placed =
        format!("<Box style={{{{ position: \"absolute\", left: 300, top: 150 }}}}>{math}</Box>");
    let p = t.scene(&spread(&placed), json!({ "component": "A", "hold": 2 }));
    let w = warnings(&p);
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].0, "fragment");
    assert!(
        w[0].1
            .starts_with("`eq` ends showing 1 of its 10 pieces (`a`)"),
        "{}",
        w[0].1
    );
    // A tiny mark alone at the end.
    let p = t.scene(
        &spread(&boxed("dot", "400", "350", 4)),
        json!({ "component": "A", "hold": 2 }),
    );
    let w = warnings(&p);
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].1.starts_with("`dot` is a 4x4 px mark"), "{}", w[0].1);
}

#[test]
fn warnings_do_not_fail_lint_and_print_with_their_level() {
    let t = Temp::new("levels");
    t.narrate("one two three four five six");
    let p = t.scene(&spread(""), json!({ "id": "a", "component": "A" }));
    let report = p.lint().unwrap();
    let errors = report
        .issues
        .iter()
        .filter(|i| i.level == LintLevel::Error && !common::component_rule(&i.to_string()));
    assert_eq!(errors.count(), 0, "{report}");
    let warning: &LintIssue = report
        .issues
        .iter()
        .find(|i| i.level == LintLevel::Warning)
        .unwrap();
    assert_eq!(warning.error.kind(), "still");
    assert_eq!(warning.group, Some(("still span", "0.2-6.2 s".to_owned())));
    assert!(
        warning
            .to_string()
            .starts_with("warning: scene 0 `a` (A): nothing on screen changes"),
        "{warning}"
    );
}

#[test]
fn a_scoped_lint_reports_only_its_scene_and_component() {
    let t = Temp::new("scope");
    t.file(
        "components/A.tsx",
        "export const props = {};\nexport default () => <Box style={{ flexGrow: 1 }}><Box style={{ width: 600, height: 300, fill: \"#5e6ad2\" }} /></Box>;\n",
    )
    .video(json!([
        { "id": "gone", "component": "Missing", "hold": 1 },
        { "id": "here", "component": "A", "hold": 1 }
    ]));
    let p = Project::load(&t.0).unwrap();
    let all = p.lint().unwrap();
    assert!(all.issues.iter().any(|i| i.scene == Some(0)), "{all}");
    let one = p.lint_scene(1).unwrap();
    let lines: Vec<String> = one
        .issues
        .iter()
        .map(ToString::to_string)
        .filter(|l| !common::component_rule(l))
        .collect();
    assert!(lines.is_empty(), "{lines:?}");
    let zero = p.lint_scene(0).unwrap();
    assert_eq!(zero.errors(), 1, "{zero}");
    assert_eq!(zero.issues[0].error.kind(), "component_not_found");
    assert!(p.lint_scene(2).is_err());
}

#[test]
fn beats_group_numbered_keys_and_merge_a_move() {
    let t = Temp::new("beats");
    let dots: String = (0..4)
        .map(|i| boxed(&format!("d{i}"), &format!("{}", 100 + i * 150), "100", 40))
        .collect();
    let mover = boxed("m", "100 + clamp(t - 1, 0, 1) * 400", "300", 40);
    let late = format!("{{t > 2.5 ? {} : null}}", boxed("late", "600", "300", 40));
    let p = t.scene(
        &format!("<Box style={{{{ flexGrow: 1 }}}}>{dots}{mover}{late}</Box>"),
        json!({ "component": "A", "hold": 3 }),
    );
    let look = p.look(&[0]).pop().unwrap().unwrap();
    let lines: Vec<String> = look.beats().iter().map(|b| b.line()).collect();
    let text = lines.join("\n");
    assert!(
        lines[0].starts_with("  0.00  appear   d0-d3  (centre (345,120))"),
        "{text}"
    );
    assert!(
        text.contains("  1.00  move     m  (120,320) → (520,320) over 1.0 s"),
        "{text}"
    );
    assert!(
        text.contains("  2.60  appear   late  at (620,320) 40x40"),
        "{text}"
    );
    assert_eq!(lines.len(), 4, "{text}");
}

#[test]
fn a_pause_is_not_still_and_is_a_beat() {
    let t = Temp::new("pause");
    let manifest = |text: &str, pauses: Value| {
        let words = json!([
            { "text": "One.", "start": 0.0, "end": 0.8, "confidence": -0.5 },
            { "text": "Two.", "start": 3.0, "end": 3.8, "confidence": -0.5 }
        ]);
        json!({ "version": 1, "scenes": { "a": {
            "key": "k", "text": text, "audio": "k.wav", "duration": 4.0, "words": words,
            "pauses": pauses } } })
        .to_string()
    };
    // Four seconds of narration over one frame, two of them a pause.
    t.file("script.md", "## a\nOne. [pause 2]\nTwo.\n").file(
        "narration/manifest.json",
        &manifest(
            "One. [pause 2] Two.",
            json!([{ "after": 0, "start": 1.0, "end": 3.0 }]),
        ),
    );
    let p = t.scene(&spread(""), json!({ "id": "a", "component": "A" }));
    assert_eq!(kinds(&p), Vec::<&str>::new());
    let look = p.look(&[0]).pop().unwrap().unwrap();
    let pause = look
        .beats()
        .into_iter()
        .find(|b| b.event == "pause")
        .unwrap();
    assert_eq!(pause.line(), "  1.25  pause    2.0 s  \"One.\"");
    t.file("script.md", "## a\nOne. Two.\n")
        .file("narration/manifest.json", &manifest("One. Two.", json!([])));
    let p = t.scene(&spread(""), json!({ "id": "a", "component": "A" }));
    assert_eq!(kinds(&p), ["still"], "without the pause it is still");
}
