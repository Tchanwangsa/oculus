//! The theme's `captions` tokens: how burned-in captions look. A theme
//! written before captions existed gets the defaults (`fill_defaults`),
//! taken from its own body font and colour tokens.

use crate::scene::Color;
use serde::Deserialize;
use serde_json::{Map, Value, json};

/// Where the caption plate sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Bottom,
    Top,
}

/// How the word being said stands out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Highlight {
    /// The word takes the highlight colour.
    Color,
    /// A pill of the highlight colour sits behind the word, which takes
    /// `highlightText`.
    Pill,
}

/// The resolved `captions` tokens. Sizes are px on a 1080 px short side and
/// scale with the frame's shorter side, so a portrait video gets the same
/// size of text as a landscape one.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptionStyle {
    pub font_family: String,
    pub font_size: f64,
    pub font_weight: f64,
    /// Words already said, and every word's colour before the highlight.
    pub color: Color,
    /// The opacity of words not yet said.
    pub upcoming: f64,
    pub highlight: Color,
    pub highlight_text: Color,
    pub highlight_style: Highlight,
    /// The plate behind the lines, its opacity applied; `None` for none.
    pub plate: Option<Color>,
    pub placement: Placement,
    /// The plate's distance from the frame's edge, as a fraction of the
    /// frame's height.
    pub margin: f64,
    /// Characters per line on a 16:9 frame; a narrower frame fits fewer
    /// (`CaptionStyle::line_chars`).
    pub max_chars: usize,
    pub max_lines: usize,
}

/// The file's shape once `fill_defaults` has run.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Tokens {
    font_family: String,
    font_size: f64,
    font_weight: f64,
    color: String,
    upcoming: f64,
    highlight: String,
    highlight_text: String,
    highlight_style: String,
    plate: String,
    plate_opacity: f64,
    position: String,
    margin: f64,
    max_chars: f64,
    max_lines: f64,
}

/// Fills the `captions` keys a theme leaves out, from its `font.body` and
/// its `ink`, `onInk`, `accent` and `onAccent` colours (or plain colours
/// when it has none of those).
pub(crate) fn fill_defaults(root: &mut Map<String, Value>) {
    let body = root
        .get("font")
        .and_then(|f| f.get("body"))
        .and_then(Value::as_str)
        .unwrap_or("Inter")
        .to_owned();
    let has = |name: &str| root.get("color").and_then(|c| c.get(name)).is_some();
    let token_or = |name: &str, hex: &str| if has(name) { name } else { hex }.to_owned();
    let defaults = json!({
        "fontFamily": body,
        "fontSize": 44,
        "fontWeight": 600,
        "color": token_or("onInk", "#ffffff"),
        "upcoming": 0.6,
        "highlight": token_or("accent", "#5e6ad2"),
        "highlightText": token_or("onAccent", "#ffffff"),
        "highlightStyle": "pill",
        "plate": token_or("ink", "#101018"),
        "plateOpacity": 0.8,
        "position": "bottom",
        "margin": 0.05,
        "maxChars": 42,
        "maxLines": 2
    });
    let Some(captions) = root
        .entry("captions")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
    else {
        return;
    };
    for (k, v) in defaults.as_object().into_iter().flatten() {
        captions.entry(k.clone()).or_insert_with(|| v.clone());
    }
}

/// Checks the filled `captions` section: `colors` is the theme's `color`
/// section, which a colour token may name; `families` the bundled fonts.
pub(crate) fn check(
    captions: &Value,
    colors: &Map<String, Value>,
    families: &[String],
) -> Result<CaptionStyle, String> {
    let t = Tokens::deserialize(captions).map_err(|e| format!("captions: {e}"))?;
    if !families.contains(&t.font_family) {
        return Err(format!(
            "captions.fontFamily: unknown font family `{}`; bundled families: {}",
            t.font_family,
            families.join(", ")
        ));
    }
    let range = |name: &str, v: f64, lo: f64, hi: f64| {
        if v.is_finite() && (lo..=hi).contains(&v) {
            Ok(v)
        } else {
            Err(format!("captions.{name} = {v}: must be {lo} to {hi}"))
        }
    };
    let whole = |name: &str, v: f64, lo: f64, hi: f64| {
        range(name, v, lo, hi).and_then(|v| {
            if v.fract() == 0.0 {
                Ok(v as usize)
            } else {
                Err(format!("captions.{name} = {v}: must be a whole number"))
            }
        })
    };
    let colour =
        |name: &str, v: &str| resolve(v, colors).map_err(|e| format!("captions.{name}: {e}"));
    let plate_opacity = range("plateOpacity", t.plate_opacity, 0.0, 1.0)?;
    let plate = match t.plate.as_str() {
        "none" => None,
        v => Some(colour("plate", v)?.with_alpha_mul(plate_opacity as f32)),
    };
    Ok(CaptionStyle {
        font_size: range("fontSize", t.font_size, 8.0, 200.0)?,
        font_weight: range("fontWeight", t.font_weight, 100.0, 1000.0)?,
        font_family: t.font_family,
        color: colour("color", &t.color)?,
        upcoming: range("upcoming", t.upcoming, 0.0, 1.0)?,
        highlight: colour("highlight", &t.highlight)?,
        highlight_text: colour("highlightText", &t.highlight_text)?,
        highlight_style: match t.highlight_style.as_str() {
            "color" => Highlight::Color,
            "pill" => Highlight::Pill,
            v => {
                return Err(format!(
                    "captions.highlightStyle = `{v}`: must be `color` or `pill`"
                ));
            }
        },
        plate,
        placement: match t.position.as_str() {
            "bottom" => Placement::Bottom,
            "top" => Placement::Top,
            v => {
                return Err(format!(
                    "captions.position = `{v}`: must be `bottom` or `top`"
                ));
            }
        },
        margin: range("margin", t.margin, 0.0, 0.4)?,
        max_chars: whole("maxChars", t.max_chars, 8.0, 120.0)?,
        max_lines: whole("maxLines", t.max_lines, 1.0, 4.0)?,
    })
}

/// A `color` token's name or a `#hex` colour.
fn resolve(v: &str, colors: &Map<String, Value>) -> Result<Color, String> {
    if v.starts_with('#') {
        return Color::parse(v);
    }
    match colors.get(v).and_then(Value::as_str) {
        Some(hex) => Color::parse(hex),
        None => Err(format!(
            "`{v}` is neither a #hex colour nor a token in this theme's `color`"
        )),
    }
}

impl CaptionStyle {
    /// Output px per token px: the frame's shorter side over 1080.
    pub fn unit(width: u32, height: u32) -> f64 {
        f64::from(width.min(height)) / 1080.0
    }

    /// Characters a line holds on a `width` x `height` frame: `max_chars`
    /// on 16:9 or wider, fewer in proportion on a narrower frame (24 of 42
    /// on 9:16), never under 12.
    pub fn line_chars(&self, width: u32, height: u32) -> usize {
        let aspect = f64::from(width) / f64::from(width.min(height));
        let fit = (self.max_chars as f64 * aspect / (16.0 / 9.0)).round() as usize;
        fit.clamp(12.min(self.max_chars), self.max_chars)
    }
}
