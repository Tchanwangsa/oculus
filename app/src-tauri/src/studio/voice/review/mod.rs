//! Pronunciation review: the Studio agent suggests a reading for a word it
//! doubts (`oculus studio pronounce suggest`), and the student decides in
//! Settings -> Voice. Accepting the suggestion writes it into the lexicon;
//! keeping the guess pins the guessed phonemes there as `/…/`; rejecting it
//! marks it `wrong` with the student's note for the agent to try again. The
//! review is the only writer of `lexicon.json` (`lexicon`).
//!
//! Each suggestion is its own JSON file,
//! `<data dir>/studio/pronunciations/<subject or _library>/<key>.json`
//! (`key`), because the CLI and the app are two processes with no lock
//! between them: neither ever rewrites a list the other holds. A new
//! suggestion for the same subject and word replaces the file.

mod key;
mod lexicon;
mod locate;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::library::paths::{data_dir, iso8601_utc, studio_dir};
use crate::runtime::{atomic_write, clock};
use crate::studio::voice::g2p::Dictionary;
use crate::studio::voice::lint::Note;

pub use key::LIBRARY;
pub use lexicon::{check, write_entry};
pub use locate::{locate, Located};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Waiting for the student.
    Pending,
    /// The student said the suggestion is wrong.
    Wrong,
}

/// The student's verdict on a pending suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    /// Write the suggested reading into the lexicon.
    Correct,
    /// Pin the original guess as `/phonemes/`, so it is no longer a guess.
    Fine,
}

/// One suggestion, as its file holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// The word as the script spells it.
    pub word: String,
    /// The suggested lexicon value: a respelling, or `/phonemes/`.
    pub reading: String,
    /// The phonemes Kokoro read the word with when it was suggested.
    pub original: String,
    /// Whether the guesser read it (the agent may suggest any word).
    pub guessed: bool,
    pub subject: Option<String>,
    /// The video's folder name (its slug).
    pub video: String,
    /// The spoken sentence the word is in: what the samples say.
    pub sentence: String,
    /// The scene's id and the script.md line (1-based).
    pub scene: String,
    pub line: usize,
    pub status: Status,
    /// The student's note on a wrong suggestion.
    pub note: Option<String>,
    /// `YYYY-MM-DDTHH:MM:SS`, UTC.
    pub created: String,
}

/// A record with its id (`<subject or _library>/<key>`), for the app and
/// the CLI.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub id: String,
    #[serde(flatten)]
    pub record: Record,
}

/// What a note shows of a word's suggestion.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Review {
    pub status: Status,
    pub reading: String,
    pub note: Option<String>,
}

impl Review {
    /// `(suggested "NESS-ter-off", awaiting review in Settings -> Voice)`,
    /// or the student's verdict on a wrong one.
    pub fn suffix(&self) -> String {
        match (self.status, &self.note) {
            (Status::Pending, _) => format!(
                "(suggested \"{}\", awaiting review in Settings -> Voice)",
                self.reading
            ),
            (Status::Wrong, Some(note)) => format!(
                "(suggestion \"{}\" marked wrong: \"{note}\"; suggest again)",
                self.reading
            ),
            (Status::Wrong, None) => format!(
                "(suggestion \"{}\" marked wrong; suggest again)",
                self.reading
            ),
        }
    }
}

/// What deciding wrote.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Decided {
    /// The lexicon file written.
    pub lexicon: PathBuf,
    pub word: String,
    pub reading: String,
}

/// A new suggestion, before it is stored.
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub reading: String,
    pub subject: Option<String>,
    pub video: String,
    pub located: Located,
}

/// The store and the lexicons it writes to.
#[derive(Debug, Clone)]
pub struct Store {
    /// `<data dir>/studio/pronunciations`.
    pub root: PathBuf,
    /// The global library's folder (its `lexicon.json`).
    pub library: PathBuf,
    /// The subjects' libraries' folder (`<CODE>/lexicon.json`).
    pub subjects: PathBuf,
}

impl Store {
    pub fn app() -> Self {
        use crate::studio::libraries::{global_dir, subjects_dir};
        Self {
            root: studio_dir(&data_dir()).join("pronunciations"),
            library: global_dir(),
            subjects: subjects_dir(),
        }
    }

    /// The lexicon a decision for `subject` writes: the subject's, else the
    /// global library's.
    pub fn lexicon_file(&self, subject: Option<&str>) -> PathBuf {
        match subject {
            Some(code) => self.subjects.join(code).join("lexicon.json"),
            None => self.library.join("lexicon.json"),
        }
    }

