//! `video.json`'s shape and the checks `Project::load` makes on it.

use crate::render::Size;
use crate::transition::{MotionSpec, TransitionSpec};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

/// `video.json`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Video {
    pub size: Size,
    /// Frames per second, one of `FRAME_RATES`. Every scene's length rounds
    /// up to whole frames at this rate (`Timeline`).
    #[serde(default = "default_fps", deserialize_with = "deserialize_fps")]
    pub fps: u32,
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
    /// The course this video belongs to: its library (`Library::subjects`)
    /// is read before the global one.
    #[serde(default)]
    pub subject: Option<String>,
    /// Burn word-highlight captions into the frames, styled by the theme's
    /// `captions` tokens (`crate::captions`). Off when absent.
    #[serde(default)]
    pub captions: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

/// The frame rates `video.json`'s `fps` takes.
pub const FRAME_RATES: [u32; 5] = [24, 25, 30, 50, 60];

fn default_fps() -> u32 {
    30
}

/// Any JSON number, so 29.97 is refused with the rates rather than as a
/// type error.
fn deserialize_fps<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let fps = f64::deserialize(d)?;
    check_fps(fps).map_err(serde::de::Error::custom)
}

fn check_fps(fps: f64) -> Result<u32, String> {
    match FRAME_RATES.iter().find(|&&r| f64::from(r) == fps) {
        Some(&r) => Ok(r),
        None => {
            let rates: Vec<String> = FRAME_RATES.iter().map(u32::to_string).collect();
            Err(format!(
                "fps is {fps}; it must be one of {} (default 30)",
                rates.join(", ")
            ))
        }
    }
}

/// `video.json`'s `voice`: each field overrides the same Settings -> Voice
/// field for this video. `model` is the server engine's only.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Voice {
    pub engine: Option<VoiceEngine>,
    pub model: Option<String>,
    pub voice: Option<String>,
    pub speed: Option<f64>,
}

/// What makes a narration clip: an OpenAI-compatible TTS server, or Kokoro
/// run inside the app.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum VoiceEngine {
    Server,
    Kokoro,
}

impl VoiceEngine {
    /// The name video.json, the settings row and the CLI use.
    pub fn as_str(self) -> &'static str {
        match self {
            VoiceEngine::Server => "server",
            VoiceEngine::Kokoro => "kokoro",
        }
    }

    /// The engine `name` names, if any.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "server" => Some(VoiceEngine::Server),
            "kokoro" => Some(VoiceEngine::Kokoro),
            _ => None,
        }
    }
}

/// A scene or script section id: lowercase letters, digits and `-`.
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A subject is a course code's folder name: letters, digits, `-` and `_`,
/// at most 64 characters.
pub fn valid_subject(subject: &str) -> bool {
    !subject.is_empty()
        && subject.len() <= 64
        && subject
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
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
    check_fps(f64::from(video.fps))?;
    if let Some(subject) = &video.subject
        && !valid_subject(subject)
    {
        return Err(format!(
            "subject `{subject}` must be a course code's folder name: letters, digits, `-` and `_`, up to 64 characters"
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
