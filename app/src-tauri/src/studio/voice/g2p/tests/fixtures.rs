//! The oracle's golden files (`golden/`, copied from
//! `data/evals/studio-g2p/golden/`), the dictionary trimmed to the entries
//! they touch (`golden/dict/`, made by `tools/trim_dict.py`), and a guesser
//! that answers from `golden/guesser.jsonl`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::super::{phonemize, Dictionary, Guess, Lexicon, Phonemized};

pub fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/studio/voice/g2p/tests/golden")
}

pub fn trimmed_dictionary() -> Dictionary {
    Dictionary::load(&golden_dir().join("dict")).unwrap()
}

#[derive(Debug, Deserialize)]
pub struct Row {
    pub id: String,
    pub text: String,
    pub phonemes: String,
    pub words: Vec<GoldWord>,
}

#[derive(Debug, Deserialize)]
pub struct GoldWord {
    pub word: String,
    pub phonemes: String,
    pub source: String,
    #[serde(default)]
    pub guessed: Vec<String>,
}

pub fn rows(file: &str) -> Vec<Row> {
    let path = golden_dir().join(file);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// The oracle's guesses, by the word or sub-token it was asked.
pub struct TableGuess(HashMap<String, String>);

impl TableGuess {
    pub fn load() -> Self {
        #[derive(Deserialize)]
        struct Guessed {
            word: String,
            phonemes: String,
        }
        let path = golden_dir().join("guesser.jsonl");
        let text = std::fs::read_to_string(path).unwrap();
        Self(
            text.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| {
                    let g: Guessed = serde_json::from_str(l).unwrap();
                    (g.word, g.phonemes)
                })
                .collect(),
        )
    }
}

impl Guess for TableGuess {
    fn guess(&mut self, part: &str) -> Result<String, String> {
        self.0
            .get(part)
            .cloned()
            .ok_or_else(|| format!("{part:?} is not in guesser.jsonl"))
    }
}

/// A golden file and the lexicon layers the oracle ran it with: the 7
/// bake-off inputs with the library's, the same with none (`nolex/`), and
/// the special cases with the subject's over the library's.
/// Rows across [`FILES`]; every one must match whole.
pub const ROWS: usize = 1030;

pub const FILES: [(&str, &[&str]); 15] = [
    ("script163.jsonl", LIBRARY),
    ("maths16.jsonl", LIBRARY),
    ("propernouns.jsonl", LIBRARY),
    ("lecture1.jsonl", LIBRARY),
    ("lecture2.jsonl", LIBRARY),
    ("lecture3.jsonl", LIBRARY),
    ("heteronyms30.jsonl", LIBRARY),
    ("nolex/script163.jsonl", &[]),
    ("nolex/maths16.jsonl", &[]),
    ("nolex/propernouns.jsonl", &[]),
    ("nolex/lecture1.jsonl", &[]),
    ("nolex/lecture2.jsonl", &[]),
    ("nolex/lecture3.jsonl", &[]),
    ("nolex/heteronyms30.jsonl", &[]),
    ("special.jsonl", BOTH),
];
const LIBRARY: &[&str] = &["lexicon.library.json"];
const BOTH: &[&str] = &["lexicon.library.json", "lexicon.subject.json"];

pub fn lexicon(layers: &[&str], dict: &Dictionary) -> Lexicon {
    let texts: Vec<(&str, String)> = layers
        .iter()
        .map(|&f| (f, std::fs::read_to_string(golden_dir().join(f)).unwrap()))
        .collect();
    let layers: Vec<(&str, &str)> = texts.iter().map(|(f, t)| (*f, t.as_str())).collect();
    Lexicon::from_json(&layers, dict).unwrap()
}

fn differs(got: &Phonemized, row: &Row) -> Option<String> {
    if got.phonemes != row.phonemes {
        return Some(format!(
            "phonemes\n  got  {:?}\n  want {:?}",
            got.phonemes, row.phonemes
        ));
    }
    if got.words.len() != row.words.len() {
        return Some(format!(
            "{} words, want {}",
            got.words.len(),
            row.words.len()
        ));
    }
    for (g, w) in got.words.iter().zip(&row.words) {
        if g.word != w.word
            || g.phonemes != w.phonemes
            || g.source.as_str() != w.source
            || g.guessed != w.guessed
        {
            return Some(format!("word {g:?}, want {w:?}"));
        }
    }
    None
}

/// Every golden row through the port with the table guesser: the rows
/// matched whole, and why each other row did not.
pub fn check(dict: &Dictionary) -> (usize, Vec<String>) {
    let mut guesser = TableGuess::load();
    let mut matched = 0;
    let mut failures = Vec::new();
    for (file, layers) in FILES {
        let lexicon = lexicon(layers, dict);
        for row in rows(file) {
            match phonemize(&row.text, dict, &lexicon, &mut guesser) {
                Ok(got) => match differs(&got, &row) {
                    None => matched += 1,
                    Some(why) => failures.push(format!("{file} {}: {why}", row.id)),
                },
                Err(e) => failures.push(format!("{file} {}: {e}", row.id)),
            }
        }
    }
    for f in &failures {
        println!("FAIL {f}");
    }
    (matched, failures)
}
