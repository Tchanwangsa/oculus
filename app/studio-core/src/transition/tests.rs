//! The cascade (scene, then video.json motion, then theme), auto magic and
//! the level a bad value is reported at.

use super::cascade::{Direction, Level};
use super::*;
use crate::easing::Easing;
use crate::theme::Theme;
use serde_json::json;
use std::path::Path;

fn spec(v: serde_json::Value) -> TransitionSpec {
    serde_json::from_value(v).unwrap()
}

fn motion(v: serde_json::Value) -> MotionSpec {
    serde_json::from_value(v).unwrap()
}

fn theme_with(edit: impl Fn(&mut serde_json::Value)) -> Theme {
    let mut v: serde_json::Value =
        serde_json::from_str(include_str!("../../themes/default.json")).unwrap();
    edit(&mut v);
    Theme::parse(&v.to_string(), Path::new("t.json")).unwrap()
}

#[test]
fn the_bundled_theme_dissolves_and_the_first_scene_never_transitions() {
    let theme = Theme::bundled("default").unwrap();
    let none = MotionSpec::default();
    assert_eq!(
        resolve(0, Some(&spec(json!({ "kind": "push" }))), &none, &theme),
        Ok(None)
    );
    let t = resolve(1, None, &none, &theme).unwrap().unwrap();
    assert_eq!(
        (t.look, t.dur, t.easing),
        (Look::Dissolve, 0.7, Easing::InOutCubic)
    );
    assert_eq!(t.direction, Direction::Left);
    assert_eq!(t.dur_level, Level::Theme);
    assert!(t.auto_magic);
}

#[test]
fn a_theme_without_transition_tokens_gets_the_built_in_default() {
    let theme = theme_with(|v| {
        let m = v["motion"].as_object_mut().unwrap();
        m.remove("transition");
        m.remove("magicMove");
    });
    let t = resolve(1, None, &MotionSpec::default(), &theme)
        .unwrap()
        .unwrap();
    assert_eq!(
        (t.look, t.dur, t.dur_level),
        (Look::Dissolve, 0.7, Level::Default)
    );
    assert!(t.auto_magic);
}

#[test]
fn video_motion_overrides_the_theme_and_the_scene_wins_field_by_field() {
    let theme = Theme::bundled("default").unwrap();
    let video =
        motion(json!({ "transition": { "kind": "push", "dur": 0.4, "easing": "standard" } }));
    let t = resolve(2, None, &video, &theme).unwrap().unwrap();
    assert_eq!(
        (t.look, t.dur, t.dur_level),
        (Look::Push, 0.4, Level::Video)
    );
    assert_eq!(t.easing, Easing::Bezier([0.2, 0.0, 0.0, 1.0]));
    let scene = spec(json!({ "kind": "wipe", "direction": "up", "easing": [0.1, 0.2, 0.3, 1] }));
    let t = resolve(2, Some(&scene), &video, &theme).unwrap().unwrap();
    assert_eq!(
        (t.look, t.direction, t.dur),
        (Look::Wipe, Direction::Up, 0.4)
    );
    assert_eq!(t.easing, Easing::Bezier([0.1, 0.2, 0.3, 1.0]));
    // A scene with its own transition never turns into magic by itself.
    assert!(!t.auto_magic);
}

#[test]
fn magic_move_off_at_either_level_stops_auto_magic() {
    let theme = Theme::bundled("default").unwrap();
    let off = motion(json!({ "magicMove": "off" }));
    assert!(!resolve(1, None, &off, &theme).unwrap().unwrap().auto_magic);
    let theme = theme_with(|v| v["motion"]["magicMove"] = json!("off"));
    assert!(
        !resolve(1, None, &MotionSpec::default(), &theme)
            .unwrap()
            .unwrap()
            .auto_magic
    );
    let on = motion(json!({ "magicMove": "auto" }));
    assert!(resolve(1, None, &on, &theme).unwrap().unwrap().auto_magic);
}

