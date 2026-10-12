//! The pronunciation review in Settings -> Voice (`studio::voice::review`):
//! the agent's suggestions, the student's three verdicts, and a sample of
//! the word's sentence read either way, synthesised by Kokoro on this computer.

use serde::Deserialize;

use super::kokoro::sample;
use crate::studio::tts::kokoro::{LexiconFiles, Packs};
use crate::studio::voice::g2p::{guess, Dictionary};
use crate::studio::voice::review::{Decided, Decision, Item, Record, Store};
use crate::studio::voice::{self, VoiceSettings};

/// Which reading a sample pins the word to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Which {
    /// The guess Kokoro made, as `/phonemes/`.
    Original,
    /// The agent's suggested reading.
    Suggestion,
}

/// `record`'s sentence as WAV bytes in the settings' voice and speed, with
/// the word pinned to `which` over its subject's lexicon.
pub(super) fn sample_record(
    record: &Record,
    which: Which,
    settings: &VoiceSettings,
    packs: Packs,
    mut lexicon: LexiconFiles,
) -> Result<Vec<u8>, String> {
    let value = match which {
        Which::Original => format!("/{}/", record.original),
        Which::Suggestion => record.reading.clone(),
    };
    lexicon.pin = Some((record.word.clone(), value));
    let voice = settings.voice.as_deref().unwrap_or_default();
    sample(
        voice,
        settings.speed,
        Some(&record.sentence),
        packs,
        lexicon,
    )
}

/// Every suggestion, newest first: pending and wrong ones.
#[tauri::command]
pub async fn studio_pronunciations_list() -> Result<Vec<Item>, String> {
    crate::runtime::blocking::run(|| Store::app().list(|_| true)).await
}

/// `correct` writes the suggestion into the lexicon, `fine` the original
/// guess as `/phonemes/`; either deletes the item. It applies at the
/// video's next narrate.
#[tauri::command]
pub async fn studio_pronunciation_decide(
    id: String,
    decision: Decision,
) -> Result<Decided, String> {
    crate::runtime::blocking::run(move || {
        let dict = Dictionary::load(&guess::dir())?;
        Store::app().decide(&id, decision, &dict)
    })
    .await
}

/// Marks the suggestion wrong, with the student's note on how the word is
/// said.
#[tauri::command]
pub async fn studio_pronunciation_wrong(id: String, note: Option<String>) -> Result<Item, String> {
    crate::runtime::blocking::run(move || Store::app().mark_wrong(&id, note)).await
}

/// The item's sentence with the word read as `which`, in Settings -> Voice's
/// Kokoro voice and speed, as WAV bytes.
#[tauri::command]
pub async fn studio_pronunciation_sample(
    id: String,
    which: Which,
) -> Result<tauri::ipc::Response, String> {
    crate::runtime::blocking::run(move || {
        let item = Store::app().get(&id)?;
        let subject = item.record.subject.as_deref();
        sample_record(
            &item.record,
            which,
            &voice::settings(),
            Packs::app(),
            LexiconFiles::app(subject),
        )
        .map(tauri::ipc::Response::new)
    })
    .await
}
