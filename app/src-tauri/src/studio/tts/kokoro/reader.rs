//! [`Reader`]: text to Kokoro phonemes with the `g2p-en-us` pack and a
//! video's lexicon, for narration, lint and `oculus studio g2p`.

use std::path::{Path, PathBuf};

use super::vocab::PAD;
use crate::runtime::onnx;
use crate::studio::voice::g2p::guess::{self, Guesser};
use crate::studio::voice::g2p::{self, Dictionary, Lexicon, Phonemized};

/// The lexicon layers a video reads (`g2p::Lexicon`); a missing file is an
/// empty layer.
#[derive(Debug, Clone, Default)]
pub struct LexiconFiles {
    pub library: Option<PathBuf>,
    pub subject: Option<PathBuf>,
    /// One word and its lexicon value over both files: the review's samples
    /// of a word read as its guess or as a suggestion.
    pub pin: Option<(String, String)>,
}

impl LexiconFiles {
    /// The global library's `lexicon.json`, and `subject`'s when the video
    /// has one.
    pub fn app(subject: Option<&str>) -> Self {
        use crate::studio::libraries::{global_dir, subjects_dir};
        Self {
            library: Some(global_dir().join("lexicon.json")),
            subject: subject.map(|code| subjects_dir().join(code).join("lexicon.json")),
            pin: None,
        }
    }
}

/// The dictionary, the lexicon and the guesser, loaded together.
pub struct Reader {
    dict: Dictionary,
    lexicon: Lexicon,
    guesser: Guesser,
}

impl Reader {
    /// Fails when the pack is not in `g2p`, and on a lexicon entry that is
    /// not valid (naming its file and key).
    pub fn load(g2p: &Path, lexicon: &LexiconFiles) -> Result<Self, String> {
        if !onnx::is_present(g2p, &guess::PACK) {
            return Err(missing("G2P pack (g2p-en-us)", guess::PACK.bytes()));
        }
        let dict = Dictionary::load(g2p)?;
        let pin = lexicon.pin.as_ref().map(|(word, value)| {
            serde_json::Value::Object(serde_json::Map::from_iter([(
                word.clone(),
                serde_json::Value::String(value.clone()),
            )]))
            .to_string()
        });
        let lexicon = Lexicon::load_over(
            lexicon.library.as_deref(),
            lexicon.subject.as_deref(),
            pin.as_deref().map(|text| ("the pinned reading", text)),
            &dict,
        )
        .map_err(|e| e.to_string())?;
        let guesser = Guesser::load(g2p)?;
        Ok(Self {
            dict,
            lexicon,
            guesser,
        })
    }

    /// The dictionary, for checking a lexicon value.
    pub fn dict(&self) -> &Dictionary {
        &self.dict
    }

    /// `text`'s phonemes and how each word was read. A `$` is refused: it
    /// is Kokoro's pad.
    pub fn read(&mut self, text: &str) -> Result<Phonemized, String> {
        no_pad(text)?;
        g2p::phonemize(text, &self.dict, &self.lexicon, &mut self.guesser)
    }
}

/// Refuses a `$` in `text`, which no reader is needed to see.
pub(super) fn no_pad(text: &str) -> Result<(), String> {
    if text.contains(PAD) {
        return Err("`$` is Kokoro's pad and cannot be said; spell it out".to_owned());
    }
    Ok(())
}

/// Kokoro's `what` (`bytes` big) is not on disk.
pub(super) fn missing(what: &str, bytes: u64) -> String {
    format!(
        "Kokoro's {what}, {} MB, is not downloaded: run `oculus studio voices download`, or download it in Settings -> Voice",
        (bytes + 500_000) / 1_000_000
    )
}
