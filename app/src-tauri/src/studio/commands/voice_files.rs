//! Kokoro's voices in Settings -> Studio -> Narration: each of the 20 listed
//! with its status, downloaded, cancelled and deleted one at a time. A
//! download reports `studio-voice-file-progress` events naming the voice
//! (`progress`). Everything but a download is local.

use std::path::Path;

use serde::Serialize;
use tauri::AppHandle;

use super::progress::download_with_events;
use crate::runtime::onnx::Status;
use crate::studio::tts::kokoro::{
    self,
    voices::{self, Voice, VOICES},
};

/// Display only, never persisted.
pub const PROGRESS_EVENT: &str = "studio-voice-file-progress";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceRow {
    /// `af_heart`: what the `voice` setting and `--voice` take.
    pub id: &'static str,
    /// `Heart`.
    pub name: &'static str,
    /// `American`.
    pub accent: &'static str,
    /// `female` or `male`.
    pub gender: &'static str,
    /// Kokoro's published grade, `A` to `F`.
    pub grade: &'static str,
    pub bytes: u64,
    pub status: Status,
}

/// Every catalogued voice in `dir`, in catalogue order, from the disk alone.
pub(super) fn voices_status(dir: &Path) -> Vec<VoiceRow> {
    VOICES
        .iter()
        .map(|v| VoiceRow {
            id: v.id,
            name: v.name,
            accent: v.accent.label(),
            gender: v.gender.label(),
            grade: v.grade,
            bytes: v.bytes(),
            status: voices::status(dir, v),
        })
        .collect()
}

pub(super) fn voice(id: &str) -> Result<&'static Voice, String> {
    voices::find(id).ok_or_else(|| format!("`{id}` is not one of Kokoro's voices"))
}

/// All of Kokoro's voices and whether each is on disk. Local only.
#[tauri::command]
pub async fn studio_voices_status() -> Result<Vec<VoiceRow>, String> {
    crate::runtime::blocking::run(|| Ok(voices_status(&kokoro::dir()))).await
}

/// Download one voice, resolving when it is on disk. A cancel rejects with
/// `cancelled`.
#[tauri::command]
pub async fn studio_voice_download(app: AppHandle, id: String) -> Result<(), String> {
    crate::runtime::blocking::run(move || {
        let voice = voice(&id)?;
        let dir = kokoro::dir();
        download_with_events(
            &app,
            PROGRESS_EVENT,
            Some(voice.id),
            &format!("Kokoro voice {}", voice.id),
            &dir,
            voice.bytes(),
            |progress| voices::download(&dir, voice, progress, || false),
        )
    })
    .await
}

/// Ask `id`'s in-flight download to stop; false when none runs.
#[tauri::command]
pub fn studio_voice_cancel(id: String) -> Result<bool, String> {
    Ok(voices::cancel(voice(&id)?))
}

/// Delete one voice's file and any partial download, stopping its download
/// if one runs. Returns the bytes freed.
#[tauri::command]
pub async fn studio_voice_delete(id: String) -> Result<u64, String> {
    crate::runtime::blocking::run(move || voices::delete(&kokoro::dir(), voice(&id)?)).await
}
