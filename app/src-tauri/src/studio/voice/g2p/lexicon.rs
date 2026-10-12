//! The user's pronunciation lexicon, read before the dictionary: a JSON
//! object from a word to its reading. Two layers, the subject's over the
//! library's:
//!
//! - `<data dir>/studio/library/lexicon.json` (`libraries::global_dir()`)
//! - `<data dir>/studio/subjects/<CODE>/lexicon.json` (`libraries::subjects_dir()`)
//!
//! A value in slashes is raw phonemes in Kokoro's alphabet (`"/kˈOʃi/"`),
//! less its pad `$`; any other value is a respelling in dictionary words (`"SOFT-max"`), each
//! `-` or space part read through the dictionary, an all-caps part longer
//! than one letter taking primary stress and the rest secondary. Keys
//! starting `_` are comments. Every entry is checked on load, so a bad
//! phoneme or an unknown respelling part fails the load naming the file and
//! key. A word finds its entry by exact key, then its lower case; a word
//! ending `'s` takes its stem's entry plus `-s`; then any key matching in
//! lower case. A curly apostrophe (`’`) reads as `'` in keys and in the
//! words looked up, since script.md's typography curls them.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::Deserialize;

use super::chars::{char_len, is_lower, is_upper, lower};
use super::dict::Dictionary;
use super::stem::plural;
use super::stress::apply_stress;
use super::word::lookup;
use crate::studio::tts::kokoro::vocab;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexiconError {
    Read {
        file: String,
        error: String,
    },
    Json {
        file: String,
        error: String,
    },
    NotText {
        file: String,
        key: String,
    },
    BadPhoneme {
        file: String,
        key: String,
        chars: Vec<char>,
    },
    UnknownPart {
        file: String,
        key: String,
        part: String,
    },
}

impl LexiconError {
    /// `bad-phoneme`, `unknown-part`, or `malformed` for a file that is not a
    /// JSON object of strings.
    pub fn kind(&self) -> &'static str {
        match self {
            LexiconError::BadPhoneme { .. } => "bad-phoneme",
            LexiconError::UnknownPart { .. } => "unknown-part",
            LexiconError::Read { .. }
            | LexiconError::Json { .. }
            | LexiconError::NotText { .. } => "malformed",
        }
    }
}

impl fmt::Display for LexiconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexiconError::Read { file, error } => write!(f, "{file}: {error}"),
            LexiconError::Json { file, error } => write!(f, "{file}: not a JSON object: {error}"),
            LexiconError::NotText { file, key } => {
                write!(f, "{file}: lexicon {key:?}: the value is not a string")
            }
            LexiconError::BadPhoneme { file, key, chars } => write!(
                f,
                "{file}: lexicon {key:?}: phoneme characters not in Kokoro's vocabulary: {chars:?}"
            ),
            LexiconError::UnknownPart { file, key, part } => write!(
                f,
                "{file}: lexicon {key:?}: respelling part {part:?} is not in the dictionary"
            ),
        }
    }
}

/// A JSON object's entries in file order (serde_json's map sorts them).
struct Ordered(Vec<(String, serde_json::Value)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Entries;
        impl<'de> Visitor<'de> for Entries {
            type Value = Ordered;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(Ordered(entries))
            }
        }
        d.deserialize_map(Entries)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Lexicon {
    /// Key and phonemes, in the order a Python dict updated layer by layer
    /// holds them: a key keeps its first place, takes its last value.
    entries: Vec<(String, String)>,
    exact: HashMap<String, usize>,
    /// Lower-cased key to entry; of keys equal in lower case, the last.
    folded: HashMap<String, usize>,
}

impl Lexicon {
    /// The library layer, then the subject's over it. A missing file is an
    /// empty layer.
    pub fn load(
        library: Option<&Path>,
        subject: Option<&Path>,
        dict: &Dictionary,
    ) -> Result<Self, LexiconError> {
        let mut layers = Vec::new();
        for path in [library, subject].into_iter().flatten() {
            let file = path.display().to_string();
            match std::fs::read_to_string(path) {
                Ok(text) => layers.push((file, text)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(LexiconError::Read {
                        file,
                        error: e.to_string(),
                    })
                }
            }
        }
        let layers: Vec<(&str, &str)> = layers
            .iter()
            .map(|(f, t)| (f.as_str(), t.as_str()))
            .collect();
        Self::from_json(&layers, dict)
    }

