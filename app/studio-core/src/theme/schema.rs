//! The theme file's shape. Sections are open maps of named tokens, but the
//! section names and each token's shape are fixed.

use crate::fonts::{MONO_FAMILY, bundled_families};
use crate::project::MAX_PAUSE;
use crate::scene::{Color, Style};
use crate::transition::{MotionSpec, TransitionSpec};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    color: BTreeMap<String, Color>,
    font: Fonts,
    #[serde(rename = "type")]
    type_scale: BTreeMap<String, Map<String, Value>>,
    space: BTreeMap<String, f64>,
    radius: BTreeMap<String, f64>,
    motion: Motion,
    narration: NarrationTokens,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fonts {
    heading: String,
    body: String,
    /// Optional in a file: `fill_defaults` supplies it first, so a theme
    /// written before the monospace face existed still loads.
    mono: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Motion {
    duration: BTreeMap<String, f64>,
    /// cubic-bezier control points, for `cubicBezier(...theme.motion.easing.x)`.
    easing: BTreeMap<String, [f64; 4]>,
    #[allow(dead_code)] // read only to validate the shape
    spring: BTreeMap<String, Spring>,
    /// Scene transition defaults; values are checked where they are used.
    #[serde(default)]
    transition: Option<TransitionSpec>,
    #[serde(default, rename = "magicMove")]
    magic_move: Option<String>,
    #[serde(default, rename = "morphColor")]
    morph_color: Option<String>,
    #[serde(default, rename = "motionBlur")]
    motion_blur: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // read only to validate the shape
struct Spring {
    stiffness: f64,
    damping: f64,
    mass: f64,
}

/// Seconds of silence in a scene's narration: the clip starts `lead` into
/// the scene, the scene lasts at least `tail` past its end, and a bare
/// `[pause]` in script.md is `pause` long.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NarrationTokens {
    pub lead: f64,
    pub tail: f64,
    /// Optional in a file, so a theme copied out before pauses existed
    /// still loads.
    #[serde(default = "default_pause")]
    pub pause: f64,
}

/// A bare `[pause]`'s seconds when a theme sets none.
pub const DEFAULT_PAUSE: f64 = 1.0;

fn default_pause() -> f64 {
    DEFAULT_PAUSE
}

/// Adds the tokens a theme written before they existed lacks, so components
/// can always read `theme.font.mono` and `theme.type.code`. Called on the
/// parsed file before `check`, so the filled values are validated too.
pub fn fill_defaults(tokens: &mut Value) {
    let Some(root) = tokens.as_object_mut() else {
        return;
    };
    if let Some(font) = root.get_mut("font").and_then(Value::as_object_mut) {
        font.entry("mono").or_insert_with(|| MONO_FAMILY.into());
    }
    if let Some(types) = root.get_mut("type").and_then(Value::as_object_mut) {
        types.entry("code").or_insert_with(|| {
            serde_json::json!({
                "fontFamily": MONO_FAMILY,
                "fontSize": 28,
                "fontWeight": 400,
                "lineHeight": 1.5,
                "letterSpacing": 0
            })
        });
    }
}

/// Type-scale entries are style fragments, spread into `style`.
const TYPE_KEYS: &[&str] = &[
    "fontFamily",
    "fontSize",
    "fontWeight",
    "lineHeight",
    "letterSpacing",
    "fontVariantNumeric",
];

pub struct Checked {
    pub background: Color,
    pub text: Color,
    pub body_font: String,
    pub motion: MotionSpec,
    pub easings: BTreeMap<String, [f64; 4]>,
    pub narration: NarrationTokens,
}

pub fn check(tokens: &Value) -> Result<Checked, String> {
    let file = ThemeFile::deserialize(tokens).map_err(|e| e.to_string())?;
    let families = bundled_families();
    let font_ok = |f: &str, what: &str| {
        if families.iter().any(|x| x == f) {
            Ok(())
        } else {
            Err(format!(
                "{what}: unknown font family `{f}`; bundled families: {}",
                families.join(", ")
            ))
        }
    };
    font_ok(&file.font.heading, "font.heading")?;
    font_ok(&file.font.body, "font.body")?;
    font_ok(&file.font.mono, "font.mono")?;
    for (name, entry) in &file.type_scale {
        for k in entry.keys() {
            if !TYPE_KEYS.contains(&k.as_str()) {
                return Err(format!(
                    "type.{name}.{k}: a type token may only set {}",
                    TYPE_KEYS.join(", ")
                ));
            }
        }
        let style = Style::from_json(entry).map_err(|e| format!("type.{name}: {e}"))?;
        if let Some(f) = &style.font_family {
            font_ok(f, &format!("type.{name}.fontFamily"))?;
        }
    }
    for (section, map) in [("space", &file.space), ("radius", &file.radius)] {
        if let Some((k, v)) = map.iter().find(|(_, v)| **v < 0.0) {
            return Err(format!("{section}.{k} = {v}: must not be negative"));
        }
    }
    if let Some((k, v)) = file.motion.duration.iter().find(|(_, v)| **v < 0.0) {
        return Err(format!("motion.duration.{k} = {v}: must not be negative"));
    }
    let n = file.narration;
    for (k, v) in [("lead", n.lead), ("tail", n.tail)] {
        if !(v.is_finite() && v >= 0.0) {
            return Err(format!(
                "narration.{k} = {v}: must be a non-negative number of seconds"
            ));
        }
    }
    if !(n.pause > 0.0 && n.pause <= MAX_PAUSE) {
        return Err(format!(
            "narration.pause = {}: must be more than 0 and at most {MAX_PAUSE} seconds",
            n.pause
        ));
    }
    let background = *file
        .color
        .get("bg")
        .ok_or("color.bg is required: it is the canvas under every frame")?;
    let text = *file
        .color
        .get("text")
        .ok_or("color.text is required: it is the colour of Text without a style color")?;
    Ok(Checked {
        background,
        text,
        body_font: file.font.body,
        motion: MotionSpec {
            transition: file.motion.transition,
            magic_move: file.motion.magic_move,
            morph_color: file.motion.morph_color,
            motion_blur: file.motion.motion_blur,
        },
        easings: file.motion.easing,
        narration: n,
    })
}
