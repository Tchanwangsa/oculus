//! Checking a reading and writing one entry into a `lexicon.json`. The
//! review is the only writer of lexicon files: an entry goes in when the
//! student accepts a suggestion or keeps the guess.

use std::path::Path;

use serde_json::{Map, Value};

use crate::runtime::atomic_write;
use crate::studio::voice::g2p::{Dictionary, Lexicon};

/// The phonemes `reading` gives `word`, or the lexicon's error for it
/// (a phoneme Kokoro lacks, a respelling part the dictionary lacks),
/// naming `source` as its file.
pub fn check(source: &str, word: &str, reading: &str, dict: &Dictionary) -> Result<String, String> {
    let entry = Value::Object(Map::from_iter([(
        word.to_owned(),
        Value::String(reading.to_owned()),
    )]))
    .to_string();
    let lexicon = Lexicon::from_json(&[(source, &entry)], dict).map_err(|e| e.to_string())?;
    lexicon
        .phonemes(word)
        .ok_or_else(|| format!("{source}: {word:?} cannot be a lexicon key"))
}

/// Set `word` to `reading` in the lexicon at `path`, checked first. Every
/// other key, comments (`_` keys) included, keeps its value and place; a
/// key already there keeps its place. A missing file is created; a file
/// that is not a JSON object is left alone and refused.
pub fn write_entry(
    path: &Path,
    word: &str,
    reading: &str,
    dict: &Dictionary,
) -> Result<(), String> {
    let file = path.display().to_string();
    check(&file, word, reading, dict)?;
    let mut entries = match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => map,
            Ok(_) => return Err(format!("{file}: not a JSON object; fix it by hand first")),
            Err(e) => {
                return Err(format!(
                    "{file}: not a JSON object: {e}; fix it by hand first"
                ))
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Map::new(),
        Err(e) => return Err(format!("{file}: {e}")),
    };
    entries.insert(word.to_owned(), Value::String(reading.to_owned()));
    atomic_write::json(path, &Value::Object(entries))
}
