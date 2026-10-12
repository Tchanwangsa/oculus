//! [`KokoroSpeaker`]: narration's `Speaker` for the Kokoro engine.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use super::chunk::chunks;
use super::model::{self, MODEL_FILE, MODEL_ID, PACK, VOICES};
use super::reader::{missing, no_pad, LexiconFiles, Reader};
use super::synth::{render, wav, Kokoro};
use crate::runtime::onnx;
use crate::studio::audio::decode_wav;
use crate::studio::narrate::Speaker;
use crate::studio::tts::Speech;
use crate::studio::voice::g2p::{guess, Phonemized};
use crate::studio::voice::KokoroVoice;

/// Where the two packs are: the voice (`voice-kokoro`) and the G2P
/// (`g2p-en-us`).
#[derive(Debug, Clone)]
pub struct Packs {
    pub voice: PathBuf,
    pub g2p: PathBuf,
}

impl Packs {
    /// The app's download folder for both.
    pub fn app() -> Self {
        Self {
            voice: model::dir(),
            g2p: guess::dir(),
        }
    }
}

/// Kokoro run in the app for one narrate run. The reader loads on the
/// first key (every scene's key holds its phonemes), the model on the first
/// clip; both are dropped with the speaker, never held idle between runs.
pub struct KokoroSpeaker {
    voice: KokoroVoice,
    packs: Packs,
    lexicon: LexiconFiles,
    reader: RefCell<Option<Reader>>,
    kokoro: RefCell<Option<Kokoro>>,
    read: RefCell<HashMap<String, Phonemized>>,
}

impl KokoroSpeaker {
    pub fn new(voice: KokoroVoice, packs: Packs, lexicon: LexiconFiles) -> Self {
        Self {
            voice,
            packs,
            lexicon,
            reader: RefCell::new(None),
            kokoro: RefCell::new(None),
            read: RefCell::new(HashMap::new()),
        }
    }

    /// How `text` was read: each word's phonemes and who read it (the
    /// guessed ones are worth a listen). Known once the text was keyed.
    pub fn phonemized(&self, text: &str) -> Option<Phonemized> {
        self.read.borrow().get(text).cloned()
    }

    /// `text` read to phonemes, once per text.
    fn read(&self, text: &str) -> Result<Phonemized, String> {
        if let Some(done) = self.read.borrow().get(text) {
            return Ok(done.clone());
        }
        no_pad(text)?;
        let mut slot = self.reader.borrow_mut();
        let reader = match &mut *slot {
            Some(r) => r,
            empty @ None => empty.insert(Reader::load(&self.packs.g2p, &self.lexicon)?),
        };
        let phonemized = reader.read(text)?;
        self.read
            .borrow_mut()
            .insert(text.to_owned(), phonemized.clone());
        Ok(phonemized)
    }

    fn load_kokoro(&self) -> Result<Kokoro, String> {
        let voice = model::voice_file(&self.packs.voice, &self.voice.voice)
            .ok_or_else(|| self.no_voice())?;
        Kokoro::load(&self.packs.voice.join(MODEL_FILE), &voice, self.voice.speed)
    }

    fn no_voice(&self) -> String {
        format!(
            "Kokoro voice `{}` is not downloaded; the voice pack has {}",
            self.voice.voice,
            VOICES.join(" and ")
        )
    }
}

impl Speaker for KokoroSpeaker {
    fn ready(&self) -> Result<(), String> {
        if !onnx::is_present(&self.packs.voice, &PACK) {
            return Err(missing("voice pack (voice-kokoro)", PACK.bytes()));
        }
        if !onnx::is_present(&self.packs.g2p, &guess::PACK) {
            return Err(missing("G2P pack (g2p-en-us)", guess::PACK.bytes()));
        }
        model::voice_file(&self.packs.voice, &self.voice.voice)
            .map(drop)
            .ok_or_else(|| self.no_voice())
    }

    /// The engine, model, voice, speed and the text's phonemes, so a
    /// lexicon edit re-narrates only the scenes whose phonemes it changes.
    fn key(&self, text: &str) -> Result<Vec<String>, String> {
        let phonemized = self.read(text)?;
        Ok(vec![
            "kokoro".to_owned(),
            MODEL_ID.to_owned(),
            self.voice.voice.clone(),
            format!("{:?}", self.voice.speed),
            phonemized.phonemes,
        ])
    }

    fn speak(&self, text: &str) -> Result<Speech, String> {
        let chunks = chunks(&self.read(text)?)?;
        let mut slot = self.kokoro.borrow_mut();
        let kokoro = match &mut *slot {
            Some(k) => k,
            empty @ None => empty.insert(self.load_kokoro()?),
        };
        let samples = render(&chunks, kokoro)?;
        let wav = wav(&samples)?;
        let decoded = decode_wav(&wav).map_err(|e| e.to_string())?;
        Ok(Speech { wav, decoded })
    }
}
