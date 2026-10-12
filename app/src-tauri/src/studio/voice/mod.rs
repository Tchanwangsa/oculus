//! Which TTS server, model, voice and speed narrate a video: the `voice`
//! settings row (Settings -> Voice), under a video's `voice` block in
//! video.json, under the CLI's flags. Keyless OpenAI-compatible servers only.

pub mod g2p;
#[cfg(test)]
mod tests;

use serde_json::Value;
use sqlx::SqlitePool;

use crate::db::store::{edit_setting, setting_blocking};

/// The settings row's key.
pub const SETTINGS_KEY: &str = "voice";

pub const DEFAULT_SPEED: f64 = 1.0;

/// What `narrate` says when no server is set anywhere.
pub const NO_SERVER: &str =
    "no TTS server: set a TTS server in Settings -> Voice, or pass --server";
/// Model and voice ids differ per server (Kokoro-FastAPI `kokoro`, mlx-audio
/// `mlx-community/Kokoro-82M-bf16`, OpenAI `tts-1`), so neither has a default.
pub const NO_MODEL: &str =
    "no TTS model: set a model in Settings -> Voice or video.json's voice block, or pass --model";
pub const NO_VOICE: &str =
    "no TTS voice: set a voice in Settings -> Voice or video.json's voice block, or pass --voice";

/// The `voice` row, `{ "baseUrl", "model", "voice", "speed" }`. A missing
/// or ill-typed field reads as unset (speed: 1.0).
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceSettings {
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub voice: Option<String>,
    pub speed: f64,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            base_url: None,
            model: None,
            voice: None,
            speed: DEFAULT_SPEED,
        }
    }
}

/// Per-field overrides: the CLI's flags, or a video's `voice` block.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VoiceOverrides {
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub voice: Option<String>,
    pub speed: Option<f64>,
}

impl From<&studio_core::Voice> for VoiceOverrides {
    fn from(v: &studio_core::Voice) -> Self {
        Self {
            base_url: None,
            model: v.model.clone(),
            voice: v.voice.clone(),
            speed: v.speed,
        }
    }
}

/// The voice a clip is made with, every field settled; `base_url` is
/// normalised (`tts::normalise_base`), so it can go in a cache key.
#[derive(Debug, Clone, PartialEq)]
pub struct Voice {
    pub base_url: String,
    pub model: String,
    pub voice: String,
    pub speed: f64,
}

/// The `voice` row as stored, read with defaults (see [`VoiceSettings`]).
pub fn settings() -> VoiceSettings {
    settings_from(setting_blocking(SETTINGS_KEY).as_deref())
}

pub fn settings_from(row: Option<&str>) -> VoiceSettings {
    let row = row.and_then(|r| serde_json::from_str::<Value>(r).ok());
    let text = |key: &str| {
        row.as_ref()
            .and_then(|r| r.get(key))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let speed = row
        .as_ref()
        .and_then(|r| r.get("speed"))
        .and_then(Value::as_f64)
        .filter(|s| valid_speed(*s));
    VoiceSettings {
        base_url: text("baseUrl"),
        model: text("model"),
        voice: text("voice"),
        speed: speed.unwrap_or(DEFAULT_SPEED),
    }
}

/// Write the row, keeping keys it does not own; a `None` base URL removes
/// it. For Settings -> Voice.
pub async fn save(pool: &SqlitePool, settings: &VoiceSettings) -> Result<(), String> {
    if !valid_speed(settings.speed) {
        return Err(format!(
            "speed {} must be a positive number",
            settings.speed
        ));
    }
    let base = match settings.base_url.as_deref().map(str::trim) {
        Some(url) if !url.is_empty() => Some(crate::studio::tts::normalise_base(url)?),
        _ => None,
    };
    edit_setting(pool, SETTINGS_KEY, |object| {
        match base {
            Some(url) => object.insert("baseUrl".into(), url.into()),
            None => object.remove("baseUrl"),
        };
        for (key, value) in [("model", &settings.model), ("voice", &settings.voice)] {
            match value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                Some(v) => object.insert(key.into(), v.into()),
                None => object.remove(key),
            };
        }
        object.insert("speed".into(), settings.speed.into());
    })
    .await
}

/// Each field from the first of `flags`, `video`, `settings` that sets it.
/// Fails with [`NO_SERVER`], [`NO_MODEL`] or [`NO_VOICE`] when nothing sets
/// that field, and on a bad URL or a non-positive speed from the flags.
pub fn resolve(
    flags: &VoiceOverrides,
    video: &VoiceOverrides,
    settings: &VoiceSettings,
) -> Result<Voice, String> {
    let pick = |a: &Option<String>, b: &Option<String>| {
        [a, b]
            .into_iter()
            .flatten()
            .map(|s| s.trim())
            .find(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let base = pick(&flags.base_url, &video.base_url)
        .or_else(|| settings.base_url.clone())
        .ok_or(NO_SERVER)?;
    let speed = flags.speed.or(video.speed).unwrap_or(settings.speed);
    if !valid_speed(speed) {
        return Err(format!("speed {speed} must be a positive number"));
    }
    Ok(Voice {
        base_url: crate::studio::tts::normalise_base(&base)?,
        model: pick(&flags.model, &video.model)
            .or_else(|| settings.model.clone())
            .ok_or(NO_MODEL)?,
        voice: pick(&flags.voice, &video.voice)
            .or_else(|| settings.voice.clone())
            .ok_or(NO_VOICE)?,
        speed,
    })
}

/// The `voice` row's server as `(host, port)`: the one network destination
/// an agent thread is granted, so `oculus studio narrate` reaches it from
/// inside the sandbox (docs/harness.md). `None` with no row, no URL or no
/// host, so no grant; never a wildcard.
pub fn server_host(settings: &VoiceSettings) -> Option<(String, u16)> {
    let url = url::Url::parse(settings.base_url.as_deref()?).ok()?;
    let host = url
        .host_str()
        .filter(|h| !h.is_empty() && !h.contains('*'))?;
    Some((host.to_owned(), url.port_or_known_default()?))
}

fn valid_speed(speed: f64) -> bool {
    speed.is_finite() && speed > 0.0
}