    /// `(file name, JSON text)` layers, each over the ones before it.
    pub fn from_json(layers: &[(&str, &str)], dict: &Dictionary) -> Result<Self, LexiconError> {
        let mut lexicon = Self::default();
        for &(file, text) in layers {
            let Ordered(entries) = serde_json::from_str(text).map_err(|e| LexiconError::Json {
                file: file.to_string(),
                error: e.to_string(),
            })?;
            for (key, value) in entries {
                let key = straighten(&key);
                if key.starts_with('_') {
                    continue;
                }
                let Some(value) = value.as_str() else {
                    return Err(LexiconError::NotText {
                        file: file.to_string(),
                        key,
                    });
                };
                let ps = respell(dict, value).map_err(|e| e.into_error(file, &key))?;
                lexicon.insert(key, ps);
            }
        }
        for (i, (key, _)) in lexicon.entries.iter().enumerate() {
            lexicon.folded.insert(lower(key), i);
        }
        Ok(lexicon)
    }

    fn insert(&mut self, key: String, ps: String) {
        match self.exact.get(&key) {
            Some(&i) => self.entries[i].1 = ps,
            None => {
                self.exact.insert(key.clone(), self.entries.len());
                self.entries.push((key, ps));
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The phonemes for `word` as written, or `None` to read it on.
    pub fn phonemes(&self, word: &str) -> Option<String> {
        let word = &straighten(word);
        let found = self
            .exact
            .get(word)
            .or_else(|| self.exact.get(&lower(word)));
        if found.is_none() {
            if let Some(stem) = word.strip_suffix("'s") {
                return plural(&self.phonemes(stem)?);
            }
        }
        let i = found.or_else(|| self.folded.get(&lower(word)))?;
        Some(self.entries[*i].1.clone())
    }
}

/// Why a value has no phonemes, before the file and key are known.
enum Bad {
    Phoneme(Vec<char>),
    Part(String),
}

impl Bad {
    fn into_error(self, file: &str, key: &str) -> LexiconError {
        let (file, key) = (file.to_string(), key.to_string());
        match self {
            Bad::Phoneme(chars) => LexiconError::BadPhoneme { file, key, chars },
            Bad::Part(part) => LexiconError::UnknownPart { file, key, part },
        }
    }
}

/// `’` as `'`; the dictionary path keeps it, as the oracle does.
fn straighten(word: &str) -> String {
    word.replace('’', "'")
}

/// `str.isupper()`: a cased letter, and no lower-case one.
fn all_caps(s: &str) -> bool {
    s.chars().any(is_upper) && !s.chars().any(is_lower)
}

/// A lexicon value as phonemes.
fn respell(dict: &Dictionary, value: &str) -> Result<String, Bad> {
    if value.starts_with('/') {
        let ps = value.trim_matches('/');
        let mut bad: Vec<char> = ps
            .chars()
            .filter(|&c| !vocab::contains(c) || c == vocab::PAD)
            .collect();
        bad.sort_unstable();
        bad.dedup();
        return if bad.is_empty() {
            Ok(ps.to_string())
        } else {
            Err(Bad::Phoneme(bad))
        };
    }
    let mut ps = String::new();
    for part in value.split(['-', ' ']) {
        let low = lower(part);
        let known = dict.in_gold(&low) || dict.in_silver(&low);
        let Some(p) = known.then(|| lookup(dict, &low, "", None, None)).flatten() else {
            return Err(Bad::Part(part.to_string()));
        };
        let stress = if all_caps(part) && char_len(part) > 1 {
            2.0
        } else {
            -1.0
        };
        ps.push_str(&apply_stress(&p, Some(stress)));
    }
    Ok(ps)
}
