//! The theme file's shape. Sections are open maps of named tokens, but the
//! section names and each token's shape are fixed.

use crate::fonts::bundled_families;
use crate::scene::{Color, Style};
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
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fonts {
    heading: String,
    body: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // read only to validate the shape
struct Motion {
    duration: BTreeMap<String, f64>,
    /// cubic-bezier control points, for `cubicBezier(...theme.motion.easing.x)`.
    easing: BTreeMap<String, [f64; 4]>,
    spring: BTreeMap<String, Spring>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // read only to validate the shape
struct Spring {
    stiffness: f64,
    damping: f64,
    mass: f64,
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
    })
}