    /// Stores `s` as pending, replacing any suggestion for the same subject
    /// and word.
    pub fn suggest(&self, s: Suggestion) -> Result<Item, String> {
        let id = key::id(s.subject.as_deref(), &s.located.word)?;
        let record = Record {
            word: s.located.word,
            reading: s.reading,
            original: s.located.original,
            guessed: s.located.guessed,
            subject: s.subject,
            video: s.video,
            sentence: s.located.sentence,
            scene: s.located.scene,
            line: s.located.line,
            status: Status::Pending,
            note: None,
            created: iso8601_utc(clock::now_secs()),
        };
        atomic_write::json(&self.path(&id)?, &record)?;
        Ok(Item { id, record })
    }

    /// Every suggestion, newest first, keeping those `keep` passes. A file
    /// that does not parse is skipped and logged.
    pub fn list(&self, keep: impl Fn(&Record) -> bool) -> Result<Vec<Item>, String> {
        let mut items = Vec::new();
        for folder in read_dir(&self.root)? {
            if !folder.is_dir() {
                continue;
            }
            for file in read_dir(&folder)? {
                if file.extension().is_none_or(|e| e != "json") {
                    continue;
                }
                let (Some(folder), Some(key)) = (name(&folder), stem(&file)) else {
                    continue;
                };
                match read(&file) {
                    Ok(record) if keep(&record) => items.push(Item {
                        id: format!("{folder}/{key}"),
                        record,
                    }),
                    Ok(_) => {}
                    Err(e) => elog!("[oculus] pronunciation {}: {e}", file.display()),
                }
            }
        }
        items.sort_by(|a, b| {
            b.record
                .created
                .cmp(&a.record.created)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(items)
    }

    pub fn get(&self, id: &str) -> Result<Item, String> {
        let path = self.path(id)?;
        let record = read(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => format!("no pronunciation suggestion {id:?}"),
            _ => format!("{}: {e}", path.display()),
        })?;
        Ok(Item {
            id: id.to_owned(),
            record,
        })
    }

    /// The suggestion for `word` under `subject`, if one is stored.
    pub fn find(&self, subject: Option<&str>, word: &str) -> Option<Record> {
        let id = key::id(subject, word).ok()?;
        read(&self.path(&id).ok()?).ok()
    }

    /// Marks `id` wrong with the student's `note` (blank is none).
    pub fn mark_wrong(&self, id: &str, note: Option<String>) -> Result<Item, String> {
        let mut item = self.get(id)?;
        item.record.status = Status::Wrong;
        item.record.note = note.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
        atomic_write::json(&self.path(id)?, &item.record)?;
        Ok(item)
    }

    /// Writes `id`'s word into its lexicon, as the suggestion (`Correct`) or
    /// as the original phonemes (`Fine`), then deletes the item, unless a
    /// new suggestion replaced it meanwhile.
    pub fn decide(
        &self,
        id: &str,
        decision: Decision,
        dict: &Dictionary,
    ) -> Result<Decided, String> {
        let Item { record, .. } = self.get(id)?;
        let reading = match decision {
            Decision::Correct => record.reading.clone(),
            Decision::Fine => format!("/{}/", record.original),
        };
        let lexicon = self.lexicon_file(record.subject.as_deref());
        write_entry(&lexicon, &record.word, &reading, dict)?;
        let path = self.path(id)?;
        if read(&path).is_ok_and(|now| now == record) {
            std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        Ok(Decided {
            lexicon,
            word: record.word,
            reading,
        })
    }

    /// Gives each note the review of its word (a possessive's, of its
    /// stem), if one is stored under `subject`.
    pub fn annotate(&self, subject: Option<&str>, notes: &mut [Note]) {
        for note in notes {
            let word = note.word.replace('’', "'");
            let stem = word.strip_suffix("'s").or_else(|| word.strip_suffix("'S"));
            let found = self
                .find(subject, &word)
                .or_else(|| stem.and_then(|stem| self.find(subject, stem)));
            note.review = found.map(|r| Review {
                status: r.status,
                reading: r.reading,
                note: r.note,
            });
        }
    }

    fn path(&self, id: &str) -> Result<PathBuf, String> {
        let (folder, key) = key::parse(id)?;
        Ok(self.root.join(folder).join(format!("{key}.json")))
    }
}

fn read(path: &Path) -> std::io::Result<Record> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// The entries of `dir`; none when it does not exist.
fn read_dir(dir: &Path) -> Result<Vec<PathBuf>, String> {
    match std::fs::read_dir(dir) {
        Ok(entries) => Ok(entries.flatten().map(|e| e.path()).collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{}: {e}", dir.display())),
    }
}

fn name(path: &Path) -> Option<&str> {
    path.file_name()?.to_str()
}

fn stem(path: &Path) -> Option<&str> {
    path.file_stem()?.to_str()
}
