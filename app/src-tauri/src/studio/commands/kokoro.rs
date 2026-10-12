//! Kokoro in Settings -> Studio -> Narration: its two packs' status, one download for both
//! that reports progress as `studio-voice-progress` events (`progress`), and
//! a sample clip synthesised on this Mac, never a billed call.

use std::path::Path;

use serde::Serialize;
use tauri::AppHandle;

use super::progress::download_with_events;
use crate::runtime::onnx::{self, Status};
use crate::studio::narrate::Speaker;
use crate::studio::tts::kokoro::{self, KokoroSpeaker, LexiconFiles, Packs};
use crate::studio::voice::{self, Engine, VoiceOverrides, VoiceSettings};

/// Display only, never persisted.
pub const PROGRESS_EVENT: &str = "studio-voice-progress";

/// What the sample button says when no text is given: spoken form, as a
/// script is.
pub const SAMPLE_TEXT: &str =
    "This is how your narration will sound. Each word is read aloud on this Mac, then timed to the picture.";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoicePacks {
    /// Both packs together: present only when both are.
    pub status: Status,
    /// Both packs' download size.
    pub bytes: u64,
    pub packs: Vec<PackRow>,
    /// The pack's voices, downloaded or not, then any other voice file in
    /// the folder (`kokoro::voices`).
    pub voices: Vec<VoiceFile>,
}

#[derive(Debug, Serialize)]
pub struct PackRow {
    /// `voice-kokoro` or `g2p-en-us`.
    pub id: &'static str,
    pub status: Status,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct VoiceFile {
    pub id: String,
    pub downloaded: bool,
}

/// The packs and voices in `dir`, from the disk alone.
pub(super) fn packs_status(dir: &Path) -> VoicePacks {
    let downloading = kokoro::downloading();
    let packs: Vec<PackRow> = kokoro::packs()
        .iter()
        .map(|p| PackRow {
            id: p.id,
            status: if onnx::is_present(dir, p) {
                Status::Present
            } else if downloading {
                Status::Downloading
            } else {
                Status::Missing
            },
            bytes: p.bytes(),
        })
        .collect();
    VoicePacks {
        status: kokoro::status(dir),
        bytes: packs.iter().map(|p| p.bytes).sum(),
        packs,
        voices: kokoro::voices(dir)
            .into_iter()
            .map(|(id, downloaded)| VoiceFile { id, downloaded })
            .collect(),
    }
}

/// `text` (or [`SAMPLE_TEXT`]) as a 24 kHz WAV in `voice` at `speed`, read
/// with `lexicon`. The speaker, its reader and its model session are
/// dropped on return.
pub(super) fn sample(
    voice: &str,
    speed: f64,
    text: Option<&str>,
    packs: Packs,
    lexicon: LexiconFiles,
) -> Result<Vec<u8>, String> {
    if voice.trim().is_empty() {
        return Err("choose a voice first".into());
    }
    let text = text
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or(SAMPLE_TEXT);
    let chosen = voice::resolve(
        &VoiceOverrides {
            engine: Some(Engine::Kokoro),
            voice: Some(voice.to_owned()),
            speed: Some(speed),
            ..VoiceOverrides::default()
        },
        &VoiceOverrides::default(),
        &VoiceSettings::default(),
    )?;
    let voice::Voice::Kokoro(chosen) = chosen else {
        unreachable!("the engine was given as Kokoro");
    };
    let speaker = KokoroSpeaker::new(chosen, packs, lexicon);
    speaker.ready()?;
    Ok(speaker.speak(text)?.wav)
}

/// Both packs' status and the voices on disk. Local only.
#[tauri::command]
pub async fn studio_voice_packs_status() -> Result<VoicePacks, String> {
    crate::runtime::blocking::run(|| Ok(packs_status(&kokoro::dir()))).await
}

/// Download both packs, resolving when they are on disk. A cancel rejects
/// with `cancelled`.
#[tauri::command]
pub async fn studio_voice_packs_download(app: AppHandle) -> Result<(), String> {
    crate::runtime::blocking::run(move || {
        let dir = kokoro::dir();
        let total = kokoro::packs().iter().map(|p| p.bytes()).sum();
        download_with_events(
            &app,
            PROGRESS_EVENT,
            "Kokoro's packs",
            &dir,
            total,
            |progress| kokoro::download(&dir, progress, || false),
        )
    })
    .await
}

/// Ask the in-flight download to stop; false when none runs.
#[tauri::command]
pub fn studio_voice_packs_cancel() -> bool {
    kokoro::cancel()
}

/// Delete both packs and any partial download, stopping a download if one
/// runs; another voice's file stays. Returns the bytes freed.
#[tauri::command]
pub async fn studio_voice_packs_delete() -> Result<u64, String> {
    crate::runtime::blocking::run(|| kokoro::delete(&kokoro::dir())).await
}

/// A sample of `voice` at `speed` reading `text` (a fixed sentence when
/// absent), synthesised by Kokoro on this Mac with the global library's
/// lexicon, as WAV bytes.
#[tauri::command]
pub async fn studio_voice_sample(
    voice: String,
    speed: f64,
    text: Option<String>,
) -> Result<tauri::ipc::Response, String> {
    crate::runtime::blocking::run(move || {
        sample(
            &voice,
            speed,
            text.as_deref(),
            Packs::app(),
            LexiconFiles::app(None),
        )
        .map(tauri::ipc::Response::new)
    })
    .await
}
