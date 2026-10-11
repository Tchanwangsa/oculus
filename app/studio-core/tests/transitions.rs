//! Scene transitions through the renderer: timing over the incoming scene's
//! head, the cascade and automatic magic move, the changed-text halves, and
//! lint naming the level a bad value came from.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::fixture;
use serde_json::{Map, Value, json};
use std::path::PathBuf;
use studio_core::{Engine, Frame, Project, RenderOptions, Renderer, SceneCall, StudioError, Time};

/// A throwaway project: `video.json` and its components.
fn project(name: &str, video: Value, components: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("studio-{name}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("components")).unwrap();
    std::fs::write(dir.join("video.json"), video.to_string()).unwrap();
    for (n, tsx) in components {
        std::fs::write(dir.join("components").join(format!("{n}.tsx")), tsx).unwrap();
    }
    dir
}

/// The magic fixture's video.json with scene 1's transition set.
fn magic_with(name: &str, transition: Option<Value>) -> PathBuf {
    let mut video: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture("magic").join("video.json")).unwrap(),
    )
    .unwrap();
    if let Some(t) = transition {
        video["scenes"][1]["transition"] = t;
    }
    let tsx = std::fs::read_to_string(fixture("magic").join("components/Deck.tsx")).unwrap();
    project(name, video, &[("Deck", &tsx)])
}

fn frame(dir: &PathBuf, index: usize, t: f64) -> Frame {
    let p = Project::load(dir).unwrap();
    Renderer::new(&p, RenderOptions::default())
        .unwrap()
        .frame(Time::Scene { index, t })
        .unwrap()
}

#[test]
fn changed_text_is_gone_at_the_halfway_point() {
    // The title box's rect is fixed in both steps: (40, 36, 400 x 56) and
    // (40, 64, 400 x 44). Inside it at p = 0.5 ± ε there is only background.
    let project = Project::load(fixture("magic")).unwrap();
    let mut r = Renderer::new(&project, RenderOptions::default()).unwrap();
    let bg = [0xf4, 0xf4, 0xf6];
    let ink_in_title = |f: &Frame, p: f64| {
        let (top, h) = (36.0 + 28.0 * p, 56.0 - 12.0 * p);
        let mut worst = 0u8;
        for y in (top.floor() as u32)..(top + h).ceil() as u32 {
            for x in 40..440 {
                let i = ((y * f.width + x) * 4) as usize;
                for (got, want) in f.rgba[i..i + 3].iter().zip(bg) {
                    worst = worst.max(got.abs_diff(want));
                }
            }
        }
        worst
    };
    for p in [0.49, 0.5, 0.51] {
        let f = r.frame(Time::Scene { index: 1, t: p }).unwrap();
        assert!(
            ink_in_title(&f, p) <= 1,
            "glyph ink in the title box at p = {p}"
        );
    }
    // Either side of the middle the text is there.
    for p in [0.3, 0.7] {
        let f = r.frame(Time::Scene { index: 1, t: p }).unwrap();
        assert!(ink_in_title(&f, p) > 40, "no text at p = {p}");
    }
}

#[test]
fn a_transition_plays_over_the_incoming_scenes_head_only() {
    let dir = fixture("magic");
    let project = Project::load(&dir).unwrap();
    assert_eq!(project.length().unwrap(), 4.0);
    let mut r = Renderer::new(&project, RenderOptions::default()).unwrap();
    let mut engine = Engine::new(
        project.theme.clone(),
        project.video.size,
        None,
        RenderOptions::default(),
    )
    .unwrap();
    let tsx = std::fs::read_to_string(dir.join("components/Deck.tsx")).unwrap();
    engine.load_component("Deck", &tsx).unwrap();
    let alone = |e: &mut Engine, step: i64, index: usize, t: f64| {
        let props: Map<String, Value> = json!({ "step": step }).as_object().cloned().unwrap();
        e.render(SceneCall {
            component: "Deck",
            props: &props,
            t,
            duration: 2.0,
            index,
            id: None,
            words: None,
        })
        .unwrap()
        .rgba
    };
    // Past `dur`, and all of the first scene, are the scene alone.
    assert!(r.frame(Time::Global(3.2)).unwrap().rgba == alone(&mut engine, 2, 1, 1.2));
    assert!(r.frame(Time::Global(1.9)).unwrap().rgba == alone(&mut engine, 1, 0, 1.9));
    assert!(r.frame(Time::Global(2.5)).unwrap().rgba != alone(&mut engine, 2, 1, 0.5));
}

