//! Which engine, voice and speed narrate a video, and for the server engine
//! which TTS server and model: the `voice` settings row (Settings -> Voice),
//! under a video's `voice` block in video.json, under the CLI's flags. The
//! server engine is a keyless OpenAI-compatible server; the Kokoro engine
//! runs in the app (`studio::tts::kokoro`).

pub mod g2p;
pub mod lint;
pub mod review;
#[cfg(test)]
mod tests;

use serde_json::Value;
use sqlx::SqlitePool;

use crate::db::store::{edit_setting, setting_blocking};
pub use studio_core::VoiceEngine as Engine;

/// The settings row's key.
pub const SETTINGS_KEY: &str = "voice";

/// A little under the voice's own pace: a lecture is easier to follow
/// slow, and a player speeds it up.
pub const DEFAULT_SPEED: f64 = 0.9;

/// What `narrate` says when no engine is chosen anywhere. There is no
/// default: Kokoro needs a download, a server needs a URL.
pub const NO_ENGINE: &str =
    "no narration engine: choose Kokoro or a TTS server in Settings -> Voice, or pass --engine";
/// What `narrate` says when the server engine has no server set anywhere.
pub const NO_SERVER: &str =
    "no TTS server: set a TTS server in Settings -> Voice, or pass --server";
/// Model and voice ids differ per server (Kokoro-FastAPI `kokoro`, mlx-audio
/// `mlx-community/Kokoro-82M-bf16`, OpenAI `tts-1`), so neither has a default.
pub const NO_MODEL: &str =
    "no TTS model: set a model in Settings -> Voice or video.json's voice block, or pass --model";
pub const NO_VOICE: &str =
    "no TTS voice: set a voice in Settings -> Voice or video.json's voice block, or pass --voice";

/// The `voice` row, `{ "engine", "baseUrl", "model", "voice", "speed" }`.
/// A missing or ill-typed field reads as unset (speed: `DEFAULT_SPEED`),
/// except that a row with a `baseUrl` and no `engine` key reads as the
/// server engine, the only one a URL is for.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceSettings {
    pub engine: Option<Engine>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub voice: Option<String>,
    pub speed: f64,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            engine: None,
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
    pub engine: Option<Engine>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub voice: Option<String>,
    pub speed: Option<f64>,
}

impl From<&studio_core::Voice> for VoiceOverrides {
    fn from(v: &studio_core::Voice) -> Self {
        Self {
            engine: v.engine,
            base_url: None,
            model: v.model.clone(),
            voice: v.voice.clone(),
            speed: v.speed,
        }
    }
}

/// The voice a clip is made with, every field its engine reads settled.
#[derive(Debug, Clone, PartialEq)]
pub enum Voice {
    Server(ServerVoice),
    Kokoro(KokoroVoice),
}

impl Voice {
    pub fn engine(&self) -> Engine {
        match self {
            Voice::Server(_) => Engine::Server,
            Voice::Kokoro(_) => Engine::Kokoro,
        }
    }

    pub fn voice(&self) -> &str {
        match self {
            Voice::Server(v) => &v.voice,
            Voice::Kokoro(v) => &v.voice,
        }
    }

    pub fn speed(&self) -> f64 {
        match self {
            Voice::Server(v) => v.speed,
            Voice::Kokoro(v) => v.speed,
        }
    }
}

/// A TTS server's voice; `base_url` is normalised (`tts::normalise_base`),
/// so it can go in a cache key.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerVoice {
    pub base_url: String,
    pub model: String,
    pub voice: String,
    pub speed: f64,
}

/// A Kokoro voice: `voice` names its style file in the voice pack
/// (`af_heart` reads `af_heart.bin`).
#[derive(Debug, Clone, PartialEq)]
pub struct KokoroVoice {
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
    let base_url = text("baseUrl");
    let engine = match row.as_ref().and_then(|r| r.get("engine")) {
        Some(engine) => engine.as_str().and_then(Engine::parse),
        None => base_url.is_some().then_some(Engine::Server),
    };
    VoiceSettings {
        engine,
        base_url,
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
        match settings.engine {
            Some(engine) => object.insert("engine".into(), engine.as_str().into()),
            None => object.remove("engine"),
        };
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
/// Fails with [`NO_ENGINE`], [`NO_VOICE`], and for the server engine
/// [`NO_SERVER`] or [`NO_MODEL`], when nothing sets that field, and on a bad
/// URL, a non-positive speed, or a Kokoro voice that is not a voice id. The
/// Kokoro engine ignores the server fields, which stay set for when the
/// server engine is chosen again.
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
    let engine = flags
        .engine
        .or(video.engine)
        .or(settings.engine)
        .ok_or(NO_ENGINE)?;
    let speed = flags.speed.or(video.speed).unwrap_or(settings.speed);
    if !valid_speed(speed) {
        return Err(format!("speed {speed} must be a positive number"));
    }
    let voice = || {
        pick(&flags.voice, &video.voice)
            .or_else(|| settings.voice.clone())
            .ok_or(NO_VOICE)
    };
    match engine {
        Engine::Kokoro => {
            let voice = voice()?;
            if !valid_kokoro_voice(&voice) {
                return Err(format!(
                    "Kokoro voice `{voice}` is not a voice id; Kokoro's are like af_heart and am_michael"
                ));
            }
            Ok(Voice::Kokoro(KokoroVoice { voice, speed }))
        }
        Engine::Server => {
            let base = pick(&flags.base_url, &video.base_url)
                .or_else(|| settings.base_url.clone())
                .ok_or(NO_SERVER)?;
            Ok(Voice::Server(ServerVoice {
                base_url: crate::studio::tts::normalise_base(&base)?,
                model: pick(&flags.model, &video.model)
                    .or_else(|| settings.model.clone())
                    .ok_or(NO_MODEL)?,
                voice: voice()?,
                speed,
            }))
        }
    }
}

/// A Kokoro voice id names a file in the voice pack, so it is lower-case
/// letters, digits and `_` only.
fn valid_kokoro_voice(voice: &str) -> bool {
    !voice.is_empty()
        && voice
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
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
