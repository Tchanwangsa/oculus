//! misaki's US dictionary: `us_gold.json` then `us_silver.json`, read from
//! the voice pack's folder (never compiled in) and grown as misaki's
//! `grow_dictionary` does, so `hello` also answers `Hello` and `Gradient`
//! also answers `gradient`.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use super::chars::{capitalize, char_len, lower};
use super::guess::{GOLD_FILE, SILVER_FILE};

/// A dictionary value: one reading, or a heteronym's readings keyed by tag
/// (`DEFAULT`, `VERB`, `NOUN`, `ADJ`, `VBD`, `None`, ...). A tag's reading
/// can be `null`, which misaki answers by spelling the word out.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Entry {
    Word(String),
    Tags(HashMap<String, Option<String>>),
}

pub struct Dictionary {
    gold: HashMap<String, Entry>,
    silver: HashMap<String, Entry>,
}

impl Dictionary {
    /// `us_gold.json` and `us_silver.json` from a pack folder.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let read = |name: &str| {
            let path = dir.join(name);
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
        };
        Self::from_json(&read(GOLD_FILE)?, &read(SILVER_FILE)?)
    }

    pub fn from_json(gold: &str, silver: &str) -> Result<Self, String> {
        Ok(Self {
            gold: grow(parse(GOLD_FILE, gold)?),
            silver: grow(parse(SILVER_FILE, silver)?),
        })
    }

    pub fn gold(&self, word: &str) -> Option<&Entry> {
        self.gold.get(word)
    }

    pub fn silver(&self, word: &str) -> Option<&Entry> {
        self.silver.get(word)
    }

    pub fn in_gold(&self, word: &str) -> bool {
        self.gold.contains_key(word)
    }

    pub fn in_silver(&self, word: &str) -> bool {
        self.silver.contains_key(word)
    }

    /// A gold entry with a single reading (`to`, `am`, the letters).
    pub fn gold_word(&self, word: &str) -> Option<&str> {
        match self.gold.get(word) {
            Some(Entry::Word(ps)) => Some(ps),
            _ => None,
        }
    }
}

/// A heteronym entry without `DEFAULT` would leave untagged words without a
/// reading, so the file is refused.
fn parse(name: &str, text: &str) -> Result<HashMap<String, Entry>, String> {
    let map: HashMap<String, Entry> =
        serde_json::from_str(text).map_err(|e| format!("{name}: {e}"))?;
    for (word, entry) in &map {
        if let Entry::Tags(tags) = entry {
            if !tags.contains_key("DEFAULT") {
                return Err(format!("{name}: {word:?} has readings but no DEFAULT"));
            }
        }
    }
    Ok(map)
}

/// misaki's `grow_dictionary`: a lower-case key of two or more characters
/// also answers its capitalised form, a capitalised key its lower-case
/// form; a key the file has itself always wins.
pub fn grow(d: HashMap<String, Entry>) -> HashMap<String, Entry> {
    let mut grown = HashMap::with_capacity(d.len() * 2);
    for (k, v) in &d {
        if char_len(k) < 2 {
            continue;
        }
        let low = lower(k);
        if *k == low {
            let cap = capitalize(k);
            if *k != cap {
                grown.insert(cap, v.clone());
            }
        } else if *k == capitalize(&low) {
            grown.insert(low, v.clone());
        }
    }
    grown.extend(d);
    grown
}