#[test]
fn auto_magic_needs_shared_keys_and_no_transition_of_the_scenes_own() {
    let auto = frame(&fixture("magic"), 1, 0.5);
    let magic = frame(
        &magic_with("explicit-magic", Some(json!({ "kind": "magic" }))),
        1,
        0.5,
    );
    let dissolve = frame(
        &magic_with("explicit-dissolve", Some(json!({ "kind": "dissolve" }))),
        1,
        0.5,
    );
    assert!(
        auto.rgba == magic.rgba,
        "shared keys and no scene transition: magic"
    );
    assert!(auto.rgba != dissolve.rgba, "a scene's own transition wins");
    let off = magic_with("magic-off", None);
    let mut video: Value =
        serde_json::from_str(&std::fs::read_to_string(off.join("video.json")).unwrap()).unwrap();
    video["motion"]["magicMove"] = json!("off");
    std::fs::write(off.join("video.json"), video.to_string()).unwrap();
    assert!(
        frame(&off, 1, 0.5).rgba == dissolve.rgba,
        "magicMove off: the cascade's kind"
    );

    // Without a shared key the theme's default (dissolve) plays.
    let plain = r##"export const props = { c: str() };
export default ({ c }) => <Box style={{ fill: c }}><Box style={{ width: 40, height: 40, fill: "#000000" }} /></Box>;
"##;
    let video = |t: Option<Value>| {
        let mut v = json!({
            "size": { "width": 64, "height": 36 },
            "theme": "default",
            "scenes": [
                { "component": "Plain", "props": { "c": "#ff0000" }, "hold": 1 },
                { "component": "Plain", "props": { "c": "#0000ff" }, "hold": 1 }
            ]
        });
        if let Some(t) = t {
            v["scenes"][1]["transition"] = t;
        }
        v
    };
    let auto = frame(
        &project("no-keys", video(None), &[("Plain", plain)]),
        1,
        0.3,
    );
    let dissolve = frame(
        &project(
            "no-keys-dissolve",
            video(Some(json!({ "kind": "dissolve" }))),
            &[("Plain", plain)],
        ),
        1,
        0.3,
    );
    assert!(auto.rgba == dissolve.rgba);
}

const FINE: &str = r##"export const props = {};
export default () => <Box style={{ fill: "#ffffff" }}><Text key="t">Hi</Text></Box>;
"##;

fn lint(
    name: &str,
    video: Value,
    components: &[(&str, &str)],
) -> Vec<(Option<usize>, String, String)> {
    let dir = project(name, video, components);
    let report = Project::load(&dir).unwrap().lint().unwrap();
    report
        .issues
        .iter()
        .filter(|i| !common::component_rule(&i.to_string()))
        .map(|i| (i.scene, i.error.kind().to_owned(), i.error.to_string()))
        .collect()
}

fn scenes(n: usize, hold: f64) -> Vec<Value> {
    (0..n)
        .map(|_| json!({ "component": "Fine", "hold": hold }))
        .collect()
}

#[test]
fn lint_names_the_level_of_a_bad_transition_value() {
    let mut s = scenes(4, 1.0);
    s[0]["transition"] = json!({ "kind": "push" });
    s[2]["transition"] = json!({ "kind": "fade" });
    s[3]["transition"] = json!({ "kind": "dissolve", "dur": 1.5 });
    let video = json!({ "size": { "width": 64, "height": 36 }, "theme": "default", "scenes": s });
    let issues = lint("lint-levels", video, &[("Fine", FINE)]);
    let has = |scene: usize, text: &str| {
        issues
            .iter()
            .any(|(s, k, m)| *s == Some(scene) && k == "transition" && m.starts_with(text))
    };
    assert_eq!(issues.len(), 3, "{issues:#?}");
    assert!(
        has(
            0,
            "scene 0 transition: the first scene has nothing to transition from"
        ),
        "{issues:#?}"
    );
    assert!(
        has(2, "scene 2 transition.kind: unknown transition `fade`"),
        "{issues:#?}"
    );
    assert!(
        has(
            3,
            "scene 3 transition.dur: 1.5 s is longer than scene 3's length of 1 s"
        ),
        "{issues:#?}"
    );

    // A video.json value is reported once, at the first scene that uses it.
    let video = json!({
        "size": { "width": 64, "height": 36 }, "theme": "default",
        "motion": { "transition": { "easing": "bouncy", "direction": "left" } },
        "scenes": scenes(3, 1.0)
    });
    let issues = lint("lint-video", video, &[("Fine", FINE)]);
    assert_eq!(issues.len(), 1, "{issues:#?}");
    assert!(
        issues[0]
            .2
            .starts_with("video.json motion.transition.easing: unknown easing `bouncy`"),
        "{issues:#?}"
    );
    assert_eq!(issues[0].0, Some(1));

    // The theme's default dur against a short scene names the theme.
    let video = json!({ "size": { "width": 64, "height": 36 }, "theme": "default", "scenes": scenes(2, 0.5) });
    let issues = lint("lint-theme", video, &[("Fine", FINE)]);
    assert_eq!(issues.len(), 1, "{issues:#?}");
    assert!(
        issues[0].2.starts_with(
            "theme motion.transition.dur: 0.7 s is longer than scene 1's length of 0.5 s"
        ),
        "{issues:#?}"
    );
}

