//! `video.json`'s shape and the checks `Project::load` makes on it.

use crate::render::Size;
use crate::transition::{MotionSpec, TransitionSpec};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// `video.json`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Video {
    pub size: Size,
    /// A bundled theme name, or a `.json` path relative to the project.
    pub theme: String,
    pub scenes: Vec<SceneSpec>,
    /// Transition defaults for this video, over the theme's.
    #[serde(default)]
    pub motion: MotionSpec,
    /// The narration voice for this video, over Settings -> Voice. Only
    /// stored here; the app's narrator reads it.
    #[serde(default)]
    pub voice: Option<Voice>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSpec {
    /// Names the scene's `## id` section in script.md; required when the
    /// project has a script.md.
    #[serde(default)]
    pub id: Option<String>,
    pub component: String,
    #[serde(default)]
    pub props: Map<String, Value>,
    /// Seconds: a silent scene's length, a narrated scene's minimum.
    #[serde(default)]
    pub hold: Option<f64>,
    /// How this scene comes in, over the video's and theme's defaults.
    #[serde(default)]
    pub transition: Option<TransitionSpec>,
}

/// `video.json`'s `voice`: each field overrides the same Settings -> Voice
/// field for this video.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Voice {
    pub model: Option<String>,
    pub voice: Option<String>,
    pub speed: Option<f64>,
}

/// A scene or script section id: lowercase letters, digits and `-`.
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The checks serde cannot make. `scripted` says the project has a
/// script.md, which makes every scene's `id` required.
pub(super) fn check(video: &Video, scripted: bool) -> Result<(), String> {
    let size = video.size;
    if size.width == 0 || size.height == 0 || size.width > 8192 || size.height > 8192 {
        return Err(format!(
            "size {}x{} must be 1 to 8192 px a side",
            size.width, size.height
        ));
    }
    if video.scenes.is_empty() {
        return Err("scenes is empty; a video needs at least one scene".to_owned());
    }
    let mut ids: HashMap<&str, usize> = HashMap::new();
    for (i, s) in video.scenes.iter().enumerate() {
        if let Some(hold) = s.hold
            && !(hold.is_finite() && hold > 0.0)
        {
            return Err(format!(
                "scenes[{i}].hold is {hold}; it must be a positive number of seconds"
            ));
        }
        match s.id.as_deref() {
            Some(id) if !valid_id(id) => {
                return Err(format!(
                    "scenes[{i}].id `{id}` must be lowercase letters, digits and `-`"
                ));
            }
            Some(id) => {
                if let Some(first) = ids.insert(id, i) {
                    return Err(format!(
                        "scenes[{i}].id `{id}` is also scenes[{first}].id; ids are unique"
                    ));
                }
            }
            None if scripted => {
                return Err(format!(
                    "scenes[{i}] has no id; with a script.md every scene needs one, to match its `## id` section (or to say it has none)"
                ));
            }
            None => {}
        }
    }
    if let Some(speed) = video.voice.as_ref().and_then(|v| v.speed)
        && !(speed.is_finite() && speed > 0.0)
    {
        return Err(format!(
            "voice.speed is {speed}; it must be a positive number"
        ));
    }
    Ok(())
}
