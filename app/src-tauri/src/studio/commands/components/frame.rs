//! One component drawn at a moment: the playground's stage.

use std::path::Path;

use serde_json::{Map, Value};
use studio_core::{Library, SceneCall, Theme};

use super::super::themes;
use super::{engine, source};

/// The widest still asked for, as the video stills allow.
const MAX_WIDTH: u32 = 4096;

/// `ctx.duration` when the caller gives none.
const DEFAULT_DURATION: f64 = 4.0;

pub struct FrameRequest {
    pub name: String,
    pub subject: Option<String>,
    pub theme: String,
    /// A theme file's text to draw in instead of the saved `theme`: the
    /// Themes page's unsaved draft. `theme` then only names it in messages.
    pub theme_text: Option<String>,
    pub props: Option<Map<String, Value>>,
    pub example: Option<String>,
    pub t: f64,
    pub width: u32,
    pub duration: Option<f64>,
}

/// The component `request.name` from the nearest library, in the theme the
/// cascade names, with the props asked for: `props`, else the example named,
/// else the first example. The props are checked against the component's
/// schema, so a wrong one is refused with the reason, never drawn as a guess.
pub fn frame_png(library: &Library, request: FrameRequest) -> Result<Vec<u8>, String> {
    let FrameRequest {
        name,
        subject,
        theme,
        theme_text,
        props,
        example,
        t,
        width,
        duration,
    } = request;
    if !(1..=MAX_WIDTH).contains(&width) {
        return Err(format!("width {width} must be 1 to {MAX_WIDTH} px"));
    }
    let duration = duration.unwrap_or(DEFAULT_DURATION);
    if !(duration.is_finite() && duration > 0.0) {
        return Err(format!("duration {duration} s must be a positive number"));
    }
    if !(t.is_finite() && (0.0..duration).contains(&t)) {
        return Err(format!(
            "t = {t} s is outside the component's {duration} s; t must be in [0, {duration})"
        ));
    }
    let subject = subject.as_deref();
    let (_, path) = source::find(library, subject, &name)?;
    let tsx = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let theme = match theme_text {
        Some(text) => draft_theme(&theme, &text)?,
        None => themes::resolve(library, subject, &theme)?,
    };
    let scale = f64::from(width) / f64::from(engine::WIDTH);
    let mut engine = engine::new(theme, scale)?;
    let info = engine
        .load_component(&name, &tsx)
        .map_err(|e| e.to_string())?;
    let props = match (props, example) {
        (Some(props), _) => props,
        (None, Some(example)) => example_props(&name, &info.examples, &example)?,
        (None, None) => match info.examples.keys().next() {
            Some(first) => example_props(&name, &info.examples, first)?,
            None => Map::new(),
        },
    };
    let frame = engine
        .render(SceneCall {
            component: &name,
            props: &props,
            t,
            duration,
            index: 0,
            id: None,
            words: None,
        })
        .map_err(|e| e.to_string())?;
    frame.encode_png().map_err(|e| e.to_string())
}

/// A theme from unsaved text, checked as a write would check it.
fn draft_theme(name: &str, text: &str) -> Result<Theme, String> {
    themes::check_size(name, text)?;
    Theme::parse(text, Path::new(&format!("themes/{name}.json"))).map_err(|e| e.to_string())
}

fn example_props(
    component: &str,
    examples: &Map<String, Value>,
    example: &str,
) -> Result<Map<String, Value>, String> {
    match examples.get(example) {
        Some(Value::Object(props)) => Ok(props.clone()),
        Some(_) => Err(format!(
            "{component}: examples.{example} must be an object of props"
        )),
        None => Err(format!(
            "{component} has no example `{example}`; it has: {}",
            if examples.is_empty() {
                "none".to_owned()
            } else {
                examples.keys().cloned().collect::<Vec<_>>().join(", ")
            }
        )),
    }
}
