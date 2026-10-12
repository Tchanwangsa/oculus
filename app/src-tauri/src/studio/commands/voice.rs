//! The `voice` settings row as Settings -> Voice reads and writes it, and
//! the server's model ids for its Model select.

use serde::{Deserialize, Serialize};

use crate::db::store::open_pool;
use crate::studio::tts::{list_models, normalise_base};
use crate::studio::voice::{self, Engine, VoiceSettings};

/// The row as the page sees it; an unset field is null. `engine` is
/// `"server"`, `"kokoro"` or null, and may be left out (null).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceRow {
    #[serde(default)]
    pub engine: Option<Engine>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub voice: Option<String>,
    pub speed: f64,
}

impl From<VoiceSettings> for VoiceRow {
    fn from(s: VoiceSettings) -> Self {
        Self {
            engine: s.engine,
            base_url: s.base_url,
            model: s.model,
            voice: s.voice,
            speed: s.speed,
        }
    }
}

/// `row` as it will be stored: fields trimmed, blanks unset, the URL
/// normalised (`tts::normalise_base`). A bad URL or a speed that is not a
/// positive number is an error worded for the page.
pub(super) fn validated(row: VoiceRow) -> Result<VoiceSettings, String> {
    let text = |value: Option<String>| value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
    if !(row.speed.is_finite() && row.speed > 0.0) {
        return Err(format!("speed {} must be a positive number", row.speed));
    }
    Ok(VoiceSettings {
        engine: row.engine,
        base_url: text(row.base_url)
            .map(|url| normalise_base(&url))
            .transpose()?,
        model: text(row.model),
        voice: text(row.voice),
        speed: row.speed,
    })
}

/// The `voice` row, with unset fields null and speed defaulted to 0.9
/// (`voice::DEFAULT_SPEED`).
#[tauri::command]
pub async fn studio_voice_settings() -> Result<VoiceRow, String> {
    crate::runtime::blocking::run(|| Ok(voice::settings().into())).await
}

/// Validate and write the whole row; returns what was stored.
#[tauri::command]
pub async fn studio_set_voice_settings(settings: VoiceRow) -> Result<VoiceRow, String> {
    let settings = validated(settings)?;
    let pool = open_pool().await?;
    let saved = voice::save(&pool, &settings).await;
    pool.close().await;
    saved.map(|()| settings.into())
}

/// The model ids the server at `base_url` lists (`GET /v1/models`). Free;
/// an unreachable or non-listing server is an error the page shows inline.
#[tauri::command]
pub async fn studio_voice_models(base_url: String) -> Result<Vec<String>, String> {
    crate::runtime::blocking::run(move || list_models(&base_url)).await
}
