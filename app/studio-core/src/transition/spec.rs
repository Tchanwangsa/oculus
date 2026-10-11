//! How a transition is written: a scene's `transition`, `video.json`'s
//! `motion` block and the theme's `motion` tokens share these shapes. Values
//! are checked when the cascade resolves them (`cascade`), so lint can name
//! the level a bad one came from.

use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransitionSpec {
    /// cut, dissolve, push, wipe, zoom or magic.
    pub kind: Option<String>,
    /// Seconds, played over the start of the incoming scene.
    pub dur: Option<f64>,
    /// A theme easing name, a prelude easing name or `[x1, y1, x2, y2]`.
    pub easing: Option<Value>,
    /// left, right, up or down (push and wipe).
    pub direction: Option<String>,
    /// oklch or oklab, for a magic move into this scene.
    #[serde(rename = "morphColor")]
    pub morph_color: Option<String>,
}

/// `video.json`'s top-level `motion` and the theme's motion defaults.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MotionSpec {
    pub transition: Option<TransitionSpec>,
    /// auto or off.
    pub magic_move: Option<String>,
    /// oklch or oklab: how magic moves and `<Morph>` blend colours.
    pub morph_color: Option<String>,
}