#[test]
fn bad_values_name_the_level_they_came_from() {
    let theme = Theme::bundled("default").unwrap();
    let none = MotionSpec::default();
    let err = |scene: Option<serde_json::Value>, video: &MotionSpec, theme: &Theme| {
        let s = scene.map(spec);
        resolve(3, s.as_ref(), video, theme)
            .unwrap_err()
            .to_string()
    };
    assert_eq!(
        err(Some(json!({ "kind": "fade" })), &none, &theme),
        "scene 3 transition.kind: unknown transition `fade`; use cut, dissolve, push, wipe, zoom, magic"
    );
    let e = err(None, &motion(json!({ "transition": { "dur": 0 } })), &theme);
    assert_eq!(
        e,
        "video.json motion.transition.dur: 0 is not a positive number of seconds"
    );
    let bad = theme_with(|v| v["motion"]["transition"]["easing"] = json!("standrd"));
    assert_eq!(
        err(None, &none, &bad),
        "theme motion.transition.easing: unknown easing `standrd` (did you mean `standard`?); use a theme.motion.easing name, a prelude easing or [x1, y1, x2, y2] (enter, exit, standard, linear, easeOutCubic, easeInOutCubic)"
    );
    let e = err(Some(json!({ "direction": "sideways" })), &none, &theme);
    assert!(
        e.starts_with(
            "scene 3 transition.direction: unknown direction `sideways`; use left, right, up, down"
        ),
        "{e}"
    );
    let e = err(Some(json!({ "easing": [2, 0, 0, 1] })), &none, &theme);
    assert!(
        e.starts_with("scene 3 transition.easing: an array is not cubic-bezier points"),
        "{e}"
    );
    let e = err(None, &motion(json!({ "magicMove": "on" })), &theme);
    assert_eq!(
        e,
        "video.json motion.magicMove: unknown magicMove `on`; use auto or off"
    );
}

#[test]
fn lint_checks_the_first_scene_and_the_hold() {
    let theme = Theme::bundled("default").unwrap();
    let push = spec(json!({ "kind": "push" }));
    let e = check_hold(0, Some(&push), None, 2.0)
        .unwrap_err()
        .to_string();
    assert_eq!(
        e,
        "scene 0 transition: the first scene has nothing to transition from; remove it"
    );
    let t = resolve(1, None, &MotionSpec::default(), &theme).unwrap();
    let e = check_hold(1, None, t.as_ref(), 0.5)
        .unwrap_err()
        .to_string();
    assert!(
        e.starts_with("theme motion.transition.dur: 0.7 s is longer than scene 1's hold of 0.5 s"),
        "{e}"
    );
    assert!(check_hold(1, None, t.as_ref(), 0.7).is_ok());
    // A cut that cannot turn into magic changes nothing over time.
    let cut = spec(json!({ "kind": "cut", "dur": 5 }));
    let t = resolve(1, Some(&cut), &MotionSpec::default(), &theme).unwrap();
    assert!(check_hold(1, Some(&cut), t.as_ref(), 0.5).is_ok());
}

#[test]
fn morph_color_cascades_and_names_the_level_of_a_bad_value() {
    use crate::morph::MorphColor;
    use crate::transition::morph_color;
    let theme = Theme::bundled("default").unwrap();
    let none = MotionSpec::default();
    let get = |scene: Option<serde_json::Value>, video: &MotionSpec, theme: &Theme| {
        let s = scene.map(spec);
        resolve(2, s.as_ref(), video, theme).map(|t| t.unwrap().morph_color)
    };
    assert_eq!(get(None, &none, &theme), Ok(MorphColor::Oklch));
    let lab = motion(json!({ "morphColor": "oklab" }));
    assert_eq!(get(None, &lab, &theme), Ok(MorphColor::Oklab));
    assert_eq!(morph_color(None, &lab, &theme), Ok(MorphColor::Oklab));
    let scene = json!({ "morphColor": "oklch" });
    assert_eq!(get(Some(scene), &lab, &theme), Ok(MorphColor::Oklch));
    let lab_theme = theme_with(|v| v["motion"]["morphColor"] = json!("oklab"));
    assert_eq!(get(None, &none, &lab_theme), Ok(MorphColor::Oklab));
    assert_eq!(morph_color(None, &none, &lab_theme), Ok(MorphColor::Oklab));

    let err = |scene, video: &MotionSpec, theme: &Theme| {
        get(scene, video, theme).unwrap_err().to_string()
    };
    assert_eq!(
        err(Some(json!({ "morphColor": "hsl" })), &none, &theme),
        "scene 2 transition.morphColor: unknown morphColor `hsl`; use oklch, oklab"
    );
    let bad = motion(json!({ "morphColor": "oklb" }));
    assert_eq!(
        err(None, &bad, &theme),
        "video.json motion.morphColor: unknown morphColor `oklb` (did you mean `oklab`?); use oklch, oklab"
    );
    let bad_theme = theme_with(|v| v["motion"]["morphColor"] = json!("rgb"));
    assert!(
        err(None, &none, &bad_theme)
            .starts_with("theme motion.morphColor: unknown morphColor `rgb`"),
        "{}",
        err(None, &none, &bad_theme)
    );
}