#[test]
fn lint_reports_duplicate_keys_and_bad_morphs_with_their_lines() {
    let dup = r##"export const props = {};
export default () => (
  <Row>
    {[0, 1].map((i) => <Box key={i} />)}
    {[0, 1].map((i) => <Box key={i} />)}
  </Row>
);
"##;
    let one = r##"export const props = {};
export default () => (
  <Box>
    <Morph p={0.5}><Box /></Morph>
  </Box>
);
"##;
    let nested = r##"export const props = {};
export default () => (
  <Morph p={0.5}>
    <Box />
    <Box><Morph p={0.2}><Box /><Box /></Morph></Box>
  </Morph>
);
"##;
    let video = json!({
        "size": { "width": 64, "height": 36 }, "theme": "default",
        "motion": { "transition": { "kind": "cut" }, "magicMove": "off" },
        "scenes": [
            { "component": "Dup", "hold": 1 },
            { "component": "One", "hold": 1 },
            { "component": "Nested", "hold": 1 }
        ]
    });
    let issues = lint(
        "lint-tree",
        video,
        &[("Dup", dup), ("One", one), ("Nested", nested)],
    );
    assert_eq!(issues.len(), 3, "{issues:#?}");
    assert_eq!(
        issues[0].2,
        "Dup.tsx:5:24: Box > Box[2]: key `0` is also on Box > Box[0]; keys must be unique in a scene's tree (not just among siblings) so magic move can match them\n    5 | {[0, 1].map((i) => <Box key={i} />)}"
    );
    assert!(
        issues[1].2.starts_with("One.tsx:4:20: TypeError: <Morph> takes exactly two elements, the layout at p = 0 and the layout at p = 1, got 1"),
        "{issues:#?}"
    );
    assert!(
        issues[2].2.starts_with("Nested.tsx:5:10: Morph > Box[1] > Morph[0]: a <Morph> inside another <Morph> is not supported"),
        "{issues:#?}"
    );
}

#[test]
fn lint_renders_the_middle_of_each_transition() {
    // Fine at both of its ends as lint samples them, but not at the moment a
    // transition holds it at.
    let late = r##"export const props = {};
export default (_p, { t, duration }) => {
  if (t > duration - 1e-4) throw new Error("held past its end");
  return <Box style={{ fill: "#ffffff" }} />;
};
"##;
    let video = json!({
        "size": { "width": 64, "height": 36 }, "theme": "default",
        "scenes": [{ "component": "Late", "hold": 1 }, { "component": "Fine", "hold": 1 }]
    });
    let issues = lint("lint-mid", video, &[("Late", late), ("Fine", FINE)]);
    assert_eq!(issues.len(), 1, "{issues:#?}");
    assert_eq!(issues[0].0, Some(1));
    assert!(issues[0].2.contains("held past its end"), "{issues:#?}");
}

#[test]
fn a_bad_transition_stops_its_scenes_frames() {
    let mut s = scenes(2, 1.0);
    s[1]["transition"] = json!({ "kind": "slide" });
    let dir = project(
        "bad-frame",
        json!({ "size": { "width": 64, "height": 36 }, "theme": "default", "scenes": s }),
        &[("Fine", FINE)],
    );
    let p = Project::load(&dir).unwrap();
    let mut r = Renderer::new(&p, RenderOptions::default()).unwrap();
    assert!(r.frame(Time::Global(0.5)).is_ok());
    let e = r.frame(Time::Global(1.8)).unwrap_err();
    assert!(matches!(e, StudioError::Transition { .. }), "{e}");
    assert_eq!(e.kind(), "transition");
}

#[test]
fn morph_color_oklab_changes_the_magic_move_and_lint_names_a_bad_one() {
    let with = |name: &str, value: &str| {
        let dir = magic_with(name, None);
        let mut video: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("video.json")).unwrap())
                .unwrap();
        video["motion"]["morphColor"] = json!(value);
        std::fs::write(dir.join("video.json"), video.to_string()).unwrap();
        dir
    };
    let default = frame(&fixture("magic"), 1, 0.5);
    let oklch = frame(&with("morph-oklch", "oklch"), 1, 0.5);
    let oklab = frame(&with("morph-oklab", "oklab"), 1, 0.5);
    assert!(default.rgba == oklch.rgba, "oklch is the default");
    assert!(
        default.rgba != oklab.rgba,
        "oklab blends the bars differently"
    );

    let bad = with("morph-bad", "okhsl");
    let report = Project::load(&bad).unwrap().lint().unwrap();
    let issues: Vec<String> = report
        .issues
        .iter()
        .map(ToString::to_string)
        .filter(|l| !common::component_rule(l))
        .collect();
    assert_eq!(issues.len(), 1, "{issues:#?}");
    assert!(
        issues[0].starts_with(
            "scene 0 (Deck): video.json motion.morphColor: unknown morphColor `okhsl`"
        ),
        "{issues:#?}"
    );
    let p = Project::load(&bad).unwrap();
    assert!(
        Renderer::new(&p, RenderOptions::default())
            .unwrap()
            .frame(Time::Global(0.5))
            .is_err()
    );
}
