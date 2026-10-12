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

/// The 7 bake-off inputs (library lexicon), the same without a lexicon, and
/// the special cases.
pub const FILES: [&str; 15] = [
    "script163.jsonl",
    "maths16.jsonl",
    "propernouns.jsonl",
    "lecture1.jsonl",
    "lecture2.jsonl",
    "lecture3.jsonl",
    "heteronyms30.jsonl",
    "nolex/script163.jsonl",
    "nolex/maths16.jsonl",
    "nolex/propernouns.jsonl",
    "nolex/lecture1.jsonl",
    "nolex/lecture2.jsonl",
    "nolex/lecture3.jsonl",
    "nolex/heteronyms30.jsonl",
    "special.jsonl",
];

/// Every golden row run through the port with no lexicon and the table
/// guesser. A row the heteronym table or the lexicon decides somewhere is
/// left to those (its dictionary words are still compared when it runs);
/// every other row must match whole.
#[derive(Default, Debug)]
pub struct Report {
    pub whole: usize,
    pub dict_words: usize,
    /// Dictionary words not compared because a lexicon or guessed word
    /// follows.
    pub before_lexicon: usize,
    pub deferred: Vec<String>,
    pub skipped: Vec<String>,
    pub failures: Vec<String>,
}

fn same(got: &Phonemized, row: &Row) -> Option<String> {
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

pub fn check(dict: &Dictionary) -> Report {
    let lexicon = Lexicon::default();
    let mut guesser = TableGuess::load();
    let mut report = Report::default();
    for file in FILES {
        for row in rows(file) {
            let id = format!("{file} {}", row.id);
            let later = row
                .words
                .iter()
                .any(|w| w.source == "lexicon" || w.source == "rule");
            let got = phonemize(&row.text, dict, &lexicon, &mut guesser);
            match (got, later) {
                (Ok(got), false) => match same(&got, &row) {
                    None => report.whole += 1,
                    Some(why) => report.failures.push(format!("{id}: {why}")),
                },
                (Err(e), false) => report.failures.push(format!("{id}: {e}")),
                (Ok(got), true) => {
                    report.deferred.push(id.clone());
                    if got.words.len() != row.words.len() {
                        report.failures.push(format!(
                            "{id}: {} words, want {}",
                            got.words.len(),
                            row.words.len()
                        ));
                        continue;
                    }
                    for (j, (g, w)) in got.words.iter().zip(&row.words).enumerate() {
                        if w.source != "dict" {
                            continue;
                        }
                        // The next word's first sound decides this one's reading
                        // ("the" before "Cauchy-Schwarz"), and with no lexicon a
                        // lexicon word, or a guessed word with a lexicon part,
                        // reads differently.
                        if row
                            .words
                            .get(j + 1)
                            .is_some_and(|n| n.source == "lexicon" || n.source == "guess")
                        {
                            report.before_lexicon += 1;
                            continue;
                        }
                        report.dict_words += 1;
                        if g.word != w.word
                            || g.phonemes != w.phonemes
                            || g.source.as_str() != "dict"
                        {
                            report
                                .failures
                                .push(format!("{id}: dict word {g:?}, want {w:?}"));
                        }
                    }
                }
                (Err(e), true) => report.skipped.push(format!("{id}: {e}")),
            }
        }
    }
    for row in &report.failures {
        println!("FAIL {row}");
    }
    report
}
